use std::path::{Path, PathBuf};
use std::sync::Mutex;

static MODELS_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Where Paddle models are downloaded to (`<config_dir>/models`). Default:
/// `$YWZA_MODELS_DIR`, else `%LOCALAPPDATA%\ywza\models` / `~/.local/share/ywza/models`.
pub fn set_models_dir(dir: impl AsRef<Path>) {
    *MODELS_DIR.lock().unwrap() = Some(dir.as_ref().to_path_buf());
}

#[cfg_attr(not(feature = "ocr-paddle"), allow(dead_code))]
pub(crate) fn models_dir() -> PathBuf {
    if let Some(d) = MODELS_DIR.lock().unwrap().clone() {
        return d;
    }
    if let Some(d) = std::env::var_os("YWZA_MODELS_DIR") {
        return d.into();
    }
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(std::env::temp_dir);
    base.join("ywza").join("models")
}
