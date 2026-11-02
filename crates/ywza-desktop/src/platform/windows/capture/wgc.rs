use super::pixels::bgra_to_rgba;
use crate::platform::windows::d3d;
use crate::platform::windows::util::com;
use crate::{Error, Image, Result};
use ::windows::core::Interface;
use ::windows::Foundation::TypedEventHandler;
use ::windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use ::windows::Graphics::DirectX::DirectXPixelFormat;
use ::windows::Win32::Graphics::Direct3D11::*;
use ::windows::Win32::System::WinRT::Direct3D11::IDirect3DDxgiInterfaceAccess;
use ::windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use std::sync::mpsc;
use std::time::Duration;

pub(super) fn item(
    create: impl FnOnce(&IGraphicsCaptureItemInterop) -> ::windows::core::Result<GraphicsCaptureItem>,
) -> Result<GraphicsCaptureItem> {
    com();
    let interop = ::windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    Ok(create(&interop)?)
}

/// Starts a session, takes the first frame and copies it to CPU memory as RGBA.
pub(super) fn grab(item: GraphicsCaptureItem) -> Result<Image> {
    let (device, ctx, winrt_device) = d3d::create_device()?;
    unsafe {
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &winrt_device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            1,
            item.Size()?,
        )?;
        let session = pool.CreateCaptureSession(&item)?;
        // Both can fail on older builds; a border or cursor in the image is acceptable.
        let _ = session.SetIsBorderRequired(false);
        let _ = session.SetIsCursorCaptureEnabled(false);
        let (tx, rx) = mpsc::channel();
        let token = pool.FrameArrived(&TypedEventHandler::new(move |_, _| {
            let _ = tx.send(());
            Ok(())
        }))?;
        session.StartCapture()?;
        let arrived = rx.recv_timeout(Duration::from_secs(2));
        let frame = arrived.ok().and_then(|_| pool.TryGetNextFrame().ok());
        // Order matters: session before pool, or in-flight callbacks hit a closed pool.
        let _ = pool.RemoveFrameArrived(token);
        let _ = session.Close();
        let result = match frame {
            Some(frame) => {
                let tex: ID3D11Texture2D = frame
                    .Surface()?
                    .cast::<IDirect3DDxgiInterfaceAccess>()?
                    .GetInterface()?;
                let size = frame.ContentSize()?;
                let img = read_texture(&device, &ctx, &tex, size.Width as u32, size.Height as u32);
                let _ = frame.Close();
                img
            }
            None => Err(Error::Timeout("no capture frame".into())),
        };
        let _ = pool.Close();
        result
    }
}

unsafe fn read_texture(
    device: &ID3D11Device,
    ctx: &ID3D11DeviceContext,
    tex: &ID3D11Texture2D,
    width: u32,
    height: u32,
) -> Result<Image> {
    unsafe {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        tex.GetDesc(&mut desc);
        let mut staging = None;
        device.CreateTexture2D(&d3d::staging_desc(desc), None, Some(&mut staging))?;
        let staging = staging.unwrap();
        ctx.CopyResource(&staging, tex);
        let mut map = D3D11_MAPPED_SUBRESOURCE::default();
        ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut map))?;
        let (w, h) = (width.min(desc.Width), height.min(desc.Height));
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let row = std::slice::from_raw_parts(
                (map.pData as *const u8).add(y * map.RowPitch as usize),
                w as usize * 4,
            );
            rgba.extend_from_slice(row);
        }
        ctx.Unmap(&staging, 0);
        bgra_to_rgba(&mut rgba);
        Ok(Image {
            width: w,
            height: h,
            rgba,
            origin: (0, 0),
        })
    }
}
