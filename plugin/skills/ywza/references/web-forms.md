# Browsers and web forms

An open browser (Chrome, Edge, Brave, Firefox) is a window like any other. Use `get_elements` with `page_only: true` on it.

## Per step of a multi-step form

1. `get_elements` (`page_only: true`).
2. Fill everything in one `batch`.
3. Read the diff: every radio `selected`, every checkbox `= on`, every combo showing its value.
4. Fix any field that did not take, then click Next / Save and continue.
5. `wait` with `text` for the next step's heading.

Required fields left empty usually keep Next `disabled` or add an error line to the diff. Read it before clicking again.

## Controls

| Control | How |
|---|---|
| Text field | `type_text`. To clear, empty `text`. |
| Radio | click the line with the option's name. Check `in "..."` matches the question: unnamed radios in a row belong to different questions. |
| Checkbox | click, then confirm `= on`. |
| Native select / combobox | `click` it, then click the option in the diff. Confirm the combo's `= "value"`. |
| Custom dropdown (list of options) | click to open, click the option's line, confirm the field's value. If clicking a named list item does nothing, click the clickable next to it. |
| Type-ahead search box | `type_text` a short query, then `wait` + `text` for an option, click it. Do not press Enter to pick. Keep the option click out of the batch that typed the query: the option does not exist yet. |
| Date with spin parts (dd/mm/yyyy) | `type_text` on the first part with all digits and no separators (`01092027`); the field moves through its parts. Spin parts often show no value: if the list cannot confirm the date, this is a fair case for one screenshot. |
| Long text (cover letter) | `type_text`; it is pasted. The value in the list may be shortened: check its start and end. An error saying the text "does not contain it" means a real mismatch. |

## Loading that never ends

If a list stays `busy` or shows "Loading" after a `wait` with `gone: true`, close it and open it once more. If it is still loading, the site session has probably expired. Do not retry more than twice. Never reload a half-filled form: the session and every entry are lost. Tell the user, or save a draft first if the site offers one.

## File uploads (native file dialog)

1. Click the site's upload button. The diff shows a line like `dialog "Open (window w456, get_elements it)"`.
2. `get_elements` on that window id.
3. Either type the full path into the "File name" edit with `submit: true`, or click the file in the list and click Open.
4. `wait` on the browser window for the file name to appear on the page.

## Sign-in popups (Google, Microsoft)

The popup is a new window: `list_windows`, find it, `get_elements` it. After consent it closes and the original window changes. Read the original's diff or `wait` there.

## Confirming a submit

`wait` window + `text` with the site's confirmation ("Application submitted", "sent", "Thank you"). The returned line is your proof. If the site shows no clear message, `wait` with the Submit button's name and `gone: true`, then `get_elements` and read the page. Do not navigate back from the confirmation.

## Layout problems

If a button you expect is missing, `scroll` the modal or page and read the diff. If a modal clips its own buttons, the browser zoom is likely high: `press_keys` `ctrl+0` resets it.

## Tabs and the address bar

`ctrl+t` new tab, `ctrl+w` close, `ctrl+l` then `type_text` with `submit: true` to go to a URL. The address bar value may be shortened in the list.
