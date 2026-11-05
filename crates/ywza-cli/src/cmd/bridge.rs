use anyhow::Result;
use ywza_proto::Control;

pub async fn run(c: &mut Control) -> Result<()> {
    let (mut stdin, mut stdout) = (tokio::io::stdin(), tokio::io::stdout());
    // Either side closing ends the bridge.
    let r = tokio::select! {
        r = tokio::io::copy(&mut stdin, &mut c.send) => r,
        r = tokio::io::copy(&mut c.recv, &mut stdout) => r,
    };
    // Exit right here: tokio's stdin reader thread would block runtime shutdown until the
    // client closes our stdin, so the client would never see EOF when the agent drops the
    // link (e.g. a login-screen switch) and its call would hang.
    if let Err(e) = &r {
        eprintln!("ywza bridge: {e}");
    }
    c.conn.close(0u32.into(), b"bye");
    std::process::exit(if r.is_ok() { 0 } else { 1 });
}
