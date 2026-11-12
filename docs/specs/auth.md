---
status: todo
created: 2026-11-12
size: M
design: [sofarr.md §7, sofarr.md §9 D8, sofarr.md §9 D15]
media: none
oracle: none
---

# Authentication

## Context

sofarr's HTTP API (`/api/movies`, `/api/wanted`, `/api/grabs`, `POST /api/command/rss-sync`, and
everything later specs add) answers anyone who can reach port 7676, and server errors return the
full internal error chain to the client. At home behind a router that is acceptable; behind a
reverse proxy or with the web UI (spec `web-ui`) it is not. D15 decides optional authentication:
one user with a password for the UI and an API key for scripts.

Terms used below:

- **Auth on / off** – `[auth] enabled`; default off, so an existing setup keeps working unchanged.
- **Credential** – a valid session cookie or the configured API key in the `X-Api-Key` header.
- **Open endpoints** – endpoints that never need a credential: the health endpoint, the login,
  logout and "who am I" endpoints, and the files of the web UI (every path outside `/api/`); every other path
  under `/api/` needs a credential. The UI itself asks for a login through the API.
- **Client address** – the address a request counts as coming from: the peer address, or – when
  the peer is a trusted proxy – the address from `X-Forwarded-For` (see AC-10). An IPv4-mapped
  IPv6 address counts as its IPv4 address, because it is the same host and must not bypass a
  range or the throttling. For trusted networks any other IPv6 address counts as the full
  address; for the login throttling (AC-09) an IPv6 address counts by its /64 prefix, because
  one home connection owns a whole /64 and could otherwise rotate addresses.
- **Trusted network** – an address range in `[auth] trusted_networks`.

## Scope

### In scope

- Switching authentication on and off in `sofarr.toml`
- Login with username and password, sessions, logout
- API key for scripts
- Brute-force protection of the login
- Trusted networks that need no credential; trusted proxies for the client address
- A CLI command that produces the password hash for the config
- Generic error messages for server errors

### Out of scope

- Several users or roles — because sofarr has one user (sofarr.md §2, non-goals)
- Changing the password or key through the API or UI — because both live in `sofarr.toml`, which the owner edits
- OAuth, OIDC, LDAP or forward-auth headers — because one local user is enough at home; a reverse proxy can add more in front
- The API key in a query parameter — because a key in a URL ends up in proxy and browser logs; a v3 shim that needs it brings its own key (D8)
- TLS — because a reverse proxy terminates it
- The login page itself — because it is part of spec `web-ui`; this spec provides the endpoints

## Acceptance criteria

- [ ] AC-01 (state): While auth is off, every endpoint answers without a credential; login and logout answer success without effect, and "who am I" reports that auth is off.
- [ ] AC-02 (state): While auth is on, a request to any endpoint other than the open endpoints without a credential, and not from a trusted network, answers status 401 with a German message and changes nothing.
- [ ] AC-03 (event): When the user logs in with the configured username and password, then sofarr answers with a session cookie that the browser cannot read from scripts and does not send with cross-site requests; requests carrying it are served.
- [ ] AC-04 (event): When a request carries the configured API key in the `X-Api-Key` header, then it is served; a request whose only credential attempt is a wrong or empty key answers status 401.
- [ ] AC-05 (event): When the user logs out, then the session is invalid at once and requests carrying its cookie answer status 401; a logout without a valid session also answers success.
- [ ] AC-06 (state): While a session authenticates a served request at least once every `session_days` days, it stays valid; a session unused for longer answers status 401.
- [ ] AC-07 (always): Sessions survive a restart of sofarr; a change of the configured username or password hash invalidates every existing session.
- [ ] AC-08 (error): If a login fails, then sofarr answers status 401 with the same German message whether the username or the password was wrong.
- [ ] AC-09 (error): If 5 logins from the same client address failed in a row, then further logins from that address answer status 429 with a German message saying when to retry and the seconds to wait in a `Retry-After` header — even with the right password — for 1 minute; attempts during this wait are not checked and do not extend it; every failed attempt after a wait doubles the next wait, up to 15 minutes; a successful login resets the count. This holds for trusted networks too, and concurrent attempts never let more than 5 failures through before the first wait.
- [ ] AC-10 (option): Where `[auth] trusted_proxies` lists the peer address of a request, the client address is the rightmost address in `X-Forwarded-For` (all such headers in order) that is not itself a trusted proxy — the leftmost if all are — and the peer address if the header is missing or any entry of it is not a valid address; from any other peer `X-Forwarded-For` is ignored.
- [ ] AC-11 (option): Where `[auth] trusted_networks` contains the client address, the request is served without a credential.
- [ ] AC-12 (error): If auth is on and the username or password hash is missing, the password hash is not a valid hash, `api_key` is set but empty, `session_days` is below 1, or an entry of `trusted_networks` or `trusted_proxies` is not a valid address or range, then sofarr does not start and names the key in a German error message.
- [ ] AC-13 (event): When the user runs the CLI command to hash a password and types it (or pipes it on stdin), then sofarr prints a password hash that, put into `sofarr.toml`, makes AC-03 succeed with that password; the password never appears in the command line or the output; an empty password is refused with a German message (exit code ≠ 0).
- [ ] AC-14 (always): An unexpected server error — e.g. the database, the file system or an upstream service failing, including the session store during authentication — answers status 500 (endpoints whose spec names another status for a case, such as 503, keep it) with a generic German message that contains no internal details (file paths, SQL, upstream URLs, error chains); the details go to the log. This holds whether auth is on or off.
- [ ] AC-15 (always): Passwords, password hashes, the API key and session tokens never appear in the log.
- [ ] AC-16 (state): While auth is on, `amigo-sofarr healthcheck` and the container health check keep working without a credential.

