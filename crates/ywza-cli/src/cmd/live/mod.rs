use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{sync_channel, Receiver, RecvTimeoutError},
        Arc,
    },
    time::Duration,
};

use anyhow::{bail, Result};
use minifb::{Key, ScaleMode, Window, WindowOptions};
use openh264::{decoder::Decoder, formats::YUVSource};
use tokio::task::JoinHandle;
use ywza_proto::{read_frame, write_frame, AgentMsg, ClientMsg, Control, FrameHeader};

mod scale;

const MAX_W: usize = 1600;
const MAX_H: usize = 900;

type Packet = (bool, Vec<u8>);

pub async fn run(c: &mut Control, device: Option<&str>, monitor: u32) -> Result<()> {
    write_frame(&mut c.send, &ClientMsg::LiveStart { monitor }).await?;
    let (tx, rx) = sync_channel::<Packet>(4);
    let conn = c.conn.clone();
    let reader: JoinHandle<()> = tokio::spawn(async move {
        let mut need_key = true;
        while let Ok(mut s) = conn.accept_uni().await {
            let Ok(head) = read_frame::<_, FrameHeader>(&mut s).await else {
                continue;
            };
            let Ok(bytes) = s.read_to_end(32 << 20).await else {
                continue;
            };
            if need_key && !head.keyframe {
                continue;
            }
            need_key = tx.try_send((head.keyframe, bytes)).is_err();
        }
    });

    let stop = Arc::new(AtomicBool::new(false));
    let title = format!("ywza live: {} monitor {monitor}", device.unwrap_or("local"));
    let mut win = tokio::task::spawn_blocking({
        let stop = stop.clone();
        move || window(&title, rx, &stop)
    });
    let r: Result<()> = async {
        loop {
            tokio::select! {
                r = &mut win => return r?,
                m = read_frame::<_, AgentMsg>(&mut c.recv) => match m? {
                    AgentMsg::Error { message } => bail!("agent: {message}"),
                    _ => {}
                },
            }
        }
    }
    .await;
    stop.store(true, Ordering::Relaxed);
    reader.abort();
    let _ = write_frame(&mut c.send, &ClientMsg::LiveStop).await;
    r
}

fn window(title: &str, rx: Receiver<Packet>, stop: &AtomicBool) -> Result<()> {
    let mut dec = Decoder::new()?;
    let (mut rgb, mut buf) = (Vec::new(), Vec::new());
    let (mut w, mut h) = (0, 0);
    let mut need_key = true;
    let mut win: Option<Window> = None;
    let mut last = (0, 0);
    loop {
        let closed = win
            .as_ref()
            .is_some_and(|x| !x.is_open() || x.is_key_down(Key::Escape));
        if closed || stop.load(Ordering::Relaxed) {
            break;
        }
        let first = if win.is_none() {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(p) => Some(p),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        } else {
            None
        };
        let mut fresh = false;
        for (keyframe, bytes) in first.into_iter().chain(rx.try_iter()) {
            need_key &= !keyframe;
            if need_key {
                continue;
            }
            match dec.decode(&bytes) {
                Ok(Some(yuv)) => {
                    (w, h) = yuv.dimensions();
                    rgb.resize(w * h * 3, 0);
                    yuv.write_rgb8(&mut rgb);
                    fresh = true;
                }
                Ok(None) => {}
                Err(_) => need_key = true,
            }
        }
        if win.is_none() {
            if !fresh {
                continue;
            }
            let (ww, wh) = if w <= MAX_W && h <= MAX_H {
                (w, h)
            } else if w * MAX_H > h * MAX_W {
                (MAX_W, h * MAX_W / w)
            } else {
                (w * MAX_H / h, MAX_H)
            };
            let mut x = Window::new(
                title,
                ww,
                wh,
                WindowOptions {
                    resize: true,
                    scale_mode: ScaleMode::Center,
                    ..WindowOptions::default()
                },
            )?;
            x.set_target_fps(60);
            win = Some(x);
        }
        let Some(win) = win.as_mut() else { continue };
        let size = win.get_size();
        if size.0 == 0 || size.1 == 0 {
            win.update();
        } else if fresh || size != last {
            scale::fit(&rgb, w, h, size.0, size.1, &mut buf);
            last = size;
            win.update_with_buffer(&buf, size.0, size.1)?;
        } else {
            win.update();
        }
    }
    Ok(())
}
