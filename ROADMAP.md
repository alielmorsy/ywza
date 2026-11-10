# Roadmap

Where ywza is going. Nothing here is promised for a date, and the order can change.

## Next

- **Your own relay.** Paired devices reach each other through public relays today. You will be able to run your own relay and point your devices at it, so none of your traffic depends on infrastructure you don't control.
- **File transfer.** Copy files and folders between this PC and any paired device, or between two paired devices, from the command line and from MCP.
- **Better logs.** Leveled logging in the agent and the CLI, with a log file option, so problems on a remote machine are easier to track down.

- **Live view with full control.** Watch a PC's screen live and take over with your own mouse and keyboard, or touch on a phone, from the desktop app or the mobile app. You can jump in whenever the agent gets stuck, then hand control back.

## Apps

The agent and the command line are all there is right now. Two apps are planned so you don't have to live in a terminal.

### Desktop app

A window for Windows and Linux that sits on top of the agent.

- Start, stop and see the status of the local agent
- Pair devices, see who is paired, and remove them
- Pick your model and set its endpoint and key
- Chat with the agent on this PC or on any paired device, and watch what it does
- See past sessions and what the agent did in each
- Live view of any paired PC with full mouse and keyboard control

### Mobile app

Drive your PC from your phone, from anywhere.

- Pair with a PC by typing its 6-digit code
- Send instructions to any paired PC and follow the progress
- Watch the screen live while the agent works, and take over with full touch control when you need to
- Stop a running task right away

Both apps will talk to the same agent as the CLI, so everything they can do will also work from the command line and MCP.
