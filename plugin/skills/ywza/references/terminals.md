# Terminals

Terminal ids look like `t2`. Window ids (`w...`) are separate.

| Need | Use |
|---|---|
| One-shot command, output back | `run_command` (not interactive) |
| See terminals you can use | `term_list`: ones you started and existing consoles you can attach to |
| Interactive program you own | `term_start` with a command (`pwsh`, `claude`) |
| Run a command and get its output and exit code | `term_run` in a `pwsh`, `powershell`, `bash` or `zsh` you started with `term_start` |
| Read the screen | `term_read` |
| Type a line | `term_write` (Enter added unless `enter: false`) |
| Keys | `term_keys`: `"esc"`, `"ctrl+c"`, `"up up enter"` |

Shells started bare with `term_start` (`pwsh`, `powershell`, `bash`, `zsh`) report their cwd and last exit code: `term_list` and `term_read` show them, and `term_run` waits for the command to finish instead of polling. On timeout it returns the output so far and the command keeps running. Other programs get "no shell integration": use `term_write` and `term_read` there.

Always `term_read` before writing: the program may be waiting for something other than what you expect.

## AI coding CLIs (Claude Code, Codex, agy)

- Find the terminal with `term_list`, then `term_read` it.
- Their approval prompts ("Do you want to proceed?", Yes/No menus) authorise real actions. Read what the prompt would run first. Confirm only when the user's request clearly covers it. Otherwise leave the prompt pending, send nothing, and ask the user what to do.
- Menus are driven with `term_keys` (`"down enter"`), not by typing the option text.
- After sending a prompt, poll with `term_read` at sensible intervals, not in a tight loop.
