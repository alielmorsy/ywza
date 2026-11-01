use std::path::{Path, PathBuf};

use iroh::SecretKey;

use crate::Error;

/// `$YWZA_CONFIG_DIR` if set. Else on Linux `/var/lib/ywza` when the service agent runs for
/// this user (its `agent.ticket` is readable), so clients find it without the variable.
/// Otherwise `%APPDATA%\ywza` on Windows, `$XDG_CONFIG_HOME/ywza` (or `~/.config/ywza`) on Linux.
pub fn config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("YWZA_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "linux")]
    {
        let service = Path::new("/var/lib/ywza");
        if std::fs::File::open(service.join("agent.ticket")).is_ok() {
            return service.into();
        }
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ywza")
}

/// Reads the 32-byte secret key at `path`, creating (and persisting) a new one if the file is missing.
pub fn load_or_create_secret_key(path: &Path) -> Result<SecretKey, Error> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let bytes: [u8; 32] = bytes.try_into().map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "secret key file must be 32 bytes",
                )
            })?;
            Ok(SecretKey::from_bytes(&bytes))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let key = SecretKey::generate();
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(path, key.to_bytes())?;
            Ok(key)
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_key_persists() {
        let path = std::env::temp_dir().join(format!(
            "ywza-proto-{}/agent.key",
            SecretKey::generate().public()
        ));
        let a = load_or_create_secret_key(&path).unwrap();
        let b = load_or_create_secret_key(&path).unwrap();
        assert_eq!(a.public(), b.public());
        std::fs::write(&path, b"short").unwrap();
        assert!(matches!(
            load_or_create_secret_key(&path),
            Err(Error::Io(_))
        ));
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn config_dir_ends_in_ywza() {
        assert!(config_dir().ends_with("ywza"));
    }
}
