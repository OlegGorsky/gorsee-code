# Architecture

Gorsee Code is split into small crates so the TUI, CLI, gateway, tools,
and NeuroGate adapter do not own each other's business logic.

## Crate Map

- `gorsee-code-core`: shared domain contracts for agents, commands, events,
  missions, and capabilities.
- `gorsee-code-config`: layered project configuration and default rendering.
- `gorsee-code-neurogate`: NeuroGate HTTP adapter, response parsing, and
  normalized errors.
- `gorsee-code-limits`: `/v1/me` usage windows and warning decisions.
- `gorsee-code-usage`: local token ledger and budget policy.
- `gorsee-code-safety`: redaction, output bounds, path policy, and permission
  decisions.
- `gorsee-code-session`: JSONL flight recorder, manifests, replay, and export.
- `gorsee-code-artifacts`: local artifact records and session report files.
- `gorsee-code-tool-runtime`: typed tool registry and policy enforcement.
- `gorsee-code-tools`: built-in safe tools such as file reads, search, repo map,
  git diff, and bounded test execution.
- `gorsee-code-ui-state`: serializable view models consumed by clients.
- `gorsee-code-tui`: Mission Control renderer and fixture mode.
- `gorsee-code-gateway`: localhost HTTP/SSE app-server for future clients.
- `gorsee-code-cli`: user-facing `gcode` command.

## Event Flow

Domain work emits append-only events. The session store records them as JSONL,
the gateway streams them as SSE, and clients render view models derived from
those events. UI code sends commands through CLI/gateway surfaces rather than
calling tools or model adapters directly.

## NeuroGate Boundary

Foundation code only calls confirmed NeuroGate-compatible endpoints:
`GET /v1/models`, `GET /v1/me`, and `POST /v1/chat/completions`. Streaming
uses the same chat completions endpoint with `stream: true`; private analytics
or per-key usage APIs are intentionally outside this release.

## Session Artifacts

Each mission creates a session directory under `.gorsee-code/sessions/` with a
manifest, JSONL events, patch/artifact folders, and a markdown report artifact.
Gateway fixture state scans local session artifacts and exposes their metadata
through `/v1/artifacts` so future clients can render reports without reading
session internals directly.

## Foundation Scope

This release implements a mature foundation slice: configuration, safety,
session recording, usage and limit parsing, NeuroGate model/account adapters,
streaming chat completions, safe tool runtime, built-in skills and hooks,
fixture TUI, local artifacts, and gateway endpoints. Full autonomous coding
orchestration is intentionally represented by a deterministic sequential mission
scaffold until real NeuroGate behavior and policy decisions are tested.
