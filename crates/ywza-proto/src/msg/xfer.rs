use serde::{Deserialize, Serialize};

/// First frame of every file-transfer stream; a file's raw bytes follow until the stream ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHeader {
    pub xfer: u64,
    /// Relative to the copied root, `/`-separated; empty for the root itself.
    pub rel: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dir: bool,
    #[serde(default)]
    pub size: u64,
    /// Unix permission bits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtime_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::testutil::check;
    use serde_json::json;

    #[test]
    fn file_header() {
        check(
            FileHeader {
                xfer: 1,
                rel: "a/b.txt".into(),
                dir: false,
                size: 3,
                mode: Some(0o644),
                mtime_ms: Some(5),
            },
            json!({"xfer":1,"rel":"a/b.txt","size":3,"mode":420,"mtime_ms":5}),
        );
        check(
            FileHeader {
                xfer: 1,
                rel: String::new(),
                dir: true,
                size: 0,
                mode: None,
                mtime_ms: None,
            },
            json!({"xfer":1,"rel":"","dir":true,"size":0}),
        );
    }
}
