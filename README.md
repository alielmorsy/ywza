# ywza

A computer-use agent for your own PC that you can drive from anywhere, with no open ports and no server of your own.

- Peer-to-peer over [iroh](https://iroh.computer): QUIC with NAT hole punching and an encrypted relay fallback.
- Text-first screen understanding: windows are described as compact, ID-numbered element lists (UI Automation + OCR). Screenshots are a tool the model calls when it needs one.
- Terminals are first-class: read and type into the terminals you already have open, or let the agent start its own.
- Works with any OpenAI-compatible model endpoint.
- Pair a device with a 6-digit code.

Windows first. Linux works on X11.

## Layout

| Path | What |
|---|---|
| `crates/ywza-proto` | Wire protocol, config, pairing handshake |
| `crates/ywza-desktop` | Monitors, windows, UI elements, input, capture |
| `crates/ywza-ocr` | OCR engines (Windows OCR, PaddleOCR via ONNX) |
| `crates/ywza-term` | Owned PTY terminals and attaching to existing consoles |
| `crates/ywza-stream` | Live screen capture and H.264 encoding |
| `crates/ywza-agent` | The background agent service |
| `crates/ywza-cli` | `ywza` command-line client |
| `python/ywza_mcp` | MCP server and Python client for the agent's tools |
| `dist/` | systemd unit and Windows firewall script |

## Build

```bash
cargo build --release
cargo build --release -p ywza-agent --features ocr-paddle          # PaddleOCR, static onnxruntime
cargo build --release -p ywza-agent --features ocr-paddle-dynamic  # loads libonnxruntime.so at runtime
```

## License

AGPL-3.0-or-later
