use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ywza_desktop::{self as desk, Button};

use super::{
    act::{act, Report},
    find,
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

/// Click an element, visible text, a menu path or a point. Returns the changes it caused. Give exactly one of `element`, `text`, or `x`+`y`.
#[derive(Deserialize, JsonSchema)]
pub struct Click {
    /// Window id from list_windows/launch, e.g. "w3412106".
    window: WindowArg,
    /// Element id from this window's latest get_elements.
    element: Option<ElementArg>,
    /// Click visible text instead: an element with that name, else text read from the pixels (one word of a merged ocr line such as "Options ADT" works). Tolerates OCR misreads; no element ids needed. "ADT > Sizing > Show plots" walks a menu path: each item is looked for in what the previous click opened. Use this for menu bars, menus and toolbars.
    text: Option<String>,
    /// With `text`: which match, 1 = first in reading order, when the text is on screen more than once.
    nth: Option<u32>,
    /// With `y`: a point in pixels of this window's `screenshot` image (as returned, scaled or not), for icons with no element or text.
    x: Option<i32>,
    y: Option<i32>,
    #[serde(default)]
    button: ClickButton,
    /// 2 = double-click.
    #[serde(default = "one")]
    count: u32,
    /// Open the element's dropdown or menu instead of its main action (elements marked +menu, e.g. split buttons in toolbars).
    #[serde(default)]
    expand: bool,
    /// Use the real mouse and keyboard instead of acting in the background (for apps that ignore background actions).
    #[serde(default)]
    real: bool,
}

impl Tool for Click {
    const NAME: &'static str = "click";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            let button: Button = self.button.into();
            let (count, expand, real) = (self.count.clamp(1, 3), self.expand, self.real);
            let id = match (self.element, self.text, self.x.zip(self.y)) {
                (Some(e), None, None) => e.0,
                (None, Some(t), None) => {
                    return find::click_text(cx, w, &t, self.nth, button, count, real).await
                }
                (None, None, Some((x, y))) => {
                    return find::click_point(cx, w, x, y, button, count).await
                }
                _ => {
                    return Err(ToolOutput::err(
                        "give exactly one of `element`, `text`, or `x` and `y`",
                    ))
                }
            };
            act(cx, w, Some(id), cx.report(Report::Diff), move |s| {
                let tier = if expand {
                    desk::expand(s, id)
                } else {
                    desk::click(s, id, button, count, real)
                };
                tier.map(Some)
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
                desk::long_press(s, id, ms).map(Some)
            })
            .await
        })
        .await
    }
}

/// Type text into an element. Focuses the element itself (an ocr box is clicked first); do not click it first. Replaces the content unless `append`; empty `text` clears the field. Long or non-Latin text is pasted (your clipboard is restored after). Without `element`, types into the window's focused control. Returns 'ok: text written' when verified; where it cannot read the field back it returns the visible changes, and an error if nothing changed.
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
    /// Press Enter after the text is in (sends the message in chat apps; in a file dialog's name box, opens the file). The changes are returned; an error if Enter changed nothing.
    #[serde(default)]
    submit: bool,
    /// Use the real mouse and keyboard instead of acting in the background (for apps that ignore background actions).
    #[serde(default)]
    real: bool,
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
                real,
                ..
            } = self;
            // OCR boxes have no object to focus or read back: click it, then type.
            let ocr_box = id
                .and_then(|id| cx.snap(w)?.get(id).map(|e| e.kind == "ocr"))
                .unwrap_or(false);
            let mode = cx.report(if id.is_some() && !ocr_box {
                Report::Verified
            } else {
                Report::Changed(
                    "nothing changed in the window: the text went nowhere. Click the field (click text= its label or placeholder), then type_text without element.",
                )
            });
            let typed = act(cx, w, id, mode, move |s| match id {
                Some(id) if ocr_box => {
                    desk::click(s, id, Button::Left, 1, real)?;
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    let keys = match (append, text.is_empty()) {
                        (true, _) => "end",
                        (false, true) => "ctrl+a delete",
                        (false, false) => "ctrl+a",
                    };
                    desk::press_keys(Some(w), keys)?;
                    desk::type_text(s, None, &text, true, real).map(Some)
                }
                _ => desk::type_text(s, id, &text, append, real).map(Some),
            })
            .await?;
            if !submit {
                return Ok(typed);
            }
            // Enter on its own, once the text is in: a completion popup opened by the typing
            // may take the first Enter, which the error below then reports.
            act(cx, w, None, cx.report(Report::Changed(
                "the text is in, but Enter changed nothing (not sent?). Check with a screenshot; press_keys enter or click the Send button.",
            )), move |_| desk::press_keys(Some(w), "enter").map(|_| None))
            .await
        })
        .await
    }
}

/// Press key combos, space-separated: "ctrl+s", "alt+f4", "enter", "ctrl+k ctrl+s". With `window`, it is brought to the front first and the changes are returned; if a dialog of that window (file picker) is in front, the dialog keeps focus and gets the keys. Without `window` the keys go to whichever window has the focus (named in the answer), which need not be the one in front: pass `window`.
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
                        desk::press_keys(Some(w), &keys).map(|_| None)
                    })
                    .await
                }
                Some(w) => {
                    blocking(move || desk::press_keys(Some(w), &keys))
                        .await
                        .map_err(desk_err)?;
                    Ok(ToolOutput::ok("pressed"))
                }
                None => {
                    if desk::ocr_norm(&keys).contains("alt+f4") {
                        return Err(ToolOutput::err(
                            "alt+f4 without `window` closes whatever has the focus; pass `window`",
                        ));
                    }
                    let to = blocking(move || {
                        let focused = desk::list_windows()
                            .ok()
                            .and_then(|l| l.into_iter().find(|w| w.focused));
                        desk::press_keys(None, &keys).map(|_| focused)
                    })
                    .await
                    .map_err(desk_err)?;
                    Ok(ToolOutput::ok(match to {
                        Some(f) => format!("pressed; the keys went to {f}"),
                        None => "pressed".into(),
                    }))
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
    /// Use the real mouse and keyboard instead of acting in the background (for apps that ignore background actions).
    #[serde(default)]
    real: bool,
}

impl Tool for Scroll {
    const NAME: &'static str = "scroll";

    async fn run(self, cx: &Ctx) -> ToolOutput {
        flat(async {
            let w = self.window.0;
            let id = self.element.map(|e| e.0);
            let (dx, dy, real) = (self.dx, self.dy, self.real);
            act(cx, w, id, cx.report(Report::Diff), move |s| {
                desk::scroll(s, id, dx, dy, real).map(Some)
            })
            .await
        })
        .await
    }
}
