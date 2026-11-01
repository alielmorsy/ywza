use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::Error;

/// Largest accepted frame body, in bytes.
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

/// Writes `msg` as one frame: u32 little-endian length, then JSON.
pub async fn write_frame<W, T>(w: &mut W, msg: &T) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
    T: Serialize,
{
    let mut buf = vec![0u8; 4];
    serde_json::to_writer(&mut buf, msg)?;
    let len = buf.len() - 4;
    if len > MAX_FRAME {
        return Err(Error::FrameTooLarge { len });
    }
    buf[..4].copy_from_slice(&(len as u32).to_le_bytes());
    w.write_all(&buf).await?;
    w.flush().await?;
    Ok(())
}

/// Reads one frame. Returns [`Error::Closed`] if the stream ends before a frame starts.
pub async fn read_frame<R, T>(r: &mut R) -> Result<T, Error>
where
    R: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let mut head = [0u8; 4];
    match r.read_exact(&mut head).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Err(Error::Closed),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_le_bytes(head) as usize;
    if len > MAX_FRAME {
        return Err(Error::FrameTooLarge { len });
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).await?;
    Ok(serde_json::from_slice(&body)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClientMsg;

    #[tokio::test]
    async fn round_trip_and_clean_close() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let msg = ClientMsg::ListSessions;
        write_frame(&mut a, &msg).await.unwrap();
        assert_eq!(read_frame::<_, ClientMsg>(&mut b).await.unwrap(), msg);
        drop(a);
        assert!(matches!(
            read_frame::<_, ClientMsg>(&mut b).await,
            Err(Error::Closed)
        ));
    }

    #[tokio::test]
    async fn rejects_oversize_on_read_without_allocating() {
        let (mut a, mut b) = tokio::io::duplex(16);
        a.write_all(&((MAX_FRAME as u32) + 1).to_le_bytes())
            .await
            .unwrap();
        let err = read_frame::<_, ClientMsg>(&mut b).await.unwrap_err();
        assert!(matches!(err, Error::FrameTooLarge { len } if len == MAX_FRAME + 1));
    }

    #[tokio::test]
    async fn rejects_oversize_on_write() {
        let (mut a, _b) = tokio::io::duplex(16);
        let big = "x".repeat(MAX_FRAME);
        let err = write_frame(&mut a, &big).await.unwrap_err();
        assert!(matches!(err, Error::FrameTooLarge { .. }));
    }

    #[tokio::test]
    async fn bad_json_is_an_error() {
        let (mut a, mut b) = tokio::io::duplex(64);
        a.write_all(&3u32.to_le_bytes()).await.unwrap();
        a.write_all(b"{x}").await.unwrap();
        assert!(matches!(
            read_frame::<_, ClientMsg>(&mut b).await,
            Err(Error::Json(_))
        ));
    }
    #[tokio::test]
    async fn live_frame_header_then_payload() {
        use crate::FrameHeader;
        let (mut a, mut b) = tokio::io::duplex(1024);
        let h = FrameHeader {
            monitor: 1,
            seq: 7,
            pts_us: 33_000,
            keyframe: true,
            width: 1920,
            height: 1080,
        };
        write_frame(&mut a, &h).await.unwrap();
        a.write_all(&[0, 0, 0, 1, 0x67]).await.unwrap();
        drop(a);
        assert_eq!(read_frame::<_, FrameHeader>(&mut b).await.unwrap(), h);
        let mut rest = Vec::new();
        b.read_to_end(&mut rest).await.unwrap();
        assert_eq!(rest, [0, 0, 0, 1, 0x67]);
    }
}
