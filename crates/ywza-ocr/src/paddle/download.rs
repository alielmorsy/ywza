use anyhow::{anyhow, bail, Result};
use std::io::Write;
use std::path::{Path, PathBuf};

// Official PaddlePaddle ONNX exports on Hugging Face (inference.onnx + inference.yml;
// the recognition dictionary lives in the yml under PostProcess.character_dict).
const DET_URL: &str =
    "https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/resolve/main/inference.onnx";
const REC_URL: &str =
    "https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/resolve/main/inference.onnx";
const REC_YML_URL: &str =
    "https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/resolve/main/inference.yml";
const DET_SIZE: u64 = 4_826_518;
const REC_SIZE: u64 = 16_534_782;

pub struct Files {
    pub det: PathBuf,
    pub rec: PathBuf,
    pub dict: Vec<String>,
}

/// Make sure the model files exist in `dir`, downloading what is missing.
pub fn ensure(dir: &Path) -> Result<Files> {
    std::fs::create_dir_all(dir)?;
    let (det, rec, yml) = (
        dir.join("det.onnx"),
        dir.join("rec.onnx"),
        dir.join("rec.yml"),
    );
    fetch(&det, DET_URL, Some(DET_SIZE))?;
    fetch(&rec, REC_URL, Some(REC_SIZE))?;
    fetch(&yml, REC_YML_URL, None)?;
    let dict = parse_dict(&std::fs::read_to_string(&yml)?)?;
    Ok(Files { det, rec, dict })
}

/// Download `url` to `path` unless it is already there (with the expected size).
fn fetch(path: &Path, url: &str, size: Option<u64>) -> Result<()> {
    let ok = |len: u64| size.is_none_or(|s| s == len) && len > 0;
    if std::fs::metadata(path).is_ok_and(|m| ok(m.len())) {
        return Ok(());
    }
    let tmp = path.with_extension("part");
    let mut resp = ureq::get(url)
        .call()
        .map_err(|e| anyhow!("download {url}: {e}"))?;
    let mut f = std::fs::File::create(&tmp)?;
    let n = std::io::copy(&mut resp.body_mut().as_reader(), &mut f)?;
    f.flush()?;
    drop(f);
    if !ok(n) {
        bail!("download {url}: got {n} bytes, expected {size:?}");
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// `PostProcess.character_dict` list from inference.yml. Entries are plain
/// scalars or single-quoted (`'` escaped as `''`).
fn parse_dict(yml: &str) -> Result<Vec<String>> {
    let mut lines = yml
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .skip_while(|l| l.trim_end() != "  character_dict:");
    lines.next();
    let dict: Vec<String> = lines
        .map_while(|l| l.strip_prefix("  - "))
        .map(
            |v| match v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
                Some(q) if v.len() >= 2 => q.replace("''", "'"),
                _ => v.to_string(),
            },
        )
        .collect();
    if dict.is_empty() {
        bail!("no character_dict in rec.yml");
    }
    Ok(dict)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dict_parsing() {
        let yml = "PostProcess:\n  name: CTCLabelDecode\n  character_dict:\n  - \u{3000}\n  - A\n  - '0'\n  - '\"'\nOther: 1\n";
        let d = parse_dict(yml).unwrap();
        assert_eq!(d, vec!["\u{3000}", "A", "0", "\""]);
        let d = parse_dict("  character_dict:\r\n  - ''''\r\n  - x\r\n").unwrap();
        assert_eq!(d, vec!["'", "x"]);
    }
}
