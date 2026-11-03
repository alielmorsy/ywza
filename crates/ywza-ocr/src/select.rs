use crate::auto::Auto;
use crate::{paddle, windows, Engine, Recognizer};
use anyhow::Result;

/// Builds the engine, or fails with a clear error when it is not compiled in or cannot
/// start (for Paddle: missing models download, missing onnxruntime library).
pub fn engine(kind: Engine) -> Result<Box<dyn Recognizer>> {
    match kind {
        Engine::Windows => windows::create(),
        Engine::Paddle => paddle::create(),
        Engine::Auto if windows::AVAILABLE => Ok(Box::new(Auto {
            first: windows::create()?,
            fallback: paddle::AVAILABLE.then_some(paddle::create as _),
        })),
        Engine::Auto => paddle::create(),
    }
}
