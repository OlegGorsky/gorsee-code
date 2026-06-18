# Gorsee Code

Gorsee Code is a local NeuroGate-native coding-agent command center.
It combines a Rust CLI, TUI Mission Control, a local gateway, safe tools,
session flight recording, budget accounting, and NeuroGate adapters.

## Install

```bash
npm install -g @gorsee/code
gcode
```

The first `gcode` launch asks for a NeuroGate API key, stores it locally, and
opens the coding TUI.

## Commands

```bash
gcode init
gcode auth set
gcode doctor
gcode models
gcode limits
gcode mission "audit this repository"
gcode skills run repo-audit
gcode pause
gcode resume
gcode export
gcode tui --fixture mission-running
gcode gateway --bind 127.0.0.1:3737
```

## NeuroGate Configuration

`gcode init` creates `gorsee-code.toml`.
The API key is read from `NEUROGATE_API_KEY` by default.
`gcode auth set` can also store a local project key in
`.gorsee-code/auth.json`, which is ignored by git.

The Foundation adapter uses the confirmed NeuroGate-compatible endpoints:

- `GET /v1/models`
- `GET /v1/me`
- `POST /v1/chat/completions`

Streaming completions use the same `POST /v1/chat/completions` endpoint with
`stream: true`.

## Safety Defaults

Gorsee Code defaults to a balanced policy:

- read/search/test inside the workspace are allowed;
- writes, patches, commands, and network actions ask for approval;
- deletes and access outside the workspace are denied;
- event logs, exports, gateway payloads, and terminal output pass through
  redaction helpers before display.

## Development

```bash
cargo fmt --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The workspace uses `reqwest` with `rustls-tls`, so it does not require
OpenSSL-specific system setup on NixOS.
