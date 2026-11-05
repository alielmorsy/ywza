use anyhow::Result;

use crate::connect::devices_dir;

pub fn run() -> Result<()> {
    let mut names: Vec<String> = std::fs::read_dir(devices_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.strip_suffix(".json").map(String::from)
        })
        .collect();
    names.sort();
    for n in names {
        println!("{n}");
    }
    Ok(())
}
