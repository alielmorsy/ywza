use std::path::Path;

use super::{compile, describe, store};

pub const USAGE: &str =
    "usage: ywza-agent plans list | show <name> | delete <name> | save <name> [<session-id>]";

/// `plans list`, `show <name>`, `delete <name>`, and `save <name> [<session-id>]`, which compiles
/// the steps recorded in that session (default: the most recent) into a plan. `args` follow the
/// word `plans`.
pub fn run(dir: &Path, args: &[String]) -> anyhow::Result<()> {
    super::init(dir);
    let arg = |i: usize| args.get(i).map(String::as_str);
    match (arg(0), arg(1), arg(2)) {
        (Some("list"), None, None) => {
            for p in store::list() {
                println!("{}", describe(&p));
            }
        }
        (Some("show"), Some(name), None) => {
            println!("{}", serde_json::to_string_pretty(&store::load(name)?)?);
        }
        (Some("delete"), Some(name), None) => store::delete(name)?,
        (Some("save"), Some(name), session) if args.len() <= 3 => {
            let file = match session {
                Some(id) => compile::for_session(id)?,
                None => compile::latest()?,
            };
            let plan = compile::compile(&file, name)?;
            store::save(&plan)?;
            eprintln!(
                "saved plan `{name}`: {} steps, {} params, from {}",
                plan.steps.len(),
                plan.params.len(),
                file.display()
            );
        }
        _ => anyhow::bail!("{USAGE}"),
    }
    Ok(())
}
