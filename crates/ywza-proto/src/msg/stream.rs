use serde::{Deserialize, Serialize};

/// What a screenshot points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Target {
    Monitor { id: u32 },
    Window { id: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Human input from the live view. Coordinates are normalized to 0..1 over the monitor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputEvent {
    MouseMove {
        x: f32,
        y: f32,
    },
    MouseButton {
        button: MouseButton,
        down: bool,
    },
    Wheel {
        dx: f32,
        dy: f32,
    },
    /// `key` uses the same names as the `press_keys` tool ("a", "enter", "ctrl").
    Key {
        key: String,
        down: bool,
    },
    /// Text for whatever has keyboard focus. More than one character is pasted, so bursts
    /// arrive whole and editors do not auto-indent or auto-close it a second time.
    Text {
        text: String,
    },
}

/// Live view: starts each unidirectional stream (as one [`crate::write_frame`] frame),
/// followed by the frame's Annex-B H.264 bytes up to the end of the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameHeader {
    pub monitor: u32,
    pub seq: u64,
    pub pts_us: u64,
    pub keyframe: bool,
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg::testutil::check;
    use serde_json::json;

    #[test]
    fn input_events() {
        use InputEvent::*;
        check(
            MouseMove { x: 0.5, y: 0.25 },
            json!({"type":"mouse_move","x":0.5,"y":0.25}),
        );
        check(
            MouseButton {
                button: crate::MouseButton::Right,
                down: false,
            },
            json!({"type":"mouse_button","button":"right","down":false}),
        );
        check(
            Wheel { dx: 0.0, dy: -1.0 },
            json!({"type":"wheel","dx":0.0,"dy":-1.0}),
        );
        check(
            Key {
                key: "enter".into(),
                down: false,
            },
            json!({"type":"key","key":"enter","down":false}),
        );
        check(
            Text {
                text: "fn main() {}".into(),
            },
            json!({"type":"text","text":"fn main() {}"}),
        );
    }
}
