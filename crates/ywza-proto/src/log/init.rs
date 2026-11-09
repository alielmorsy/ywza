use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{Builder, Rotation},
};
use tracing_subscriber::{fmt::writer::MakeWriterExt, EnvFilter};

use super::{Format, LogConfig};
use crate::Error;

const OWN_CRATES: [&str; 9] = [
    "ywza_proto",
    "ywza_agent",
    "ywza_cli",
    "ywza_relay",
    "ywza_stream",
    "ywza_desktop",
    "ywza_ocr",
    "ywza_term",
    "iroh_relay",
];

/// A bare level applies to ywza's own crates; everything else stays at most at info, so
/// `debug` does not drown the log in transport noise. Anything with `=` or `,` is used as is.
fn directives(level: &str) -> String {
    if level.contains(['=', ',']) {
        return level.into();
    }
    let others = if matches!(level, "debug" | "trace") { "info" } else { level };
    let own = OWN_CRATES.map(|c| format!("{c}={level}")).join(",");
    format!("{others},{own}")
}

/// Installs the global subscriber: stderr always, plus daily files `<dir>/<prefix>.log.<date>`
/// when `cfg.dir` is set. `RUST_LOG` beats `cfg.level`, which beats `default_level`. Keep the
/// returned guard alive to flush the file.
pub fn init(
    cfg: &LogConfig,
    file_prefix: &str,
    default_level: &str,
) -> Result<Option<WorkerGuard>, Error> {
    let level = if cfg.level.is_empty() { default_level } else { &cfg.level };
    let filter = match EnvFilter::try_from_default_env() {
        Ok(f) => f,
        Err(_) => EnvFilter::try_new(directives(level))
            .map_err(|e| Error::Log(format!("invalid log level {level:?}: {e}")))?,
    };
    let builder = tracing_subscriber::fmt().with_env_filter(filter);
    let (guard, file) = match &cfg.dir {
        Some(dir) => {
            let appender = Builder::new()
                .rotation(Rotation::DAILY)
                .filename_prefix(format!("{file_prefix}.log"))
                .max_log_files(cfg.max_files)
                .build(dir)
                .map_err(|e| Error::Log(format!("opening log dir {}: {e}", dir.display())))?;
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
    use super::directives;

    #[test]
    fn a_bare_level_scopes_debug_to_ywza() {
        let d = directives("debug");
        assert!(d.starts_with("info,") && d.contains("ywza_agent=debug"));
        assert!(directives("warn").starts_with("warn,"));
        assert_eq!(directives("info,iroh=debug"), "info,iroh=debug");
    }
}
