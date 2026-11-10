# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Stack

Tauri v2 + React 19 + Tailwind 4 in `app/`. Platform is `web` (webview); it must read as a small native desktop window.

## Users

One person who owns several computers and a phone. They sit at their PC, pair other devices to it, decide what each device may do, and send prompts to an AI agent that operates a device for them. Inferred from docs/design.md and docs/usage.md (not interviewed).

## Product Purpose

ywza is a computer-use agent for your own PC, reachable from anywhere with no open ports. The desktop app is where the owner pairs devices, gives each device access, chats with an agent, watches a live view of a device and answers approvals.

## Positioning

Peer-to-peer (iroh), no server of ours. Safe by default: only paired devices connect, each with explicit permissions.

## Operating Context

Small desktop window next to other work, not full screen. Footer status bar. Windows first, Linux later.

## Capabilities and Constraints

- Pairing: one-time 6-digit code, valid 10 min, single use.
- Per-device permissions: view, chat, control, tools, settings (admin).
- Settings for this computer (not per device): AI endpoint and key, default model and effort, desktop, OCR, streaming, relay.
- Every chat runs on this PC's AI. For a paired device the AI is told it operates that device remotely, and its tools run there.
- Live view of any monitor of a device, with mouse and keyboard control. Split view follows the window the AI works in, with an element debug overlay.

## Brand Commitments

User pinned: keep the existing colors (`app/src/theme.css`, warm near-black with muted indigo accent). Desktop-app feel, not website. Device rail, minimal toolbar, footer status bar.

## Evidence on Hand

Old app (D:/works/ywza/app) and docs/specs/desktop-app.md. Device names in the preview mock are synthetic.

## Product Principles

- Pairing key and linked devices are the home screen.
- Access is explicit: global default first, per-device override second.
- Risky things ask; the human can always take control.
- Status lives in the footer, calm by default.
