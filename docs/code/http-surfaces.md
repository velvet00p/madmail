# HTTP surfaces

Most HTTP traffic is served by the **`chatmail`** endpoint module. For full behavior (init, security, admin, Shadowsocks, ALPN), see **[chatmail.md](./chatmail.md)**.

WebIMAP/WebSMTP are not a separate endpoint; they register on the chatmail `ServeMux` at `/webimap`.

---

## Route index

| Path | Purpose | Detail |
|------|---------|--------|
| `POST /mxdeliv` | Federation mail ingress | [chatmail.md § mxdeliv](./chatmail.md#federation-ingress-post-mxdeliv), [message-incoming.md](./message-incoming.md) |
| `GET/POST /new` | Account registration | [accounts-auth.md](./accounts-auth.md) |
| `GET /qr` | QR PNG (`?data=`) | [chatmail.md](./chatmail.md) |
| `GET /madmail` | Download running server binary | [chatmail.md](./chatmail.md) |
| `GET /inv/…` | Invite landing page | [chatmail.md](./chatmail.md) |
| `GET /.well-known/_domainkey/…` | DKIM TXT for federation | [chatmail.md](./chatmail.md) |
| `GET /share` | Contact sharing (optional) | [chatmail.md § Contact sharing](./chatmail.md#contact-sharing) |
| `GET /app` | Embedded Delta Chat web client shell | [chatmail.md](./chatmail.md) |
| `GET /docs` | Operator documentation HTML | [chatmail.md](./chatmail.md) |
| `GET /` | Static `www/` + contact slugs | [chatmail.md](./chatmail.md) |
| `{admin_path}` | Admin JSON API (POST) | [chatmail.md § Admin API](./chatmail.md#admin-api) |
| `{admin_web_path}/` | Admin dashboard SPA | [chatmail.md § Admin Web UI](./chatmail.md#admin-web-ui) |

---

## WebIMAP / WebSMTP (`/webimap`)

Registered by [`webimap.Handler.Register`](../../internal/endpoint/webimap/webimap.go) from chatmail `Init`:

| Path | Method | Purpose |
|------|--------|---------|
| `/webimap/mailboxes` | GET | List mailboxes |
| `/webimap/messages` | GET | List messages in mailbox |
| `/webimap/message/{uid}` | GET, DELETE | Fetch or delete one message |
| `/webimap/message/flags` | — | Flag updates (via `handleMessage` routing) |
| `/webimap/ws` | WebSocket | Interactive IMAP + `send` command |
| `/webimap/send`, `/websmtp/send` | POST | WebSMTP JSON send |

Auth: `X-Email` + `X-Password` (plain auth against `auth_db`). Feature flags: `__WEBIMAP_ENABLED__` / `__WEBSMTP_ENABLED__`.

Outbound: `module.GetInstance("outbound_delivery")` as `RemoteTarget` (typically `target.remote`). See [message-outgoing.md](./message-outgoing.md).

Implementation: [`internal/endpoint/webimap/`](../../internal/endpoint/webimap/) (main tree, not a submodule).

---

## Admin API protocol

Single mount (default `/api/admin`, overridable in DB). Request envelope:

```json
{
  "method": "GET|POST|PUT|DELETE|PATCH",
  "resource": "/admin/status",
  "headers": { "Authorization": "Bearer <token>" },
  "body": {}
}
```

Rate limiting and 1 MB body cap apply before auth ([`admin.go`](../../internal/api/admin/admin.go)). Full resource list: [chatmail.md § Admin API](./chatmail.md#admin-api).

---

## ALPN single-port mode

When `alpn_smtp` / `alpn_imap` are set on the chatmail TLS listener, port 443 (or configured HTTPS port) multiplexes HTTP + SMTP + IMAP. See [chatmail.md § ALPN](./chatmail.md#alpn-multiplexing) and [goroutines.md](./goroutines.md).

---

## Related

- [chatmail.md](./chatmail.md) — primary reference for this endpoint
- [message-incoming.md](./message-incoming.md) — `/mxdeliv`, exchanger inject
- [message-outgoing.md](./message-outgoing.md) — WebSMTP remote leg
- [runtime.md](./runtime.md) — admin reload vs SIGUSR2
