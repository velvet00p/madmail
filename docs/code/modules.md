# Module and package index

Main-tree packages grouped by role. Paths are relative to repository root (`madmail/`).

## Config module registry

Factories registered with `module.Register` / `RegisterEndpoint` in the main tree (excluding `test_*` and submodule binaries):

### Endpoints

| Name | Package |
|------|---------|
| `smtp`, `submission`, `lmtp` | `internal/endpoint/smtp` |
| `imap` | `internal/endpoint/imap` |
| `chatmail` | `internal/endpoint/chatmail` |
| `turn` | `internal/endpoint/turn` |
| `openmetrics` | `internal/endpoint/openmetrics` |

### Storage and targets

| Name | Package |
|------|---------|
| `storage.imapsql`, `target.imapsql` (alias, same `New`) | `internal/storage/imapsql` |
| `storage.blob` (filesystem) | `internal/storage/blob/fs` |
| `target.remote` | `internal/target/remote` |
| `target.queue` | `internal/target/queue` |
| `target.smtp`, `target.lmtp` | `internal/target/smtp` |

### Pipeline, checks, modifiers

| Name | Package |
|------|---------|
| `msgpipeline` | `internal/msgpipeline` |
| `checks` | `internal/msgpipeline` (group) |
| `modifiers` | `internal/modify` (group) |
| `check.spf`, `check.dkim`, `check.authorize_sender`, `check.pgp_encryption` | `internal/check/…` |
| `check.require_tls`, `check.require_matching_rdns`, `check.require_mx_record` | stateless via `RegisterStatelessCheck` |
| `modify.dkim`, `modify.replace_sender`, `modify.replace_rcpt` | `internal/modify/…` |

DMARC evaluation is built into `msgpipeline` (`internal/dmarc`), not a registered `check.*` name.

### Auth, tables, TLS, limits

| Name | Package |
|------|---------|
| `auth.pass_table` | `internal/auth/pass_table` |
| `table.file`, `table.static`, `table.chain`, `table.regexp`, `table.identity`, `table.email_*`, `table.sql_table`, `table.sql_query`, `table.gorm` | `internal/table` |
| `tls.loader.file`, `tls.loader.autocert`, `tls.loader.self_signed`, `tls.loader.acme` | `internal/tls`, `internal/tls/acme` |
| `limits` | `internal/limits` |
| `mx_auth`, `mx_auth.mtasts`, `mx_auth.dane`, `mx_auth.dnssec`, `mx_auth.local_policy`, `mx_auth.sts_preload` | `internal/target/remote` |

## Entry and framework

| Package | Role |
|---------|------|
| [`maddy.go`](../../maddy.go) | CLI wiring, `moduleMain`, global config |
| [`framework/module/`](../../framework/module/) | `Module`, `DeliveryTarget`, `Delivery`, `Check`, `Storage`, registries |
| [`framework/config/`](../../framework/config/) | Config map, lexer, TLS, endpoints |
| [`framework/cfgparser/`](../../framework/cfgparser/) | Parse `maddy.conf`, imports, env substitution |
| [`framework/exterrors/`](../../framework/exterrors/) | SMTP errors, temporary detection |
| [`framework/hooks/`](../../framework/hooks/) | Shutdown/reload/log rotate |
| [`framework/buffer/`](../../framework/buffer/) | In-memory / file message buffers |
| [`framework/dns/`](../../framework/dns/) | Resolver, DNSSEC helpers |
| [`framework/address/`](../../framework/address/) | Split, normalize, IDNA |
| [`framework/log/`](../../framework/log/) | Logging outputs |
| [`framework/future/`](../../framework/future/) | Async DNS/policy results for remote delivery |
| [`framework/module/settings.go`](../../framework/module/settings.go) | Global settings provider (DB-backed flags) |
| [`framework/module/msgcounter.go`](../../framework/module/msgcounter.go) | Atomic sent/received/outbound counters |

## Endpoints (`internal/endpoint/`)

| Package | Config name | Purpose |
|---------|-------------|---------|
| [`smtp`](../../internal/endpoint/smtp/) | `smtp`, `submission`, `lmtp` | Inbound mail + authenticated submission |
| [`imap`](../../internal/endpoint/imap/) | `imap` | IMAP4rev1, COMPRESS, SORT, QUOTA, PGP on APPEND |
| [`chatmail`](../../internal/endpoint/chatmail/) | `chatmail` | HTTP(S) Chatmail server — [chatmail.md](./chatmail.md) |
| [`webimap`](../../internal/endpoint/webimap/) | (mounted by chatmail) | WebIMAP + WebSMTP over HTTP/WebSocket |
| [`turn`](../../internal/endpoint/turn/) | `turn` | TURN credentials (VoIP) |
| [`openmetrics`](../../internal/endpoint/openmetrics/) | `openmetrics` | Prometheus metrics |

## Routing and policy

| Package | Purpose |
|---------|---------|
| [`internal/msgpipeline/`](../../internal/msgpipeline/) | Source/destination routing, check/modifier orchestration |
| [`internal/check/`](../../internal/check/) | SPF, DKIM, DNS stateless checks, require_tls, authorize_sender, pgp_encryption |
| [`internal/dmarc/`](../../internal/dmarc/) | DMARC verifier (used from msgpipeline) |
| [`internal/pgp_verify/`](../../internal/pgp_verify/) | Central PGP-only gate (structure check, no decrypt) — [pgp-verification.md](./pgp-verification.md) |
| [`internal/modify/`](../../internal/modify/) | DKIM sign, envelope rewrites |
| [`internal/table/`](../../internal/table/) | SQL/file/regexp tables for auth and routing |
| [`internal/auth/pass_table/`](../../internal/auth/pass_table/) | Credential lookup, blocklist integration |
| [`internal/authz/`](../../internal/authz/) | Username normalization |
| [`internal/auth/`](../../internal/auth/) | SASL helpers shared by SMTP/IMAP |
| [`internal/federationtracker/`](../../internal/federationtracker/) | Federation policy + metrics |
| [`internal/limits/`](../../internal/limits/) | Rate / concurrency limits |

