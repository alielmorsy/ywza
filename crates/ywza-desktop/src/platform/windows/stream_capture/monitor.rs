use crate::platform::windows::util::com;
use crate::platform::windows::{d3d, wm};
use crate::Result;
use ::windows::core::Interface;
use ::windows::Foundation::TypedEventHandler;
use ::windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use ::windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use ::windows::Graphics::DirectX::DirectXPixelFormat;
use ::windows::Graphics::SizeInt32;
use ::windows::Win32::Graphics::Direct3D11::*;
use ::windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use ::windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const FORMAT: DirectXPixelFormat = DirectXPixelFormat::B8G8R8A8UIntNormalized;

/// A running WGC session on one monitor. WGC only delivers a frame when the screen
/// (or cursor) changes, so a static screen yields nothing. Not `Send`: create it on
/// the thread that reads it.
pub struct MonitorCapture {
    device: ID3D11Device,
    ctx: ID3D11DeviceContext,
    winrt_device: IDirect3DDevice,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    token: i64,
    rx: mpsc::Receiver<()>,
    size: SizeInt32,
    staging: Option<(ID3D11Texture2D, u32, u32)>,
    frame: Duration,
    next_at: Instant,
}

impl MonitorCapture {
    /// Starts capturing monitor `index` (as in `list_monitors`) at up to `fps`,
    /// with or without the cursor.
    pub fn new(index: usize, fps: u32, cursor: bool) -> Result<Self> {
        com();
        let m = wm::monitor_handle(index)?;
        let interop =
            ::windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
        let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(m)? };
        let (device, ctx, winrt_device) = d3d::create_device()?;
        let size = item.Size()?;
        let pool =
            Direct3D11CaptureFramePool::CreateFreeThreaded(&winrt_device, FORMAT, 2, size)?;
        let session = pool.CreateCaptureSession(&item)?;
        let _ = session.SetIsBorderRequired(false);
        let _ = session.SetIsCursorCaptureEnabled(cursor);
        let (tx, rx) = mpsc::channel();
        let token = pool.FrameArrived(&TypedEventHandler::new(move |_, _| {
            let _ = tx.send(());
            Ok(())
        }))?;
        session.StartCapture()?;
        Ok(Self {
            device,
            ctx,
            winrt_device,
            pool,
            session,
            token,
            rx,
            size,
            staging: None,
            frame: Duration::from_secs(1) / fps.max(1),
            next_at: Instant::now(),
        })
    }

    /// Waits up to `timeout` for a changed frame and copies the newest one into `out`
    /// as tightly packed BGRA. Returns its size, or None on timeout.
    pub fn next(&mut self, timeout: Duration, out: &mut Vec<u8>) -> Result<Option<(u32, u32)>> {
        // Pace to the frame rate; frames arriving meanwhile are drained to the newest below.
        let wait = self.next_at.saturating_duration_since(Instant::now());
        if !wait.is_zero() {
            std::thread::sleep(wait.min(timeout));
        }
        if self.rx.recv_timeout(timeout.saturating_sub(wait)).is_err() {
            return Ok(None);
        }
        self.next_at = Instant::now() + self.frame;
        while self.rx.try_recv().is_ok() {}
        // Older frames are stale.
        let mut frame = None;
        while let Ok(f) = self.pool.TryGetNextFrame() {
            if let Some(old) = frame.replace(f) {
                let _ = old.Close();
            }
        }
        let Some(frame) = frame else { return Ok(None) };
        let size = frame.ContentSize()?;
        let result = unsafe {
            let tex: ID3D11Texture2D = frame
                .Surface()?
                .cast::<IDirect3DDxgiInterfaceAccess>()?
                .GetInterface()?;
            self.read(&tex, size.Width as u32, size.Height as u32, out)
        };
        let _ = frame.Close();
        if size != self.size {
            // Monitor resolution changed: resize the pool; the next frame has the new size.
            self.size = size;
            self.pool.Recreate(&self.winrt_device, FORMAT, 2, size)?;
        }
        result.map(Some)
    }

    unsafe fn read(
        &mut self,
        tex: &ID3D11Texture2D,
        width: u32,
        height: u32,
        out: &mut Vec<u8>,
    ) -> Result<(u32, u32)> {
        unsafe {
            let mut desc = D3D11_TEXTURE2D_DESC::default();
            tex.GetDesc(&mut desc);
            if !matches!(&self.staging, Some((_, w, h)) if *w == desc.Width && *h == desc.Height) {
                let mut staging = None;
                self.device
                    .CreateTexture2D(&d3d::staging_desc(desc), None, Some(&mut staging))?;
                self.staging = Some((staging.unwrap(), desc.Width, desc.Height));
            }
            let staging = &self.staging.as_ref().unwrap().0;
            self.ctx.CopyResource(staging, tex);
            let mut map = D3D11_MAPPED_SUBRESOURCE::default();
            self.ctx
                .Map(staging, 0, D3D11_MAP_READ, 0, Some(&mut map))?;
            let (w, h) = (width.min(desc.Width), height.min(desc.Height));
            out.clear();
            out.reserve((w * h * 4) as usize);
            for y in 0..h as usize {
                let row = std::slice::from_raw_parts(
                    (map.pData as *const u8).add(y * map.RowPitch as usize),
                    w as usize * 4,
                );
                out.extend_from_slice(row);
            }
            self.ctx.Unmap(staging, 0);
            Ok((w, h))
        }
    }
}

impl Drop for MonitorCapture {
    fn drop(&mut self) {
        // Session before pool, or in-flight callbacks hit a closed pool.
        let _ = self.pool.RemoveFrameArrived(self.token);
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}
