//! Logging settings shared by every binary (`[log]` in a config file). With the `log` feature,
//! `init` installs the subscriber.

#[cfg(feature = "log")]
mod init;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(feature = "log")]
pub use init::init;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Text,
    Json,
}

/// An empty `level` means the caller's default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LogConfig {
    /// `error`, `warn`, `info`, `debug`, `trace`, or a full `RUST_LOG` filter.
    pub level: String,
    /// Daily log files go here, besides stderr.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<PathBuf>,
    /// One file appended to instead of daily files in `dir`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    pub max_files: usize,
    pub format: Format,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: String::new(),
            dir: None,
            file: None,
            max_files: 14,
            format: Format::Text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct W {
        log: LogConfig,
    }

    #[test]
    fn defaults() {
        let c = LogConfig::default();
        assert_eq!(
            (c.level.as_str(), c.max_files, c.format),
            ("", 14, Format::Text)
        );
        assert_eq!((c.dir, c.file), (None, None));
    }

    #[test]
    fn parses_a_log_table() {
        let w: W = toml::from_str(
            "[log]\nlevel = \"debug\"\ndir = \"/var/log/ywza\"\nfile = \"/tmp/a.log\"\nmax_files = 3\nformat = \"json\"",
        )
        .unwrap();
        assert_eq!(w.log.level, "debug");
        assert_eq!(w.log.dir, Some(PathBuf::from("/var/log/ywza")));
        assert_eq!(w.log.file, Some(PathBuf::from("/tmp/a.log")));
        assert_eq!((w.log.max_files, w.log.format), (3, Format::Json));
        let w: W = toml::from_str("[log]").unwrap();
        assert_eq!(w.log, LogConfig::default());
    }
}
