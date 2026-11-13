use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ywza_desktop::{self as desk, Button};

use super::{
    act::{act, Report},
    output::{desk_err, flat},
    util::blocking,
    Ctx, ElementArg, Tool, ToolOutput, WindowArg,
};

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClickButton {
    #[default]
    Left,
    Right,
    Middle,
}

impl From<ClickButton> for Button {
    fn from(b: ClickButton) -> Self {
        match b {
            ClickButton::Left => Button::Left,
            ClickButton::Right => Button::Right,
            ClickButton::Middle => Button::Middle,
        }
    }
}

fn one() -> u32 {
    1
}

/// Click an element. Returns the changes it caused.
#[derive(Deserialize, JsonSchema)]
pub struct Click {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Element id from this window's latest get_elements.
    element: ElementArg,
    #[serde(default)]
    button: ClickButton,
    /// 2 = double-click.
    #[serde(default = "one")]
    count: u32,
    /// Open the element's dropdown or menu instead of its main action (elements marked +menu, e.g. split buttons in toolbars).
    #[serde(default)]
    expand: bool,
}

impl Tool for Click {
    const NAME: &'static str = "click";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let (w, id) = (self.window.0, self.element.0);
            let button: Button = self.button.into();
            let (count, expand) = (self.count.clamp(1, 3), self.expand);
            act(cx, w, Some(id), cx.report(Report::Diff), move |s| {
                if expand {
                    desk::expand(s, id)
                } else {
                    desk::click(s, id, button, count)
                }
            })
            .await
        })
        .await
    }
}

/// Press and hold the left button on an element. Returns the changes it caused.
#[derive(Deserialize, JsonSchema)]
pub struct LongPress {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Element id from this window's latest get_elements.
    element: ElementArg,
    /// Hold time in milliseconds.
    ms: u64,
}

impl Tool for LongPress {
    const NAME: &'static str = "long_press";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let (w, id) = (self.window.0, self.element.0);
            let ms = self.ms.min(10_000);
            act(cx, w, Some(id), cx.report(Report::Diff), move |s| {
                desk::long_press(s, id, ms)
            })
            .await
        })
        .await
    }
}

/// Type text into an element. Focuses the element itself; do not click it first. Replaces the content unless `append`; empty `text` clears the field. Long or non-Latin text is pasted (your clipboard is restored after). Without `element`, types into the window's focused control. Returns 'ok: text written' when verified.
#[derive(Deserialize, JsonSchema)]
pub struct TypeText {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Element id from this window's latest get_elements.
    element: Option<ElementArg>,
    text: String,
    /// false replaces the field's content, true types at its end.
    #[serde(default)]
    append: bool,
    /// Press Enter after the text is verified (sends the message in chat apps; in a file dialog's name box, opens the file). The changes are returned.
    #[serde(default)]
    submit: bool,
}

impl Tool for TypeText {
    const NAME: &'static str = "type_text";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            let id = self.element.map(|e| e.0);
            let Self {
                text,
                append,
                submit,
                ..
            } = self;
            let mode = cx.report(if submit {
                Report::Diff
            } else {
                Report::Verified
            });
            act(cx, w, id, mode, move |s| {
                desk::type_text(s, id, &text, append)?;
                if submit {
                    desk::press_keys(Some(w), "enter")?;
                }
                Ok(())
            })
            .await
        })
        .await
    }
}

/// Press key combos, space-separated: "ctrl+s", "alt+f4", "enter", "ctrl+k ctrl+s". With `window`, it is brought to the front first and the changes are returned; if a dialog of that window (file picker) is in front, the dialog keeps focus and gets the keys.
#[derive(Deserialize, JsonSchema)]
pub struct PressKeys {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: Option<WindowArg>,
    keys: String,
}

impl Tool for PressKeys {
    const NAME: &'static str = "press_keys";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let Self { window, keys } = self;
            match window.map(|w| w.0) {
                Some(w) if cx.has_snap(w) => {
                    act(cx, w, None, cx.report(Report::Diff), move |_| {
                        desk::press_keys(Some(w), &keys)
                    })
                    .await
                }
                w => {
                    blocking(move || desk::press_keys(w, &keys))
                        .await
                        .map_err(desk_err)?;
                    Ok(ToolOutput::ok("pressed"))
                }
            }
        })
        .await
    }
}

/// Scroll an element (or the window centre) by wheel notches: dy > 0 down, dx > 0 right. Returns the changes.
#[derive(Deserialize, JsonSchema)]
pub struct Scroll {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Element id from this window's latest get_elements.
    element: Option<ElementArg>,
    #[serde(default)]
    dx: i32,
    #[serde(default)]
    dy: i32,
}

impl Tool for Scroll {
    const NAME: &'static str = "scroll";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            let id = self.element.map(|e| e.0);
            let (dx, dy) = (self.dx, self.dy);
            act(cx, w, id, cx.report(Report::Diff), move |s| {
                desk::scroll(s, id, dx, dy)
            })
            .await
        })
        .await
    }
}
