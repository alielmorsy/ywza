---
name: ywza
description: Use when the task needs to see or drive a computer's desktop or terminal (windows, UI elements, clicks, typing, screenshots, shells) on this PC or a paired ywza device, or to copy files between them.
---

# ywza

MCP tools that drive desktops and terminals on "local" (this PC) and every paired device.

## Flow

1. `list_devices` to see targets. Pass `device` to any tool to pick one; omitted means local.
2. `list_windows`, then `get_elements` on a window. Act on element ids.
3. Ids (windows, elements) belong to the device that returned them. Never reuse them on another device.
4. Re-read `get_elements` after any action that changes the UI.

## Files

`copy` moves a file or folder, scp style. A path is `device:path` for a paired device, or a plain path for this PC. A destination that is an existing folder receives the source inside it. On a remote device relative paths and `~` are under the user's home.

## Approval

Some actions wait for the user to approve in the ywza app, and may be denied. Do not retry a denied action another way.

## Setup

Needs the `ywza` CLI on PATH and a running agent. Pair a device with `ywza pair <code> --name NAME`.

Without MCP, the same tools run from a shell: `python <plugin>/server/run.py [--device NAME] tools|call <tool> '<json>'`.
