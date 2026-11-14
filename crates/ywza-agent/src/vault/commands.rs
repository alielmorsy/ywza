use std::{
    io::{IsTerminal, Write},
    path::Path,
};

use super::store::{load, save, FILE};

pub const USAGE: &str = "usage: ywza-agent vault set <key> | list | remove <key>";

/// `vault set <key>` (the value is one stdin line, so it stays out of shell history),
/// `vault list` (keys only), `vault remove <key>`. `args` follow the word `vault`.
pub fn cli(dir: &Path, args: &[String]) -> anyhow::Result<()> {
    let key = args.get(1).map(String::as_str);
    match (args.first().map(String::as_str), key) {
        (Some("set"), Some(key)) => {
            anyhow::ensure!(
                !key.is_empty() && !key.contains('}'),
                "the key must not be empty or contain `}}`"
            );
            let value = read_value()?;
            let mut entries = load(dir)?;
            entries.insert(key.into(), value);
            save(dir, &entries)?;
            eprintln!("saved `{key}` in {}; use it as {{{{vault:{key}}}}}", dir.join(FILE).display());
        }
        (Some("list"), None) => {
            for key in load(dir)?.keys() {
                println!("{key}");
            }
        }
        (Some("remove"), Some(key)) => {
            let mut entries = load(dir)?;
            anyhow::ensure!(entries.remove(key).is_some(), "no vault key `{key}`");
            save(dir, &entries)?;
        }
        _ => anyhow::bail!("{USAGE}"),
    }
    Ok(())
}

fn read_value() -> anyhow::Result<String> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        eprint!("value (shown as you type): ");
        std::io::stderr().flush()?;
    }
    let mut line = String::new();
    stdin.read_line(&mut line)?;
    let value = line.trim_end_matches(['\r', '\n']);
    anyhow::ensure!(!value.is_empty(), "no value on stdin");
    Ok(value.to_string())
}
