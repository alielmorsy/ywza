//! ywza-ocr: swappable OCR engines, compiled in by feature and chosen at runtime.
//!
//! `engine(kind)` builds a [`Recognizer`]; `recognize(image, kind)` adds validation and a
//! result cache. `Engine::Auto` uses the Windows engine and escalates to Paddle when it
//! fails or finds nothing in a non-blank image.

mod auto;
mod cache;
mod engine;
mod models_dir;
mod paddle;
mod select;
mod tile;
mod types;
mod windows;

pub use engine::{Engine, Recognizer};
pub use models_dir::set_models_dir;
pub use select::engine;
pub use types::{Image, Rect, TextLine, Word};

use anyhow::Result;

/// Blocking. Call from `spawn_blocking`. Paddle may download models on first use.
pub fn recognize(img: &Image, kind: Engine) -> Result<Vec<TextLine>> {
    img.check()?;
    if img.width == 0 || img.height == 0 {
        return Ok(vec![]);
    }
    let key = cache::key(img, kind);
    if let Some(hit) = cache::get(key) {
        return Ok(hit);
    }
    let out = engine(kind)?.recognize(img)?;
    cache::put(key, out.clone());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        let bad = Image {
            width: 2,
            height: 2,
            rgba: vec![0; 3],
        };
        assert!(recognize(&bad, Engine::Auto).is_err());
        let empty = Image {
            width: 0,
            height: 0,
            rgba: vec![],
        };
        assert!(recognize(&empty, Engine::Auto).unwrap().is_empty());
    }
}
