# `madmail webimap status`

Parent: [`webimap`](webimap.md)

Show whether the API is enabled

## Synopsis

```bash
madmail webimap status [OPTIONS]
```


After changes, run `madmail reload` (or restart) to apply.

## JSON output (`--json`)

```bash
madmail webimap status --json
```

Success stdout:

```json
{"ok": true, "command": "webimap status", "data": { ... }}
```

Schema: [json-output.md](json-output.md#webimap-status).


---
[← `webimap`](webimap.md) · [CLI index](README.md) · [Global flags](global-flags.md)
