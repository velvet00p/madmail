# Runtime behavior

For the **full timeline from process start** (`init()` → `madmail run` → `moduleMain` → listeners → signals → shutdown) and **configuration layers** (`maddy.conf` vs settings DB), see **[startup-and-config.md](./startup-and-config.md)**.

## Process model

- **Single process** per `madmail run` instance.
- **Goroutine-heavy**: per-connection handlers (go-smtp, go-imap), per-domain outbound delivery in `target.remote`, parallel checks in `msgpipeline` early checks, queue workers, HTTP server for chatmail/admin.
- **No separate worker binary** for queue: retry logic runs inside the same process (`internal/target/queue` time wheel).

Full inventory of explicit `go` statements, background loops, and per-request spawns: **[goroutines.md](./goroutines.md)**.

Shutdown ([`maddy.go`](../../maddy.go) `moduleMain`):

1. Signal handler unblocks `handleSignals` (first SIGINT/SIGTERM/SIGHUP; second forces exit via nested handler — see [goroutines.md](./goroutines.md)).
2. `hooks.EventShutdown` — each registered `io.Closer` module (endpoints, queue, TLS loaders, …).
3. Log: "Waiting for running transactions to complete…"

Endpoints close listeners first, then `go-smtp`/`http` shutdown (SMTP documents lock ordering in `Endpoint.Close`).

## Hooks and signals

Defined in [`framework/hooks/hooks.go`](../../framework/hooks/hooks.go):

| Event | Trigger | Typical subscribers |
|-------|---------|-------------------|
| `EventShutdown` | Process exit | Endpoint `Close`, log flush |
| `EventReload` | SIGUSR2 (Linux) | pass_table cache, imapsql blocklist, alias tables, TLS certs |
| `EventLogRotate` | SIGUSR1 | Reopen log files |

**SIGUSR2 soft reload** is also sent by CLI after account ban/delete ([`internal/cli/ctl/reload_signal_linux.go`](../../internal/cli/ctl/reload_signal_linux.go), `webmail_services.go`).

IMAP endpoint ([`internal/endpoint/imap/imap.go`](../../internal/endpoint/imap/imap.go)): on `EventReload`, closes sessions for blocklisted users.

**Admin HTTP reload** ([`internal/api/admin/resources/reload.go`](../../internal/api/admin/resources/reload.go)): writes pending config; install scripts may restart systemd (harder reload than SIGUSR2).

Chatmail registers `/admin/reload` and `/admin/cache/reload` ([`internal/endpoint/chatmail/chatmail.go`](../../internal/endpoint/chatmail/chatmail.go)).

## Configuration reload limits

`EventReload` does **not** re-parse full `maddy.conf` module graph (by design — see hooks comment). Full config changes usually require process restart.

Dynamic data reload examples:

- Blocklist / banned users in memory (`imapsql.blockedSet`)
- Endpoint cache for federation overrides
- File-based alias tables referenced by tables module

## Concurrency and locking

| Area | Mechanism |
|------|-----------|
| SMTP session | `msgLock` on `Session` — serializes MAIL/RCPT/DATA vs async `Close` |
| msgpipeline delivery | Per-target `Delivery` map; underlying storage may serialize via SQLite |
| imapsql | `SerializationError` → SMTP 453 retry |
| remote pool | `connMu`, per-domain connection reuse |
| Queue | On-disk files + in-process wheel; panic recovery in workers |
| Blocked users cache | `blockedMu` RWMutex |

## Limits and backpressure

[`internal/limits/`](../../internal/limits/) — configured per endpoint (`limits { all rate … concurrency … }`):

- SMTP: `TakeMsg` / `ReleaseMsg` around each transaction (`session.startDelivery` / `cleanSession`).
- Remote: `TakeDest` per recipient domain during outbound delivery.

## Tracing

`runtime/trace` tasks in SMTP session (`MAIL FROM`, `RCPT TO`, `DATA`), imapsql delivery regions, remote `BodyNonAtomic`.

Enable via Go trace tooling / debug builds; not required for normal operation.

## IPC and FFI

Madmail **does not** embed `chatmail-core` in-process. There is no Rust FFI in the main Go tree.

Integration with Delta Chat is **protocol-level**:

