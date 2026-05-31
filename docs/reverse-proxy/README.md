# Reverse proxy: TLS + `X-API-Key` for crates-docs

The server's **in-process** authentication validates an API key sent as a
bearer token — `Authorization: Bearer <key>` — and it speaks **plain HTTP**
(no TLS). That is enough on a trusted/loopback network, but for anything exposed
you usually want two more things:

1. **TLS** so traffic can't be read or tampered with on the wire.
2. The ability to keep sending the key in the familiar **`X-API-Key`** header
   (many clients and the project's own config default to that name).

A small reverse proxy in front of the server provides both. It terminates TLS
and rewrites `X-API-Key: <key>` into `Authorization: Bearer <key>` so the
server's built-in check still runs.

```
                      TLS (HTTPS)                      plain HTTP, localhost
   ┌────────┐   X-API-Key: <key>     ┌───────────┐   Authorization: Bearer <key>   ┌──────────────┐
   │ client │ ─────────────────────► │  proxy    │ ──────────────────────────────► │  crates-docs │
   │        │ ◄───────────────────── │ (TLS term)│ ◄────────────────────────────── │ 127.0.0.1:8080│
   └────────┘                        └───────────┘                                 └──────────────┘
                                   edge key check (opt.)        in-process Argon2 verify (source of truth)
```

**Defense in depth.** The proxy may reject requests with no key at the edge
(fail fast), but the server remains the cryptographic source of truth: it
re-verifies every key against its stored Argon2 hash in constant time. If the
proxy is ever misconfigured or bypassed, the server still rejects bad keys.

> **Why not just send `X-API-Key` to the server directly?** The MCP SDK's auth
> middleware reads **only** the `Authorization: Bearer` header — it cannot be
> told to read `X-API-Key`. The proxy bridges that gap. (See the note in
> `src/server/auth/api_key_provider.rs`.)

---

## 1. Configure the backend (crates-docs)

Bind to **loopback only** so the server is reachable solely through the proxy,
and enable API-key auth.

Generate a key (prints the plain-text key once and its Argon2 hash):

```bash
crates-docs generate-api-key
```

Store the **hash** in your config and keep the **plain-text key** for clients:

```toml
# config.toml
[server]
host = "127.0.0.1"      # loopback: only the local proxy can reach it
port = 8080
transport_mode = "http" # or "sse" / "hybrid"

[auth.api_key]
enabled = true                       # runtime on/off switch (restart to apply)
keys = ["$argon2id$v=19$m=...$..."]  # the HASH from generate-api-key
```

Run it:

```bash
crates-docs serve --config config.toml
```

You should see a startup log line confirming **`API key authentication is
ENFORCED`**. (Auth is compiled in by default; flipping `enabled` and restarting
turns enforcement on/off — no rebuild needed.)

Quick local check that enforcement is on (before adding the proxy):

```bash
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/health                 # 200 (open)
curl -s -o /dev/null -w '%{http_code}\n' -X POST http://127.0.0.1:8080/mcp             # 401
curl -s -o /dev/null -w '%{http_code}\n' -X POST \
     -H "Authorization: Bearer <plain-key>" http://127.0.0.1:8080/mcp                  # not 401
```

## 2. Put a proxy in front

Pick one:

- **[`Caddyfile`](./Caddyfile)** — automatic HTTPS (Let's Encrypt for a real
  domain, internal CA for `localhost`). Simplest to run.
- **[`nginx.conf`](./nginx.conf)** — bring your own certificate.

Both: terminate TLS on `:443`, require `X-API-Key` on everything except
`/health`, translate it to `Authorization: Bearer`, and forward to
`127.0.0.1:8080`. Endpoints forwarded: `/mcp` (Streamable HTTP), `/sse` +
`/messages` (SSE), `/health` (open).

## 3. Verify end to end

```bash
# Missing key → rejected at the edge.
curl -sk -o /dev/null -w '%{http_code}\n' -X POST https://docs.example.com/mcp          # 401

# Valid key in X-API-Key → translated to Bearer, accepted by the backend.
curl -sk -o /dev/null -w '%{http_code}\n' -X POST \
     -H "X-API-Key: <plain-key>" \
     -H "Accept: application/json, text/event-stream" \
     https://docs.example.com/mcp                                                       # not 401

# Health stays open for monitoring.
curl -sk -o /dev/null -w '%{http_code}\n' https://docs.example.com/health               # 200
```

## Notes

- **Rate-limit at the edge for public exposure.** Each key check runs an
  intentionally CPU/memory-hard Argon2 verification, and the server does not cap
  request rate itself. Without an edge limit, an attacker can force expensive
  work with a flood of bad keys. Add a rate limit at the proxy (nginx
  `limit_req`; Caddy's `rate_limit` handler or a WAF) when the server faces
  untrusted networks — the edge `X-API-Key` check above already rejects
  keyless requests cheaply before they reach the backend.
- **Key rotation / revocation** is not hot-reloaded: update `keys` and
  **restart** the server (a deliberate safeguard so removed keys stop working
  immediately and predictably).
- **MCP `initialize`** clients should send `Accept: application/json,
  text/event-stream`.
- **SSE** needs response buffering disabled and long read timeouts; both proxy
  configs already set this.
- Keep `host = "127.0.0.1"`. Binding the backend to `0.0.0.0` while running a
  proxy would let clients reach it directly over plain HTTP and skip TLS.
