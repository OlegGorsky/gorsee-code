# Security

Please report vulnerabilities privately to the maintainers before public
disclosure.

## Defaults

- Writes, patches, arbitrary commands, and network actions require approval.
- Deletes are denied by default.
- Path policy rejects files outside the configured workspace.
- Tool output is bounded before it reaches session logs or the terminal.
- Redaction covers common tokens, bearer headers, cookies, private keys, and
  user-provided regexes.

## Threat Model

The Foundation release assumes a local trusted user and an untrusted model.
Model outputs must pass through command parsing, permission policy, bounded
tool execution, and redaction before they can affect the workspace.
