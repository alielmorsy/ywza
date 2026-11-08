<p align="center">
  <img src="assets/icon.png" alt="ywza icon" width="128">
</p>

<h1 align="center">ywza</h1>

<p align="center">
  <b>Let an AI model use your PC, from anywhere.</b><br>
  <a href="#demo">Demo</a> •
  <a href="#build">Build</a> •
  <a href="#quick-start">Quick start</a> •
  <a href="#control-it-from-another-device">Pairing</a> •
  <a href="#use-it-from-an-mcp-client">MCP</a> •
  <a href="#linux">Linux</a>
</p>

---

ywza lets an AI model use your computer the way you do. It can look at your windows, click and type in any app, and work
in the terminals you already have open. You can talk to it from the same machine or from another one, and nothing has to
be exposed to the internet to make that work.

It runs as a small background agent on the PC you want to control. Other devices connect to it directly, peer to peer,
after you pair them once with a 6-digit code. There is no account and no server to host. You bring your own model:
anything that speaks the OpenAI chat API works, including local ones like Ollama and LM Studio.

Instead of sending the model a screenshot at every step, ywza describes each window as a numbered list of elements. That
keeps the token count low and the model's reasoning focused. It can still ask for a screenshot when it needs to see
something.

ywza works perfectly on Windows. Linux works on X11.

## Demo

<table>
  <tr>
    <td width="50%" align="center">
      <video src="https://github.com/user-attachments/assets/a8e35cd8-a06e-4d2b-b73a-34e204fda3d1" controls muted></video>
      <br>Opening Notepad, typing Hello World and moving the window to monitor 0
    </td>
    <td width="50%" align="center">
      <video src="https://github.com/user-attachments/assets/100879fa-9e3b-42be-8375-6a776974e1e9" controls muted></video>
      <br>Opening Reddit in Chrome
    </td>
  </tr>
  <tr>
    <td width="50%" align="center">
      <video src="https://github.com/user-attachments/assets/6fe66bb5-ee3c-4f7d-85ff-e31ea51a5b00" controls muted></video>
      <br>Typing Hello World into an empty terminal, then starting Claude in it
    </td>
    <td width="50%"></td>
  </tr>
</table>

## Build

You need a recent [Rust toolchain](https://rustup.rs). On Windows, the MSVC build tools that rustup asks for are enough.

```bash
git clone https://github.com/alielmorsy/ywza
cd ywza
cargo build --release
```

This produces two programs in `target/release`:

| Program | What it is |
|---|---|
| `ywza-agent` | The background service. Runs on the machine you want to control. |
| `ywza` | The command line client. Runs wherever you give the instructions. |

Put both on your PATH, or install them with cargo:

```bash
cargo install --path crates/ywza-agent
cargo install --path crates/ywza-cli
```

### OCR

Some apps draw their own text and expose nothing to Windows accessibility. For those, build the agent with an OCR
engine. You can compile several in and pick one in the config.

| Feature | Engine | Platform |
|---|---|---|
| `ocr-windows` | Built-in Windows OCR | Windows |
| `ocr-paddle` | PaddleOCR through a bundled ONNX Runtime | Windows, Linux |
| `ocr-paddle-dynamic` | PaddleOCR, ONNX Runtime loaded at startup | Linux, older distros |

```bash
cargo build --release -p ywza-agent --features ocr-windows
```

PaddleOCR models download on first use. `ocr-paddle-dynamic` replaces `ocr-paddle`, so don't enable both. It needs
`libonnxruntime.so` next to the agent binary, or at the path in `ORT_DYLIB_PATH`.

## Quick start

Start the agent:

```bash
ywza-agent run
```

The first launch creates your config folder and the agent's identity. Point it at a model:

```bash
ywza config set-llm --base-url https://api.openai.com/v1 --model <model> --api-key <key>
```

Then give it something to do:

```bash
ywza prompt "Open Notepad and write hello"
```

## Control it from another device

On the machine you want to control, ask the running agent for a pairing code:

```bash
ywza-agent pair
```

On the device you want to control it from:

```bash
ywza pair 123456 --name desktop
ywza -d desktop prompt "Take a screenshot and tell me what's open"
```

The code works once and expires after 10 minutes. On Windows, if the connection times out, run
`dist/windows-firewall.bat` as administrator with the path to `ywza-agent.exe`.

## Use it from an MCP client

The MCP server gives any MCP client control of this PC and of every device you have paired, through one server. The model
picks the device per call, so a single chat can work on your desktop and a remote machine side by side.

Paired devices don't need to be on the same network. They connect through public relays, so a laptop on a coffee shop
wifi can drive a PC at home. Own relay support is planned, so you will be able to run your own instead of the public
ones.

Register it straight from the repo, no install step. Replace `<repo>` with the folder you cloned ywza into:

```bash
claude mcp add ywza -- uv run --quiet --project <repo>/python ywza-mcp --cli <repo>/target/release/ywza.exe
```

On Linux the last path is `<repo>/target/release/ywza`. `--cli` is how the server reaches your paired devices, so
it can be left out if `ywza` is already on your PATH or you only control this PC. Any other MCP client can run the same
command.

## Linux

Only X11 sessions are supported. `dist/ywza-agent.service` runs the agent on the login screen and in every user
session. Edit the `ExecStart` path, then follow the comments at the top of the file.

