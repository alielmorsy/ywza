use serde::{Deserialize, Serialize};

use crate::ClientMsg;

/// One paired device: its endpoint id, a display name and what it may do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Peer {
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(flatten)]
    pub perms: Permissions,
}

/// What a paired device may do. `settings` is admin: config (including `peers`) and pairing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Permissions {
    pub view: bool,
    pub chat: bool,
    pub control: bool,
    pub tools: bool,
    pub settings: bool,
    /// May start sessions here that use this machine's paired devices.
    pub drive: bool,
}

/// New pairings: everything but `settings` and `drive`.
impl Default for Permissions {
    fn default() -> Self {
        Self {
            settings: false,
            drive: false,
            ..Self::all()
        }
    }
}

impl Permissions {
    pub fn all() -> Self {
        Self {
            view: true,
            chat: true,
            control: true,
            tools: true,
            settings: true,
            drive: true,
        }
    }

    /// The permission `msg` needs that this set lacks, if any.
    pub fn missing(&self, msg: &ClientMsg) -> Option<&'static str> {
        use ClientMsg::*;
        let (ok, name) = match msg {
            Hello { .. } => return None,
            WindowInfo { elements: true, .. } => (self.tools, "tools"),
            Screenshot { .. } | LiveStart { .. } | LiveStop | ListMonitors | WindowInfo { .. } => {
                (self.view, "view")
            }
            Input { .. } | Pull { .. } | Push { .. } | PushEnd { .. } => (self.control, "control"),
            Prompt { device, devices, .. } if device.is_some() || !devices.is_empty() => {
                if self.chat {
                    (self.drive, "drive")
                } else {
                    (false, "chat")
                }
            }
            ListDevices => (self.drive, "drive"),
            Prompt { .. }
            | Approve { .. }
            | Cancel { .. }
            | ListSessions
            | GetSession { .. }
            | ListModels => (self.chat, "chat"),
            ListTools | CallTool { .. } => (self.tools, "tools"),
            GetConfig | SetConfig { .. } | CreatePairing => (self.settings, "settings"),
        };
        (!ok).then_some(name)
    }

    pub fn allows(&self, msg: &ClientMsg) -> bool {
        self.missing(msg).is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, InputEvent, SessionId, Target};

    #[test]
    fn permissions_gate_each_message() {
        use ClientMsg::*;
        let sid = SessionId::default();
        let table: Vec<(ClientMsg, Option<&str>)> = vec![
            (
                Hello {
                    client_name: "x".into(),
                    token: None,
                },
                None,
            ),
            (
                Screenshot {
                    target: Target::Monitor { id: 0 },
                    max_width: None,
                },
                Some("view"),
            ),
            (LiveStart { monitor: 0 }, Some("view")),
            (LiveStop, Some("view")),
            (
                Input {
                    event: InputEvent::Wheel { dx: 0.0, dy: 1.0 },
                },
                Some("control"),
            ),
            (
                Prompt {
                    session: None,
                    text: "t".into(),
                    device: None,
                    devices: Vec::new(),
                    model: None,
                    effort: None,
                    approval: None,
                },
                Some("chat"),
            ),
            (ListDevices, Some("drive")),
            (
                Approve {
                    request: 1,
                    allow: true,
                    remember: false,
                },
                Some("chat"),
            ),
            (ListModels, Some("chat")),
            (ListMonitors, Some("view")),
            (
                WindowInfo {
                    window: "w1".into(),
                    elements: false,
                },
                Some("view"),
            ),
            (
                WindowInfo {
                    window: "w1".into(),
                    elements: true,
                },
                Some("tools"),
            ),
            (Cancel { session: sid }, Some("chat")),
            (ListSessions, Some("chat")),
            (GetSession { session: sid }, Some("chat")),
            (ListTools, Some("tools")),
            (
                CallTool {
                    call: 1,
                    name: "wait".into(),
                    args: serde_json::Value::Null,
                },
                Some("tools"),
            ),
            (GetConfig, Some("settings")),
            (
                SetConfig {
                    config: Config::default(),
                },
                Some("settings"),
            ),
            (CreatePairing, Some("settings")),
        ];
        let none = Permissions {
            view: false,
            chat: false,
            control: false,
            tools: false,
            settings: false,
            drive: false,
        };
        for (msg, need) in table {
            assert_eq!(none.missing(&msg), need, "{msg:?}");
            assert!(Permissions::all().allows(&msg), "{msg:?}");
            assert_eq!(
                Permissions::default().allows(&msg),
                !matches!(need, Some("settings" | "drive")),
                "{msg:?}"
            );
        }
        let on_devices = Prompt {
            session: None,
            text: "t".into(),
            device: None,
            devices: vec!["vm".into()],
            model: None,
            effort: None,
            approval: None,
        };
        assert_eq!(none.missing(&on_devices), Some("chat"));
        let chat_only = Permissions { chat: true, ..none };
        assert_eq!(chat_only.missing(&on_devices), Some("drive"));
        assert!(Permissions::all().allows(&on_devices));
        assert!(!Permissions::default().allows(&on_devices));
    }
}