## Delivery targets (`internal/target/`)

| Package | Config | Implements |
|---------|--------|------------|
| [`remote`](../../internal/target/remote/) | `target.remote` | MX SMTP + HTTP `/mxdeliv` |
| [`queue`](../../internal/target/queue/) | `target.queue` | Disk spool + retries + DSN |
| [`smtp`](../../internal/target/smtp/) | `target.smtp` | Fixed downstream SMTP/LMTP |
| [`skeleton.go`](../../internal/target/skeleton.go) | — | Template for new targets |

## Storage

| Package | Config | Implements |
|---------|--------|------------|
| [`internal/storage/imapsql/`](../../internal/storage/imapsql/) | `storage.imapsql` | IMAP + SMTP delivery + quotas |
| [`internal/go-imap-sql/`](../../internal/go-imap-sql/) | (library) | SQL schema, blobs, compression, IDLE |
| [`internal/storage/blob/fs/`](../../internal/storage/blob/fs/) | blob backend | Filesystem blob store |
| [`internal/quota/`](../../internal/quota/) | — | In-memory quota cache |

## GORM models (`internal/db/models.go`)

| Model | Table / use |
|-------|-------------|
| `Quota` | Per-user storage limits, first/last login, registration token consumption |
| `BlockedUser` | `blocked_users` — ban list |
| `Contact` | Contact-sharing slugs (`sharing.db`) |
| `EndpointOverride` | `dns_overrides` — outbound routing overrides |
| `Exchanger` | Pull relay configuration |
| `RegistrationToken` | Invite-only registration |
| `MessageStat` | Persisted sent/received/outbound counters |
| `FederationRule` | Federation policy exceptions (`federationtracker`) |
| `TableEntry` | Generic KV for `table.sql_table` |

## Supporting infrastructure

| Package | Purpose |
|---------|---------|
| [`internal/db/`](../../internal/db/) | GORM models + [`New()`](../../internal/db/db.go) driver helpers |
| [`internal/endpoint_cache/`](../../internal/endpoint_cache/) | DB-backed MX/endpoint overrides |
| [`internal/updatepipe/`](../../internal/updatepipe/) | IMAP update pub/sub between instances |
| [`internal/smtpconn/pool/`](../../internal/smtpconn/pool/) | Outbound SMTP connection pool |
| [`internal/dsn/`](../../internal/dsn/) | Delivery status notification generation |
| [`internal/api/admin/`](../../internal/api/admin/) | REST handlers for admin UI |
| [`internal/adminweb/`](../../internal/adminweb/) | Embedded admin UI static files (built from submodule) |
| [`internal/servertracker/`](../../internal/servertracker/) | Seen peer IPs/domains (`madmail online`) |
| Contact sharing | `internal/endpoint/chatmail` + `internal/db` (`Contact` model), `internal/cli/ctl/sharing.go` |
| Iroh relay URL | Exposed via IMAP METADATA (`internal/endpoint/imap`) and admin settings; no separate package |
| [`internal/proxy_protocol/`](../../internal/proxy_protocol/) | HAProxy PROXY v1/v2 |
| [`internal/tls/`](../../internal/tls/), [`internal/tls/acme/`](../../internal/tls/acme/) | TLS loaders, ACME |

## CLI (`internal/cli/`)

| Area | Examples |
|------|----------|
| [`ctl/accounts_*.go`](../../internal/cli/ctl/) | create, ban, bulk import |
| [`ctl/imap.go`](../../internal/cli/ctl/imap.go) | IMAP account/mailbox/msg tools |
| [`ctl/install.go`](../../internal/cli/ctl/install.go) | systemd, stealth deploy |
| [`ctl/federation.go`](../../internal/cli/ctl/federation.go) | Federation CLI |
| [`ctl/reload_config.go`](../../internal/cli/ctl/reload_config.go) | Config management |

## Key interfaces (quick reference)

```go
// framework/module/delivery_target.go
type DeliveryTarget interface {
    Start(ctx, msgMeta, mailFrom) (Delivery, error)
}
type Delivery interface {
    AddRcpt(ctx, rcptTo, opts) error
    Body(ctx, header, body buffer.Buffer) error
    Abort(ctx) error
    Commit(ctx) error
}
```

```go
// framework/module/check.go — order: CheckConnection, CheckSender, CheckRcpt, CheckBody
type Check interface { /* stateful per message */ }
```

```go
// framework/module/storage.go
type Storage interface {
    backend.Backend // go-imap
    /* + management APIs */
}
```

## Submodule touchpoints (code in main tree only)

| Submodule | Main-tree integration |
|-----------|------------------------|
| `admin-web` | Built to `internal/adminweb/build/`, served by chatmail |
| `exchangers/madexchanger` | `internal/endpoint/chatmail/exchanger.go` pull/inject |
| `chatmail-core` | Not linked; clients use IMAP/SMTP against this server |
| `tests/cmlxc` | External test harness |

Human input may be needed to document exchanger HTTP API details — see submodule `docs/` if required.
