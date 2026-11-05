use std::os::windows::process::CommandExt;

/// "Windows 11 Pro 25H2 (build 26200.5074)", read through the OS's own `reg` tool: no extra
/// dependency for a prompt line.
pub fn os_description() -> String {
    let out = std::process::Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
        ])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output();
    out.ok()
        .and_then(|o| windows_version(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_else(|| std::env::consts::OS.to_string())
}

/// Parses `reg query ...\CurrentVersion` output. `None` without a ProductName.
fn windows_version(reg: &str) -> Option<String> {
    let value = |name: &str| {
        reg.lines().find_map(|l| {
            let (n, rest) = l.trim().split_once(char::is_whitespace)?;
            let (_, v) = rest.trim_start().split_once(char::is_whitespace)?; // skip REG_SZ
            (n == name).then(|| v.trim().to_string())
        })
    };
    let mut name = value("ProductName").filter(|v| !v.is_empty())?;
    let build = value("CurrentBuild");
    // ProductName still says "Windows 10" on Windows 11.
    if build
        .as_deref()
        .and_then(|b| b.parse::<u32>().ok())
        .is_some_and(|b| b >= 22000)
    {
        name = name.replacen("Windows 10", "Windows 11", 1);
    }
    let mut out = name;
    if let Some(v) = value("DisplayVersion") {
        out += &format!(" {v}");
    }
    if let Some(b) = build {
        // UBR is a REG_DWORD printed as 0x1234.
        let ubr =
            value("UBR").and_then(|u| u32::from_str_radix(u.trim_start_matches("0x"), 16).ok());
        out += &match ubr {
            Some(u) => format!(" (build {b}.{u})"),
            None => format!(" (build {b})"),
        };
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::windows_version;

    #[test]
    fn parses_registry_output() {
        let reg = r"
HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows NT\CurrentVersion
    CurrentBuild    REG_SZ    26200
    DisplayVersion    REG_SZ    25H2
    ProductName    REG_SZ    Windows 10 Pro
    UBR    REG_DWORD    0x1f2e
";
        assert_eq!(
            windows_version(reg).unwrap(),
            "Windows 11 Pro 25H2 (build 26200.7982)"
        );
        let old = "    ProductName    REG_SZ    Windows 10 Home
    CurrentBuild    REG_SZ    19045
";
        assert_eq!(
            windows_version(old).unwrap(),
            "Windows 10 Home (build 19045)"
        );
        assert_eq!(windows_version("nothing"), None);
    }
}
