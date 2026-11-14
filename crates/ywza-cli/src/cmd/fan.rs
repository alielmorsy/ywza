//! `ywza fan`: one tool on several devices at once.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::task::JoinSet;
use ywza_proto::client::{self, fanout_section};

use super::remote::call;

pub async fn run(spec: &str, tool: &str, args: Option<&str>) -> Result<()> {
    let args: Value = match args {
        Some(a) => serde_json::from_str(a).context("the arguments are not valid JSON")?,
        None => json!({}),
    };
    let targets = client::parse_targets(spec, &client::list_devices())?;
    let mut set = JoinSet::new();
    for (i, device) in targets.iter().cloned().enumerate() {
        let (tool, args) = (tool.to_string(), args.clone());
        set.spawn(async move {
            let r = call(&device, &tool, args).await;
            (i, r)
        });
    }
    let mut sections = vec![String::new(); targets.len()];
    let mut failed = 0;
    while let Some(done) = set.join_next().await {
        let (i, r) = done?;
        let device = &targets[i];
        sections[i] = match r {
            Ok(r) => {
                failed += !r.ok as usize;
                fanout_section(device, r.ok, &r.text, r.png_base64.is_some())
            }
            Err(e) => {
                failed += 1;
                fanout_section(device, false, &format!("{e:#}"), false)
            }
        };
    }
    println!("{}", sections.join("\n\n"));
    if failed > 0 {
        bail!("{failed} of {} devices failed", targets.len());
    }
    Ok(())
}
