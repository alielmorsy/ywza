---
name: ywza
description: Use when a task needs to see or operate a real computer through the ywza MCP tools - desktop apps, an already-open browser, web forms, native dialogs, terminals or AI CLIs running in them, on this PC or a paired device, or copying files between devices.
---

# ywza

The ywza MCP tools read and drive desktops through the accessibility tree, not pixels. Text is cheap, pictures are not: one session spent over a million tokens on screenshots. Read the element list, act on ids and read the diff each action returns.

## Core loop

1. `list_windows` (add `device` for a paired machine; omitted means this PC).
2. `get_elements` on the window. Browsers: `page_only: true`.
3. Act on element ids: `click`, `type_text`, `press_keys`, `scroll`, or several at once with `batch`.
4. Read the returned diff. Call `get_elements` again only when the diff is a summary or you need ids it does not show.
5. Confirm outcomes with `wait` + `text` ("Submitted", "Saved"), not with a screenshot.

An already-open browser is just a window: drive it with ywza. Do not hunt for a debug port, CDP or other browser automation.

## Rules

- Ids belong to the latest `get_elements` of that window on that device. A `stale` result did nothing and carries the fresh list: use it.
- `type_text` focuses the field itself and replaces its content: do not click a field before typing into it. Empty `text` clears it.
- Check the element line after every choice (`selected`, `= on`, the value) before moving on.
- Screenshot only for what the tree cannot show: images, canvas, layout. Never to read text or state.
- An action the user denies stays denied. Do not retry it another way.
- Never reload or navigate back (`alt+left`, F5) on a page holding unsaved input.
- Always pass `window` to `press_keys`: without it the keys go to the focused window, which may not be the one in front.
- Menus: click by `text`, never by keyboard arrows or by the id of an `ocr` line (see references/elements.md).

## Quick reference

| Need | Use |
|---|---|
| Wait for a result, spinner gone | `wait` window + `text` (+ `gone: true`) |
| Wait for a long task | `wait` window + `element` on what will change (Stop button) |
| Find one control in a big window | `find_elements` window + `text`, `role`, `near` or `region` |
| Fill a form step | one `get_elements`, then one `batch` |
| Open a split button's menu | `click` with `expand: true` on a `+menu` element |
| Menu bar, menu, toolbar, OCR-only app | `click` + `text`: `"ADT > Sizing > Show plots"` walks the path |
| Icon with no element or text | `click` + `x`/`y` in pixels of the window's `screenshot` |
| Wait for a chat/LLM answer | `wait` + `window` alone (+ `quiet_ms: 5000`) |
| Wait for a window or terminal output | `wait` + `title`, or `term` + `text` |
| Native file dialog | its own window id from the `dialog` line |
| Type a password or token | `type_text` with `{{vault:<key>}}`; never ask for the secret |
| Text missing from the list | `get_elements` with `accurate_ocr: true` |
| Show the user a window | `show_screenshot` (you do not see it) |
| One-shot shell work | `run_command` |
| Move files between devices | `copy` with `device:path` |
| Repeat a task done before | `list_plans`, then `run_plan` name + `params`; on a stop, `patch_plan` the step and `run_plan` with `from` |
| Save a sequence for later | `save_plan` (steps take `window` and `element` ids; they are stored as anchors) |
| Name an unnamed icon button | `label_element`; later lists show `"name" (learned)` |
| Guard a batch step | `expect: "<element name>"` on the action |

## References

Read the one that matches the task:

- `references/elements.md`: element lines, flags, diffs, ids, OCR, batch and wait details.
- `references/web-forms.md`: browsers, dropdowns, radios, type-ahead, dates, file uploads, sign-in popups, multi-step forms.
- `references/terminals.md`: terminals and the AI coding CLIs that run in them.
- `references/devices.md`: paired devices, file copy, fanout, clipboard, finding which device has an app, approvals.
- `references/watchers.md`: persistent zero-token triggers (windows, elements, text, terminals, schedules).
