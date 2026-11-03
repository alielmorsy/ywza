//! PP-OCRv5 mobile detection + recognition through ONNX Runtime (`ort`).
//!
//! Pipeline: resize -> det (DB probability map) -> threshold + connected
//! components -> unclipped boxes -> crop -> rec (CTC) -> greedy decode.

use crate::Recognizer;
use anyhow::Result;

#[cfg(feature = "ocr-paddle")]
mod det;
#[cfg(feature = "ocr-paddle")]
mod download;
#[cfg(feature = "ocr-paddle")]
mod model;
#[cfg(feature = "ocr-paddle")]
mod rec;
#[cfg(feature = "ocr-paddle")]
mod runtime;
#[cfg(feature = "ocr-paddle")]
mod sample;

pub const AVAILABLE: bool = cfg!(feature = "ocr-paddle");

/// Loads the ONNX Runtime library and the models (downloading them on first use) once per
/// process, so a missing library or download failure is reported here.
#[cfg(feature = "ocr-paddle")]
pub fn create() -> Result<Box<dyn Recognizer>> {
    Ok(Box::new(model::Paddle::shared()?))
}

#[cfg(not(feature = "ocr-paddle"))]
pub fn create() -> Result<Box<dyn Recognizer>> {
    anyhow::bail!("ywza-ocr was built without the `ocr-paddle` feature")
}
