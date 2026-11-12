use std::path::PathBuf;

use anyhow::{bail, Result};
use ywza_proto::{client::ask, AgentMsg, ClientMsg, Control, Target};

pub async fn run(
    c: &mut Control,
    monitor: Option<u32>,
    window: Option<u64>,
    output: PathBuf,
) -> Result<()> {
    let target = match window {
        Some(id) => Target::Window { id },
        None => Target::Monitor {
            id: monitor.unwrap_or(0),
        },
    };
    match ask(c, ClientMsg::Screenshot { target, max_width: None }).await? {
        AgentMsg::Image {
            png_base64,
            width,
            height,
        } => {
            std::fs::write(
                &output,
                data_encoding::BASE64.decode(png_base64.as_bytes())?,
            )?;
            println!("{} ({width}x{height})", output.display());
            Ok(())
        }
        m => bail!("unexpected reply {m:?}"),
    }
}
