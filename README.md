# Gorsee Code

Gorsee Code is a local NeuroGate-native coding workspace.
Install it once, run `gcode`, add your NeuroGate API key, and work from the
terminal UI.

## Install

```bash
npm install -g @gorsee/code
gcode
```

The first `gcode` launch asks for a NeuroGate API key, stores it locally, and
opens the coding TUI.

## Common Commands

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
```

## Configuration

`gcode init` creates `gorsee-code.toml`.
The API key is read from `NEUROGATE_API_KEY` by default.
`gcode auth set` can also store a local project key in
`.gorsee-code/auth.json`, which is ignored by git.

## Safety

Gorsee Code defaults to a balanced policy:

- read/search/test inside the workspace are allowed;
- writes, patches, commands, and network actions ask for approval;
- deletes and access outside the workspace are denied;
- event logs, exports, gateway payloads, and terminal output pass through
  redaction helpers before display.
