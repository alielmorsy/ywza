# Elements, diffs and actions

## Reading `get_elements`

First line: `window w123 "Title" (process) 1280x900 mon 0 snapshot 7`. Then one element per line:

```
[3] edit "Email" = "a@b.com"
[7] radio "Yes" in "Willing to relocate?" selected
[8] radio "No" in "Willing to relocate?"
[9] checkbox "I agree" = off
[12] combobox "Country" = "Egypt" +menu
[15] list "Results" busy
[20] button "Next" disabled
[31] dialog "Open (window w456, get_elements it)"
[40] ocr "Total: 12.50"
```

| Part | Meaning |
|---|---|
| `= "..."` | current value of an edit, combo, slider or spinner |
| `= on` / `= off` | toggle state of a checkbox or switch |
| `selected` | the chosen radio, list option or tab |
| `in "..."` | the group a radio belongs to (usually the question) |
| `busy` | the element is loading |
| `+menu` | has a dropdown besides its main action: `click` with `expand: true` |
| `disabled` | cannot be used yet (often a required field is missing) |
| `(id=...)` | unnamed element, shown by automation id |
| `ocr` | text read from pixels; clicking hits its centre, and one line may span neighbours ("Options ADT"): click those with `text` |

An unnamed clickable may borrow the label it wraps or sits next to. The line is what the tool will act on.

`page_only: true` (browsers) hides tabs, toolbar and bookmarks. Ids of hidden elements stay valid.

## Ids

- Numbers from that window's latest list, on that device only.
- Elements that persist (a Next button) keep their id across steps. Regenerated content gets new ids. Never guess an id you have not seen.
- `stale` means nothing was done; the result carries the fresh list.

## Finding one element

`find_elements` with `window` and any of `text`, `role` (an element kind such as `button`), `near` (an id or a text) and `region` (`top`, `bottom`, `left`, `right`, `center`) returns the best matches in the same line format, best first. Use it for "the Send button near the compose box" in a big window instead of reading the whole list. Its ids are the window's latest list, so `click` and `type_text` take them directly.

## Diffs after an action

```
+[52] button "Submit"      new
-[31]                      gone
~[7] radio "Yes" ... selected   changed
```

`no visible change` means the UI did not react: the click missed or the control needs a different action. Large changes (a page load) collapse to `N removed, M added (sample; get_elements lists all)`: call `get_elements` for the new page.

## Actions

- `click`: `count: 2` double-clicks, `button: right` opens context menus. Instead of `element`: `text` (visible text) or `x`+`y` (screenshot pixels), see Menus below.
- `type_text`: replaces content unless `append: true`. Long or non-Latin text is pasted and the clipboard restored. On an `ocr` box it clicks the box first; where the field cannot be read back, nothing changing in the window is an error, not a silent success. `submit: true` presses Enter once the text is in, and errors if Enter changed nothing. Without `element` it types into the focused control. An error saying the text "does not contain it" means the field differs from what you sent: read the element line before retrying.
- Secrets: type `{{vault:<key>}}` in `type_text` (also batch steps and `term_write`). The agent fills in the value; tool results show the placeholder instead of the secret and screenshots blur it and password fields. The user adds keys with `ywza-agent vault set <key>` on that device. An `unknown vault key` error means the user must add it: ask, never request the secret itself.
- `press_keys`: `"ctrl+a"`, `"tab tab enter"`. With `window` the window comes forward, but its own open dialog keeps focus.
- `scroll`: `dy` > 0 is down, in wheel notches. Read the diff for newly visible elements.
- `long_press`: hold for `ms`.

Background actions: `click`, `type_text` and `scroll` act without moving the user's mouse or taking focus whenever they can, and the result names how: `done (via pattern); ...`, `ok: text written (via window message)`, `done (via takeover); ...`. `pattern` and `window message` touch nothing on screen. `takeover` used the real mouse and keyboard briefly and put the cursor and the previous window back where it was safe (a menu or dialog it opened stays in front).
If an app ignores a background action (`no visible change` after a `pattern` or `window message` result), repeat it with `real: true` to force the real mouse and keyboard.

## Menus and toolbars

Menu bars, Motif/Qt menus and custom toolbars often have no element tree and ignore the keyboard, so their items are `ocr` lines. Menu bar titles are split at the gaps between them, and an `ocr` id stays with its text and place, also when a menu closes and opens again. To find out what a menu holds:

1. `click` the menu title (by id or `text`). The diff lists the items that opened, `+[57] ocr "Show plots"`, with their ids.
2. `click` the item by id. Reopening the same menu later gives the same ids.

When you already know the names, `click` with `text` does it in one call:

- `{"window": "w1", "text": "ADT"}` clicks the word, even inside a merged `ocr` line. It tries element names first, then text read from the pixels, and tolerates OCR misreads.
- `{"window": "w1", "text": "ADT > Sizing > Show plots"}` walks the whole path in one call. Each item is looked for first in what the previous click opened, so a title that also appears elsewhere is not hit by mistake. If an item is missing, the menus it opened are closed with Escape and the error lists what the menu showed: fix the path and retry.
- Text on screen more than once: the error lists the matches; pass `nth`.
- No text at all (an icon): take one `screenshot` of the window and `click` with `x`, `y` in its pixels.

Do not navigate menus with arrow keys and Enter unless `text` fails: the highlighted item is invisible to you.

## Batch

One `get_elements`, then `batch` with every action of the step:

```json
{"window": "w123", "actions": [
  {"tool": "type_text", "element": 3, "text": "Sam"},
  {"tool": "type_text", "element": 5, "text": "Lee"},
  {"tool": "click", "element": 8},
  {"tool": "click", "element": 20}
]}
```

It stops at the first failure and returns one line per action plus the changes. All ids come from the list taken before the batch, so do not batch an action on an element that an earlier action in the same batch creates (an option inside a dropdown it opens).

## Wait

- `wait` + `window` + `text`: returns as soon as an element whose name or value contains the text appears, with its line. OCR-only windows work too (misreads tolerated). Add `gone: true` to wait for a spinner or "Loading" to leave.
- `wait` + `window` + `element`: returns when that element changes or disappears (up to 10 minutes). Use for long jobs.
- `wait` + `window` alone: returns once the window has changed and then stayed still for `quiet_ms` (default 2000), with the changes. Use it for chat apps and LLM answers instead of a fixed pause; it also returns when the app stops to ask something. A blinking caret does not count as a change.
- `wait` + `title`: a window with that title appears (`gone: true`: none is left). `wait` + `term` + `text`: the terminal shows the text.
- `wait` + `ms` alone: plain pause, last resort.

## Screenshots

`screenshot` is the most expensive call. Take one only for images, canvas or visual layout. `marks: true` draws ids on boxes when you cannot tell which element is which. `show_screenshot` puts a picture in the user's chat; you do not receive it.
