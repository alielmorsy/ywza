//! Finding the X server's cookie file for a display.

use std::fs;

/// The `-auth` file of the X server for `display` (`:N`, `:N.S` or `host:N`).
pub fn xserver_auth(display: &str) -> Option<String> {
    for e in fs::read_dir("/proc").ok()?.flatten() {
        let Ok(raw) = fs::read(e.path().join("cmdline")) else {
            continue;
        };
        let argv: Vec<String> = raw
            .split(|&b| b == 0)
            .map(|a| String::from_utf8_lossy(a).into_owned())
            .collect();
        if let Some(auth) = auth_arg(&argv, display) {
            return Some(auth);
        }
    }
    None
}

fn auth_arg(argv: &[String], display: &str) -> Option<String> {
    let exe = argv.first()?.rsplit('/').next()?;
    if !matches!(exe, "Xorg" | "X" | "Xorg.bin") {
        return None;
    }
    let num = display.rsplit_once(':')?.1;
    let num = num.split('.').next()?;
    if !argv.iter().any(|a| a.strip_prefix(':') == Some(num)) {
        return None;
    }
    let i = argv.iter().position(|a| a == "-auth")?;
    argv.get(i + 1).cloned()
}

#[cfg(test)]
mod tests {
    use super::auth_arg;

    fn argv(s: &str) -> Vec<String> {
        s.split(' ').map(String::from).collect()
    }

    #[test]
    fn auth_arg_matches_display() {
        let lightdm = argv(
            "/usr/lib/xorg/Xorg :0 -seat seat0 -auth /var/run/lightdm/root/:0 -nolisten tcp vt7",
        );
        assert_eq!(
            auth_arg(&lightdm, ":0").as_deref(),
            Some("/var/run/lightdm/root/:0")
        );
        assert_eq!(
            auth_arg(&lightdm, ":0.0").as_deref(),
            Some("/var/run/lightdm/root/:0")
        );
        assert_eq!(auth_arg(&lightdm, ":1"), None);
        let sddm = argv("/usr/bin/X -nolisten tcp -auth /run/sddm/xauth_x :0 vt1");
        assert_eq!(auth_arg(&sddm, ":0").as_deref(), Some("/run/sddm/xauth_x"));
        assert_eq!(auth_arg(&argv("/usr/bin/Xvfb :0 -auth /a"), ":0"), None);
        assert_eq!(auth_arg(&argv("/usr/lib/xorg/Xorg :0 vt7"), ":0"), None);
    }
}
