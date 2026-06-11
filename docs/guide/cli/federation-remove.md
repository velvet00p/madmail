# `madmail federation remove`

Parent: [`federation`](federation.md)

Remove a domain from the rules table

## Synopsis

```bash
madmail federation remove [OPTIONS] <DOMAIN>
```

## JSON output (`--json`)

```bash
madmail federation remove --json
```

Success stdout:

```json
{"ok": true, "command": "federation remove", "data": { ... }}
```

Schema: [json-output.md](json-output.md#federation-remove).


---
[← `federation`](federation.md) · [CLI index](README.md) · [Global flags](global-flags.md)
