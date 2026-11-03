//! Windows.Media.Ocr engine (Windows only, feature `ocr-windows`).

use crate::Recognizer;
use anyhow::Result;

#[cfg(all(windows, feature = "ocr-windows"))]
mod ocr;

pub const AVAILABLE: bool = cfg!(all(windows, feature = "ocr-windows"));

#[cfg(all(windows, feature = "ocr-windows"))]
pub fn create() -> Result<Box<dyn Recognizer>> {
    Ok(Box::new(ocr::WindowsOcr))
}

#[cfg(not(all(windows, feature = "ocr-windows")))]
pub fn create() -> Result<Box<dyn Recognizer>> {
    anyhow::bail!("the Windows OCR engine needs Windows and the `ocr-windows` feature")
}
