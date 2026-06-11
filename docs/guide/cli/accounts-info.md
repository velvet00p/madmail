# `madmail accounts info`

Parent: [`accounts`](accounts.md)

One account (credentials, quota, blocklist)

## Synopsis

```bash
madmail accounts info [OPTIONS] <USERNAME>
```

## Examples

```bash
madmail accounts info alice@example.org
```

## JSON output (`--json`)

```bash
madmail accounts info --json
```

Success stdout:

```json
{"ok": true, "command": "accounts info", "data": { ... }}
```

Schema: [json-output.md](json-output.md#accounts-info).


---
[← `accounts`](accounts.md) · [CLI index](README.md) · [Global flags](global-flags.md)
