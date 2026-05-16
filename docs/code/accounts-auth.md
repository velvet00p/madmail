# Accounts, authentication, and registration

How users are created, authenticated, and blocked in the **main Madmail tree**. Submodule client code (`chatmail-core`) is out of scope.

## Credential storage (`auth.pass_table`)

**Package:** [`internal/auth/pass_table/`](../../internal/auth/pass_table/)

- Passwords live in a mutable **table** module (often `table.sql_table` / sqlite `credentials.db`).
- SMTP/IMAP AUTH: [`internal/auth/sasl.go`](../../internal/auth/sasl.go) + endpoint session → `pass_table` verify.
- Usernames normalized via [`internal/authz/`](../../internal/authz/) and global `auth_map` / `auth_map_normalize` in `maddy.conf`.
- Opportunistic hash upgrade: legacy algorithms re-hashed in a background goroutine after successful login ([`table.go`](../../internal/auth/pass_table/table.go)).

`pass_table` consults [`module.IsBlocked`](../../framework/module/blocklist.go) (registered by `imapsql`) so banned users cannot authenticate or be JIT-recreated.

## Registration (`GET/POST /new`)

**Handler:** [`handleNewAccount`](../../internal/endpoint/chatmail/chatmail.go) — see [chatmail.md](./chatmail.md#account-registration-new)

| Step | Behavior |
|------|----------|
| Policy | `__REGISTRATION_OPEN__` in settings DB; optional `registration_token_required` |
| Token | Validates [`RegistrationToken`](../../internal/db/models.go); quota row may store `used_token` until first login |
| Create | `storage.CreateIMAPAcct(email)` + password in credentials table |
| Blocklist | Rejects usernames in `blocked_users` |
| Browser success | Redirect to `dclogin://…` deep link with IMAP/SMTP host, ports, security (`dcloginTransport`) |

CLI equivalent: `madmail create-user`, `madmail accounts create`, bulk import in [`accounts_bulk.go`](../../internal/cli/ctl/accounts_bulk.go).

## JIT (just-in-time) accounts

JIT creates mailboxes on **first use** without visiting `/new`.

| Trigger | Code path |
|---------|-----------|
| IMAP/SMTP login, unknown user | [`GetOrCreateIMAPAcct`](../../internal/storage/imapsql/imapsql.go) when `__JIT_REGISTRATION_ENABLED__` |
| Inbound delivery to new address | [`delivery.AddRcpt`](../../internal/storage/imapsql/delivery.go) → `CreateIMAPAcct` when JIT enabled |
| IMAP endpoint | [`GetOrCreateIMAPAcct`](../../internal/endpoint/imap/imap.go) on login |

Guards: registration-open defaults, **blocklist** check, quota row with `FirstLoginAt` marker for new JIT users.

`pass_table` JIT for auth credentials is separate (flight-tested in `jit_flight_test.go`); storage JIT creates the IMAP account and mailbox layout.

## Blocklist

**Model:** [`db.BlockedUser`](../../internal/db/models.go) — table `blocked_users`.

- Populated by admin API / CLI ban / delete flows.
- Loaded into `imapsql` in-memory cache; reloaded on **SIGUSR2** (`EventReload`).
- IMAP endpoint disconnects sessions for newly blocked users on reload.

## Settings keys (representative)

Stored via GORM settings table (`table.gorm` / install templates). Read at runtime through `authDB.GetSetting` or `module.GetGlobalSetting`:

| Key | Purpose |
|-----|---------|
| `__REGISTRATION_OPEN__` | Allow `/new` |
| `__JIT_REGISTRATION_ENABLED__` | Auto-create mailbox on delivery/login |
| `__WEBIMAP_ENABLED__` / `__WEBSMTP_ENABLED__` | HTTP mail APIs |
| `*_local_only` | Bind service to loopback only |
| `dclogin_*_security` | Deep-link socket type (ssl/starttls/plain) |

Full list: admin settings in [chatmail.md § Admin API](./chatmail.md#admin-api).

## Related flows

- Inbound mail to new JIT user: [message-incoming.md](./message-incoming.md) §7 (`AddRcpt` → `CreateIMAPAcct`).
- Outbound mail requires existing auth: [message-outgoing.md](./message-outgoing.md) §1 (`authorize_sender` on submission).
- Federation does not create users; unknown rcpts on `/mxdeliv` may be dropped.
