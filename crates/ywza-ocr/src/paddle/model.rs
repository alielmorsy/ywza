use super::{det, download, rec, runtime};
use crate::models_dir::models_dir;
use crate::{Image, Recognizer, TextLine};
use anyhow::{Context, Result};
use ort::{session::Session, value::Tensor};
use std::path::Path;
use std::sync::{Arc, Mutex};

struct Models {
    det: Session,
    rec: Session,
    dict: Vec<String>,
}

/// Cheap handle to the process-wide loaded models.
#[derive(Clone)]
pub struct Paddle(Arc<Mutex<Models>>);

static SHARED: Mutex<Option<Paddle>> = Mutex::new(None);

impl Paddle {
    pub fn shared() -> Result<Self> {
        let mut slot = SHARED.lock().unwrap();
        if let Some(p) = slot.as_ref() {
            return Ok(p.clone());
        }
        let p = Paddle(Arc::new(Mutex::new(load(&models_dir().join("ppocr-v5"))?)));
        *slot = Some(p.clone());
        Ok(p)
    }
}

impl Recognizer for Paddle {
    fn recognize(&self, img: &Image) -> Result<Vec<TextLine>> {
        let mut guard = self.0.lock().unwrap();
        let m = &mut *guard;
        let boxes = det::detect(&mut m.det, img).context("PP-OCR detection")?;
        let mut out = Vec::new();
        for b in boxes {
            if let Some(line) =
                rec::recognize_box(&mut m.rec, &m.dict, img, b).context("PP-OCR recognition")?
            {
                out.push(line);
            }
        }
        Ok(out)
    }
}

/// The runtime comes first so a missing library fails before any download.
fn load(dir: &Path) -> Result<Models> {
    let env = runtime::environment()?.with_name("ywza-ocr").build()?;
    let files = download::ensure(dir)?;
    let session =
        |p: &Path| -> Result<Session> { Ok(Session::builder(&env)?.commit_from_file(p)?) };
    Ok(Models {
        det: session(&files.det)?,
        rec: session(&files.rec)?,
        dict: files.dict,
    })
}

/// Run a single-input model; returns (shape, data) of output 0.
pub(super) fn run(
    s: &mut Session,
    shape: [usize; 4],
    data: Vec<f32>,
) -> Result<(Vec<i64>, Vec<f32>)> {
    let name = s.inputs()[0].name().to_string();
    let out = s.run(ort::inputs![name => Tensor::from_array((shape, data))?])?;
    let (sh, d) = out[0].try_extract_tensor::<f32>()?;
    Ok((sh.to_vec(), d.to_vec()))
}
