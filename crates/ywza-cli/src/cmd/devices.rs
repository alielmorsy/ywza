use anyhow::Result;

pub fn run() -> Result<()> {
    for n in ywza_proto::client::list_devices() {
        println!("{n}");
    }
    Ok(())
}
