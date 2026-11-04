use ywza_desktop::{Button, Rect};
use ywza_proto::{InputEvent, MouseButton};

/// Normalized (0..1) monitor coordinates to virtual-screen pixels, clamped to the monitor.
pub fn to_pixels(r: Rect, x: f32, y: f32) -> (i32, i32) {
    let map = |v: f32, origin: i32, span: i32| {
        origin + (v.clamp(0.0, 1.0) * (span - 1).max(0) as f32).round() as i32
    };
    (map(x, r.x, r.w), map(y, r.y, r.h))
}

pub fn inject(rect: Rect, ev: &InputEvent) -> anyhow::Result<()> {
    match ev {
        InputEvent::MouseMove { x, y } => {
            let (x, y) = to_pixels(rect, *x, *y);
            ywza_desktop::live_mouse_move(x, y)?
        }
        InputEvent::MouseButton { button, down } => {
            let b = match button {
                MouseButton::Left => Button::Left,
                MouseButton::Right => Button::Right,
                MouseButton::Middle => Button::Middle,
            };
            ywza_desktop::live_mouse_button(b, *down)?
        }
        InputEvent::Wheel { dx, dy } => ywza_desktop::live_wheel(*dx, *dy)?,
        InputEvent::Key { key, down } => ywza_desktop::live_key(key, *down)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_coords_map_onto_the_monitor() {
        let r = Rect::new(-1920, 100, 1920, 1080);
        assert_eq!(to_pixels(r, 0.0, 0.0), (-1920, 100));
        assert_eq!(to_pixels(r, 1.0, 1.0), (-1, 1179));
        assert_eq!(to_pixels(r, 0.5, 0.5), (-960, 640));
        assert_eq!(to_pixels(r, -3.0, 9.0), (-1920, 1179), "clamped");
    }
}
