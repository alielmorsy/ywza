# Watchers

A watcher is a trigger that stays on the device and costs no tokens until it fires. Use one instead of a long `wait` or a polling loop when the user wants to be told about something later ("tell me when the build window closes", "every morning at 9 check my inbox"). The agent checks it about once a second and fires when its condition turns from false to true, not while it stays true. A condition that already holds when the watcher is created or turned on does not fire.

Pass `device` to create the watcher on a paired device; it then lives and runs there.

## Conditions (`on`)

| `on` | Needs | Fires when |
|---|---|---|
| `window_appears` | `title` | a window whose title contains it (any case) shows up |
| `window_closes` | `title` | such a window was open and none is left |
| `element_appears` | `title`, `text` | an element whose name or value contains `text` appears in that window |
| `element_changes` | `title`, `text` | the element whose name contains `text` changes name, value or state |
| `text` | `title`, `text`, optional `ocr` | `text` shows in the window; with `ocr: true` also in the pixels (slower, tolerates misreads) |
| `term_output` | `term`, `text` | the terminal's recent output contains `text` |
| `term_exit` | `term`, optional `code` | a command in the terminal finished with that exit code (default: any non-zero), or the terminal's process ended |
| `schedule` | `every_min` or `at` | every N minutes, or daily at `at` ("HH:MM", the device's local time) |

Window ids do not survive restarts, so windows are matched by title. Terminal ids die with the agent: a `term_*` watcher only makes sense for a terminal that is alive now, and stops matching after a restart. `term_exit` needs shell integration (a bare pwsh, powershell, bash or zsh started with `term_start`); a command shorter than a second that repeats the same exit code can be missed.

## Actions (`then`)

- `notify` (default): shows `message`, or a description of what happened, to the user's devices.
- `prompt`: starts a new AI session with `prompt` (and a note saying which watcher fired and why), on the paired device `run_on` if given. This spends tokens each time it fires, so keep the condition narrow.

## Limits

- At most one fire per `min_gap_s` (default 60).
- More than `max_per_hour` fires in an hour (default 12) turns the watcher off and notifies the user. Turn it back on with `watcher_set` once the cause is fixed.
- 50 watchers per device. `watcher_create` asks the user to approve.

## Tools

- `watcher_create`: the arguments above plus `name`, `min_gap_s`, `max_per_hour`. Returns the new id.
- `watcher_list`: one line per watcher with id, on/off, name, condition, action and fire count.
- `watcher_set` `{id, enabled}`: turn one on or off.
- `watcher_delete` `{id}`.

## Examples

Tell the user when a build window closes:

```json
{"on": "window_closes", "title": "Build", "message": "The build window closed"}
```

Look into any failing command in a terminal you started:

```json
{"on": "term_exit", "term": "t2", "then": "prompt", "prompt": "The last command in t2 failed. Read the terminal and say why."}
```

A daily check, at most once a day:

```json
{"on": "schedule", "at": "09:00", "then": "prompt", "prompt": "Open the mail app and summarize anything urgent."}
```

The same from a shell: `ywza watch add --on window_closes --title Build`, `ywza watch list`, `ywza watch off <id>`, `ywza watch rm <id>`. Add `--device vm` to manage another machine's watchers.
