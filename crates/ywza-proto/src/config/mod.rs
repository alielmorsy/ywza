mod approval;
mod desktop;
mod llm;
mod ocr;
mod peer;
mod stream;

use serde::{Deserialize, Serialize};

use crate::LogConfig;

pub use approval::{glob, Action, Approval, ApprovalMode, Rule};
pub use desktop::Desktop;
pub use llm::Llm;
pub use ocr::{Ocr, OcrEngine};
pub use peer::{Peer, Permissions};
pub use stream::Stream;

/// `<config_dir>/config.toml`. Every field has a default, so a partial file is valid.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Legacy list of endpoint ids, read only: the agent moves them into `peers` with every
    /// permission and never writes this key back.
    #[serde(skip_serializing)]
    pub trusted_peers: Vec<String>,
    /// Paired devices allowed to connect, each with its own permissions (`[[peers]]`).
    pub peers: Vec<Peer>,
    /// Custom relay (e.g. https://relay.example.com). Unset uses the public n0 relays. Every device must use the same one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay_url: Option<String>,
    pub llm: Llm,
    /// Which tool calls wait for the user's answer (`[[approval.rules]]`).
    pub approval: Approval,
    pub ocr: Ocr,
    pub desktop: Desktop,
    pub stream: Stream,
    pub log: LogConfig,
}

impl Config {
    /// Copy with `llm.api_key` blanked, for sending to clients.
    pub fn masked(&self) -> Self {
        let mut c = self.clone();
        c.llm.api_key.clear();
        c
    }

    pub fn peer(&self, id: &str) -> Option<&Peer> {
        self.peers.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_toml_is_the_documented_defaults() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.llm.max_steps, 60);
        assert!(c.llm.vision);
        assert_eq!(c.ocr.engine, OcrEngine::Auto);
        assert_eq!(c.desktop.idle_before_input_ms, 1000);
    }

    #[test]
    fn parses_the_design_doc_shape() {
        let c: Config = toml::from_str(
            r#"
            [[peers]]
            id = "abc"
            name = "phone"
            control = false
            [llm]
            api_key = "sk-secret"
            model = "m"
            [ocr]
            engine = "paddle"
            [stream]
            fps = 15
            "#,
        )
        .unwrap();
        let p = c.peer("abc").unwrap();
        assert_eq!(p.name, "phone");
        assert!(!p.perms.control && p.perms.view && !p.perms.settings);
        assert_eq!((c.stream.fps, c.stream.bitrate_kbps), (15, 0));
        assert!(!c.stream.cursor);
        assert_eq!(c.llm.model, "m");
        assert_eq!(c.llm.base_url, "https://api.openai.com/v1");
        assert_eq!(c.ocr.engine, OcrEngine::Paddle);
    }

    #[test]
    fn toml_round_trip() {
        let mut c = Config::default();
        c.peers.push(Peer {
            id: "abc".into(),
            name: "phone".into(),
            perms: Permissions {
                chat: false,
                ..Permissions::default()
            },
        });
        c.stream.bitrate_kbps = 4000;
        assert_eq!(
            toml::from_str::<Config>(&toml::to_string(&c).unwrap()).unwrap(),
            c
        );
    }

    #[test]
    fn legacy_trusted_peers_are_read_but_never_written() {
        let c: Config = toml::from_str(r#"trusted_peers = ["abc"]"#).unwrap();
        assert_eq!(c.trusted_peers, ["abc"]);
        assert!(!toml::to_string(&c).unwrap().contains("trusted_peers"));
    }

    #[test]
    fn masked_blanks_only_the_key() {
        let mut c = Config::default();
        c.llm.api_key = "sk-secret".into();
        let m = c.masked();
        assert_eq!(m.llm.api_key, "");
        assert_eq!(c.llm.api_key, "sk-secret");
        assert_eq!(m.llm.model, c.llm.model);
    }
}
