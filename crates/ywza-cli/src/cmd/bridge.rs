use anyhow::Result;
use ywza_proto::Control;

pub async fn run(c: &mut Control) -> Result<()> {
    let (mut stdin, mut stdout) = (tokio::io::stdin(), tokio::io::stdout());
    // Either side closing ends the bridge.
    let r = tokio::select! {
        r = tokio::io::copy(&mut stdin, &mut c.send) => r,
        r = tokio::io::copy(&mut c.recv, &mut stdout) => r,
    };
    if let Err(e) = &r {
        tracing::error!(error = %e, "bridge stopped");
    }
    r?;
    Ok(())
}
