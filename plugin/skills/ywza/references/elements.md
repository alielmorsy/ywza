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
| `ocr` | text read from pixels; clicking hits its centre |

An unnamed clickable may borrow the label it wraps or sits next to. The line is what the tool will act on.

`page_only: true` (browsers) hides tabs, toolbar and bookmarks. Ids of hidden elements stay valid.

## Ids

- Numbers from that window's latest list, on that device only.
- Elements that persist (a Next button) keep their id across steps. Regenerated content gets new ids. Never guess an id you have not seen.
- `stale` means nothing was done; the result carries the fresh list.

## Diffs after an action

```
+[52] button "Submit"      new
-[31]                      gone
~[7] radio "Yes" ... selected   changed
```

`no visible change` means the UI did not react: the click missed or the control needs a different action. Large changes (a page load) collapse to `N removed, M added (sample; get_elements lists all)`: call `get_elements` for the new page.

## Actions

- `click`: `count: 2` double-clicks, `button: right` opens context menus.
- `type_text`: replaces content unless `append: true`. Long or non-Latin text is pasted and the clipboard restored. `submit: true` presses Enter after verifying. Without `element` it types into the focused control. An error saying the text "does not contain it" means the field differs from what you sent: read the element line before retrying.
- `press_keys`: `"ctrl+a"`, `"tab tab enter"`. With `window` the window comes forward, but its own open dialog keeps focus.
- `scroll`: `dy` > 0 is down, in wheel notches. Read the diff for newly visible elements.
- `long_press`: hold for `ms`.

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

- `wait` + `window` + `text`: returns as soon as an element whose name or value contains the text appears, with its line. Add `gone: true` to wait for a spinner or "Loading" to leave.
- `wait` + `window` + `element`: returns when that element changes or disappears (up to 10 minutes). Use for long jobs.
- `wait` + `ms` alone: plain pause, last resort.

## Screenshots

`screenshot` is the most expensive call. Take one only for images, canvas or visual layout. `marks: true` draws ids on boxes when you cannot tell which element is which. `show_screenshot` puts a picture in the user's chat; you do not receive it.
