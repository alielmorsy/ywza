//! The per-user Explorer verbs as plain data, so the layout is testable without touching the registry.

/// Files, folders, and a folder's background.
pub const ROOTS: [&str; 3] = ["*", "Directory", r"Directory\Background"];
/// Marks the Explorer selection in the app's command line.
pub const ARG: &str = "--explorer";

pub struct Key {
    pub path: String,
    pub values: Vec<(&'static str, String)>,
}

/// The one key per root that holds everything this feature adds; removing it undoes the install.
pub fn verb(root: &str) -> String {
    format!(r"Software\Classes\{root}\shell\ywza")
}

/// A cascading verb: `SubCommands` set to empty makes Explorer read the entries from the verb's
/// own `shell` subkey. Unlike a named `SubCommands` list (HKLM CommandStore) or `IExplorerCommand`
/// (needs package identity on Windows 11), it works per user without admin, and Windows 11 shows
/// it under "Show more options".
pub fn keys(exe: &str) -> Vec<Key> {
    // Command verbs start one process per selected item; "Document" keeps the verb to selections
    // of 15 or fewer, which bounds that fan-out. The app merges the launches into one dock open.
    let multi = || ("MultiSelectModel", "Document".to_string());
    ROOTS
        .iter()
        .flat_map(|root| {
            let target = if *root == ROOTS[2] { "%V" } else { "%1" };
            let top = verb(root);
            [
                Key {
                    path: top.clone(),
                    values: vec![
                        ("MUIVerb", "ywza".into()),
                        ("SubCommands", String::new()),
                        ("Icon", format!("{exe},0")),
                        multi(),
                    ],
                },
                Key {
                    path: format!(r"{top}\shell\ask"),
                    values: vec![("MUIVerb", "Ask ywza about this...".into()), multi()],
                },
                Key {
                    path: format!(r"{top}\shell\ask\command"),
                    values: vec![("", format!("\"{exe}\" {ARG} \"{target}\""))],
                },
            ]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        let exe = r"C:\Apps\ywza\ywza-app.exe";
        let k = keys(exe);
        assert_eq!(k.len(), 9);
        assert!(k
            .iter()
            .all(|k| ROOTS.iter().any(|r| k.path.starts_with(&verb(r)))));
        assert_eq!(k[0].path, r"Software\Classes\*\shell\ywza");
        assert!(k[0].values.contains(&("SubCommands", String::new())));
        assert_eq!(
            k[2].path,
            r"Software\Classes\*\shell\ywza\shell\ask\command"
        );
        assert_eq!(
            k[2].values,
            vec![(
                "",
                r#""C:\Apps\ywza\ywza-app.exe" --explorer "%1""#.to_string()
            )]
        );
        assert_eq!(
            k[5].path,
            r"Software\Classes\Directory\shell\ywza\shell\ask\command"
        );
        assert_eq!(
            k[8].path,
            r"Software\Classes\Directory\Background\shell\ywza\shell\ask\command"
        );
        assert_eq!(
            k[8].values[0].1,
            r#""C:\Apps\ywza\ywza-app.exe" --explorer "%V""#
        );
    }
}
