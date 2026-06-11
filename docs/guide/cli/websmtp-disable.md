# `madmail websmtp disable`

Parent: [`websmtp`](websmtp.md)

Disable the API (HTTP 404)

## Synopsis

```bash
madmail websmtp disable [OPTIONS]
```


After changes, run `madmail reload` (or restart) to apply.

## JSON output (`--json`)

```bash
madmail websmtp disable --json
```

Success stdout:

```json
{"ok": true, "command": "websmtp disable", "data": { ... }}
```

Schema: [json-output.md](json-output.md#websmtp-disable).


---
[← `websmtp`](websmtp.md) · [CLI index](README.md) · [Global flags](global-flags.md)
