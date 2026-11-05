//! Live view over loopback: needs a real desktop (skipped when no monitor is found).

mod common;

use std::time::Duration;

use common::start;
use ywza_proto::{read_frame, write_frame, ClientMsg, FrameHeader};

#[tokio::test(flavor = "multi_thread")]
async fn live_start_sends_a_keyframe_stream() {
    if ywza_desktop::list_monitors().map_or(true, |m| m.is_empty()) {
        eprintln!("no monitor; skipping");
        return;
    }
    let (_dir, client, mut c) = start(false).await;

    write_frame(&mut c.send, &ClientMsg::LiveStart { monitor: 0 })
        .await
        .unwrap();
    let mut uni = tokio::time::timeout(Duration::from_secs(10), c.conn.accept_uni())
        .await
        .expect("no frame stream within 10 s")
        .unwrap();
    let h: FrameHeader = read_frame(&mut uni).await.unwrap();
    let data = uni.read_to_end(16 << 20).await.unwrap();
    assert_eq!((h.monitor, h.seq), (0, 0));
    assert!(h.keyframe, "first frame must be a keyframe");
    assert!(h.width > 0 && h.width <= 1920 && h.height > 0, "{h:?}");
    assert_eq!(&data[..4], &[0, 0, 0, 1], "Annex-B start code");
    // SPS (NAL type 7) leads a keyframe.
    assert_eq!(data[4] & 0x1f, 7);
    eprintln!("first frame: {h:?}, {} bytes", data.len());

    write_frame(&mut c.send, &ClientMsg::LiveStop)
        .await
        .unwrap();
    c.conn.close(0u32.into(), b"bye");
    client.close().await;
}
