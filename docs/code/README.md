# Madmail — code documentation

Developer-oriented documentation for the **main Madmail tree** (Go server, CLI, admin API, embedded web endpoints). This complements user-facing docs under [`docs/`](../index.md) and [`HACKING.md`](../../HACKING.md).

## Scope

| In scope | Out of scope (git submodules) |
|----------|-------------------------------|
| `maddy.go`, `framework/`, `internal/` | `chatmail-core/` (Delta Chat Rust core) |
| `internal/go-imap-sql/` (vendored fork in-tree) | `admin-web/` (Svelte admin UI sources) |
| `tests/` (references only) | `exchangers/madexchanger/`, `exchangers/madexchanger-php/` |
| | `tests/cmlxc/` |

Submodule boundaries are defined in [`.gitmodules`](../../.gitmodules).

When tracing behavior, prefer the linked Go sources over prose — configs vary (`maddy.conf`, install templates, Docker examples).

**Recommended reading order:** [startup-and-config.md](./startup-and-config.md) (boot + `maddy.conf`) → [chatmail.md](./chatmail.md) (HTTP endpoint, admin, federation) → [pgp-verification.md](./pgp-verification.md) (encryption policy) → [message-incoming.md](./message-incoming.md) / [message-outgoing.md](./message-outgoing.md) (mail paths) → [connectivity-updating.md](./connectivity-updating.md) (client “Updating…” vs IMAP) → [goroutines.md](./goroutines.md) / [runtime.md](./runtime.md) (concurrency + reload).

## Document map

| File | Contents |
|------|----------|
| [startup-and-config.md](./startup-and-config.md) | **Runtime from `madmail run`**, shutdown, `maddy.conf`, settings DB |
| [overview.md](./overview.md) | Repository layout, binaries, configuration summary |
| [architecture.md](./architecture.md) | Module system, startup, composable pipeline |
| [message-incoming.md](./message-incoming.md) | All paths that accept mail into storage |
| [message-outgoing.md](./message-outgoing.md) | Submission, queue, remote/federation delivery |
| [runtime.md](./runtime.md) | Concurrency, hooks, reload, limits, observability |
| [goroutines.md](./goroutines.md) | Goroutine catalog: listeners, background workers, per-message spawns |
| [chatmail.md](./chatmail.md) | **Chatmail endpoint:** routes, admin API, mxdeliv, SS, ALPN, exchanger, www |
| [http-surfaces.md](./http-surfaces.md) | HTTP route index (points to chatmail.md for detail) |
| [accounts-auth.md](./accounts-auth.md) | Registration, JIT, pass_table, blocklist, dclogin |
| [pgp-verification.md](./pgp-verification.md) | PGP-only policy: `pgp_verify`, Secure-Join, per-path gates |
| [performance.md](./performance.md) | Large SMTP upload CPU/I/O (buffering, duplicate PGP checks, queue copy) |
| [message-checks-pipeline.md](./message-checks-pipeline.md) | **Checks vs pipeline audit:** `PGPPolicyVerified`, inconsistencies, optimization paths |
| [connectivity-updating.md](./connectivity-updating.md) | **“Updating…” badge:** chatmail-core connectivity FSM, when it ends, Madmail IMAP impact, stuck UI |
| [modules.md](./modules.md) | Package index, config module registry, key types |

**16 documents** in this folder.

- Regenerate PGP/pipeline context: `bash docs/code/build-context.sh` → [`context.txt`](./context.txt)
- Regenerate connectivity / “Updating…” context: `bash docs/code/build-context-connectivity.sh` → [`context-connectivity.txt`](./context-connectivity.txt)

## Related user docs

- SMTP routing rules: [`docs/reference/smtp-pipeline.md`](../reference/smtp-pipeline.md)
- Federation wire format: [`docs/chatmail/federation.md`](../chatmail/federation.md)
- PGP-only policy: [`docs/chatmail/only_pgp_mails.md`](../chatmail/only_pgp_mails.md)
- Example config: [`maddy.conf`](../../maddy.conf)
