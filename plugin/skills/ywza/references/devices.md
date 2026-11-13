# Devices, files and approvals

## Devices

`list_devices` lists "local" and every paired device. Pass `device` to any tool to act there; omit it for this PC. Window, element and terminal ids belong to the device that returned them: keep passing the same `device` for follow-up calls.

## Copying files

`copy` takes `src` and `dst`, scp style:

- `device:path` on a paired device (`vm:/home/vm/report.pdf`), a plain path on this PC (`E:\files`).
- An existing folder as destination receives the source inside it.
- On a remote device, relative paths and `~` are under that user's home.

To upload a file that lives on another device through a web form, `copy` it to the browser's device first, then use the file dialog.

## Approvals

Some actions wait for the user to approve them in the ywza app. A denied action is final: do not retry it with another tool, keys or a shell command. Say what was denied and continue with the rest of the task, or stop if it was essential.
