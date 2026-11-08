use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{Builder, Rotation},
};
use tracing_subscriber::{fmt::writer::MakeWriterExt, EnvFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LogConfig {
    pub level: String,
    pub dir: Option<PathBuf>,
    pub max_files: usize,
    pub format: Format,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            dir: None,
            max_files: 14,
            format: Format::Text,
        }
    }
}

/// Installs the global subscriber. Keep the returned guard alive to flush the log file.
pub fn init(cfg: &LogConfig) -> Result<Option<WorkerGuard>> {
    let filter = match EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => EnvFilter::try_new(format!("{},iroh_relay={}", cfg.level, cfg.level))
            .with_context(|| format!("invalid log level {:?}", cfg.level))?,
    };
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    let (guard, file) = match &cfg.dir {
        Some(dir) => {
            let appender = Builder::new()
                .rotation(Rotation::DAILY)
                .filename_prefix("ywza-relay.log")
                .max_log_files(cfg.max_files)
                .build(dir)
                .with_context(|| format!("opening log dir {}", dir.display()))?;
            let (w, g) = tracing_appender::non_blocking(appender);
            (Some(g), Some(w))
        }
        None => (None, None),
    };
    match (cfg.format, file) {
        (Format::Text, None) => builder.with_writer(std::io::stderr).init(),
        (Format::Json, None) => builder.json().with_writer(std::io::stderr).init(),
        (Format::Text, Some(f)) => builder
            .with_ansi(false)
            .with_writer(std::io::stderr.and(f))
            .init(),
        (Format::Json, Some(f)) => builder.json().with_writer(std::io::stderr.and(f)).init(),
    }
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let c = LogConfig::default();
        assert_eq!((c.level.as_str(), c.max_files, c.format), ("info", 14, Format::Text));
        assert_eq!(c.dir, None);
    }

    #[test]
    fn parses_a_log_table() {
        #[derive(Deserialize)]
        struct W {
            log: LogConfig,
        }
        let w: W = toml::from_str(
            "[log]\nlevel = \"debug\"\ndir = \"/var/log/ywza\"\nmax_files = 3\nformat = \"json\"",
        )
        .unwrap();
        assert_eq!(w.log.level, "debug");
        assert_eq!(w.log.dir, Some(PathBuf::from("/var/log/ywza")));
        assert_eq!((w.log.max_files, w.log.format), (3, Format::Json));
        let w: W = toml::from_str("[log]").unwrap();
        assert_eq!(w.log, LogConfig::default());
    }
}