- IMAP/SMTP from clients to this server.
- Optional Python E2E tests in submodule `tests/cmlxc` / `tests/deltachat-test`.

Admin Web UI is a **submodule** (`admin-web`); built artifacts are embedded from [`internal/adminweb/build/`](../../internal/adminweb/build/) for production.

## Observability

| Mechanism | Location |
|-----------|----------|
| Structured logging | `framework/log` (zap-backed) |
| OpenMetrics endpoint | `internal/endpoint/openmetrics` |
| Message counters | `framework/module/msgcounter.go` |
| Federation stats | `internal/federationtracker`, admin API |
| Server online tracking | `internal/servertracker` |

Chatmail **nolog** policy toggles log sinks via settings DB (see user doc [nolog.md](../chatmail/nolog.md)).

## Module initialization order

1. Parse config → `RegisterModules` constructs all instances (endpoints + named blocks).
2. `initModules` → `Init()` on each **endpoint** only.
3. First `&reference` from another module → `GetInstance` → `Init()` on that module (and shutdown hook if `Closer`).
4. Inline modules in config → `initInlineModule` at parse time of the parent.

Settings-dependent behavior uses [`GetGlobalSetting`](../../framework/module/settings.go) which may trigger step 3 for the auth/settings module.

## CLI ↔ daemon interaction

[`internal/cli/ctl/`](../../internal/cli/ctl/) commands (see [overview.md](./overview.md#cli-madmail-without-run)):

- Direct DB access (sqlite/postgres) for accounts, credentials, settings, queue files.
- `reloadRunningDaemons()` — SIGUSR2 to PIDs from runtime dir / systemd ([`reload_signal_linux.go`](../../internal/cli/ctl/reload_signal_linux.go)).
- Ban/delete paths often reload blocklist + signal daemon.
- Install/uninstall writes systemd units and `maddy.conf` ([`install.go`](../../internal/cli/ctl/install.go)).

No custom Unix socket RPC for admin (except optional IMAP `enable_update_pipe` unix socket for replication). Management is **DB + signals + HTTP admin API**.

## Async delivery semantics

| Path | Client-visible completion | Actual remote send |
|------|---------------------------|-------------------|
| Submission → queue | SMTP 250 after queue `Commit` | Later, queue worker |
| Submission → remote (no queue) | 250 after remote attempt completes | Same connection era |
| WebSMTP → `RemoteTarget` = queue | HTTP 200 after queue `Commit` | Queue worker |
| WebSMTP → `RemoteTarget` = remote | HTTP 200 after remote `Commit` | Same request (blocks on delivery) |
| Inbound SMTP → imapsql | 250 after storage `Commit` | N/A |

Queue retries use exponential backoff (`initialRetryTime`, `retryTimeScale`, `maxTries` in queue config).

## Security-related runtime gates

Central PGP policy: [`internal/pgp_verify/pgp_verify.go`](../../internal/pgp_verify/pgp_verify.go). Per-path behavior, config, and known layering (session vs pipeline): **[pgp-verification.md](./pgp-verification.md)**.

Federation inbound SMTP: [`internal/federationtracker/policy.go`](../../internal/federationtracker/policy.go) before pipeline.

Mxdeliv TLS: [`internal/endpoint/chatmail/mxdeliv_security.go`](../../internal/endpoint/chatmail/mxdeliv_security.go).

## Message counters

| Counter | Incremented when |
|---------|------------------|
| `received_messages` | Inbound SMTP DATA commit (not submission) |
| `sent_messages` | Submission DATA commit |
| `outbound_messages` | Queue worker successful delivery to downstream target |

Atomics in process; flushed to `message_stats` table every 30s ([`imapsql.flushMessageCounters`](../../internal/storage/imapsql/imapsql.go)).

## IMAP replication (`enable_update_pipe`)

Optional in `storage.imapsql` config:

| Driver | Mechanism |
|--------|-----------|
| sqlite3 | Unix socket under `runtime_dir` |
| postgres | `LISTEN/NOTIFY` via [`updatepipe/pubsub`](../../internal/updatepipe/pubsub/pq.go) |

Pushes IMAP `mess.Update` to peer instances for IDLE on replicated deployments.