## Technical notes

- Config: `[auth] enabled` (default false), `username`, `password_hash` (Argon2id, PHC string),
  `api_key` (optional; without it scripts need a session or a trusted network),
  `session_days` (default 30), `trusted_networks` and `trusted_proxies` (CIDR lists, default empty).
- Endpoints: `POST /api/auth/login` (JSON `username`, `password`), `POST /api/auth/logout`,
  `GET /api/auth/me` (whether auth is on, whether the request is authenticated and how). Open:
  `GET /api/health`, `POST /api/auth/login`, `POST /api/auth/logout`, `GET /api/auth/me`, UI assets.
- Cookie: `HttpOnly`, `SameSite=Strict`, `Path=/`, `Secure` when the request came over HTTPS
  (directly or per `X-Forwarded-Proto` from a trusted proxy).
- Sessions: a table with the SHA-256 of a random 256-bit token, created and last-used time, and a
  fingerprint of username + password hash (AC-07). The API key is compared in constant time.
- CLI: `amigo-sofarr hash-password` (reads without echo from a terminal, else one line from stdin).
- Login throttling is kept in memory; a restart resets it. An entry is dropped 15 minutes after
  its last failed attempt or the end of its wait, whichever is later; at most 10,000 addresses
  are kept (the oldest go first).
- AC-14 and AC-15 are proven by tests that provoke each error source (database, file system,
  upstream HTTP) and scan the response and the captured log.
- New dependencies (workspace): `argon2`, `rand`, `subtle`, `sha2` – a stopping point for `spec-build`
  only if the owner objects here.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Auth is switched on in an existing setup | API clients without a key get 401 until configured; the data is untouched |
| The same user is logged in on phone and laptop | Two independent sessions; logging out on one keeps the other |
| A request has both a valid session and a wrong API key | Served — one valid credential is enough |
| A request from a trusted network with a wrong API key | Served — the trusted network needs no credential |
| `X-Forwarded-For` with several addresses from a trusted proxy chain | The rightmost non-proxy address counts (AC-10) |
| A client without a trusted proxy fakes `X-Forwarded-For: 192.168.1.5` | Ignored; its peer address counts (AC-10) |
| 5 failed logins from one address, then a login from another address | The other address is not throttled |
| A login body that is not JSON or misses a field | Status 400 with a German message; counts as a failed attempt |
| A trusted proxy sends no `X-Forwarded-For` | The proxy's own address counts (AC-10) |
| IPv6 client addresses and IPv4-mapped IPv6 (`::ffff:192.168.1.5`) | Matched against the ranges as the IPv4 address |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Sessions are sliding: 30 days after the last use | The owner opens the UI irregularly; a fixed lifetime would log out mid-use | A stolen cookie stays valid while used |
| 5 failed attempts, then 1 minute doubling up to 15 minutes | Stops password guessing without locking the owner out for long | An attacker on the same address as the owner (NAT) delays the owner's login |
| Login throttling resets on restart | No persistent state for a rare case | A restart lets an attacker try 5 more passwords |
| Per-address throttling is enough | sofarr runs at home or behind the owner's proxy; Argon2id makes each guess expensive | An attacker with many IPv4 addresses, or one that fills the 10,000 entries, gets more guesses |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |
| AC-07 |      | open   |        |
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |

## Decisions

- 2026-11-12: Login plus API key, switchable in `sofarr.toml` (owner; D15).
- 2026-11-12: Optional trusted networks need no credential; trusted proxies decide whose `X-Forwarded-For` counts (owner).
- 2026-11-12 (revision): AC-09 also sends the wait as `Retry-After`, because the login page of spec `web-ui-shell` disables the button for exactly that time.
