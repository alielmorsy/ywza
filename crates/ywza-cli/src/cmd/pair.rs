use anyhow::Result;

pub async fn run(code: &str, name: &str) -> Result<()> {
    ywza_proto::client::pair(code, name).await?;
    println!("paired as `{name}`; use --device {name}");
    Ok(())
}
