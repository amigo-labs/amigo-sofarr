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
- [ ] AC-10 (option): Where `[auth] trusted_proxies` lists the peer address of a request, the client address is the rightmost address in `X-Forwarded-For` (all such headers in order) that is not itself a trusted proxy — the leftmost if all are — and the peer address if the header is missing; if, reading from the right, an entry that is not a valid address comes before any address that is not a trusted proxy, the client address is unknown: it lies in no trusted network, and the login throttling counts it under the peer address. From any other peer `X-Forwarded-For` is ignored.
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
| A client behind a trusted proxy in a trusted network sends `X-Forwarded-For: garbage` | The client address is unknown: a credential is needed (AC-10) |
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
| AC-01 | `auth_tests::auth_off_serves_everything` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-02 | `auth_tests::auth_on_refuses_requests_without_a_credential` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-03 | `auth_tests::login_gives_a_strict_http_only_session_cookie`, `auth_tests::the_cookie_is_secure_behind_an_https_proxy` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-04 | `auth_tests::the_api_key_header_authenticates` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-05 | `auth_tests::logout_ends_only_its_session` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-06 | `auth_tests::sessions_slide_and_expire` (`crates/sofarr-server/src/auth_tests.rs`); `auth_tests::using_a_session_renews_its_cookie` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-07 | `auth_tests::sessions_survive_a_restart_but_not_a_new_password` (`crates/sofarr-server/src/auth_tests.rs`); `sessions::tests::sessions_round_trip` (`crates/sofarr-db/src/sessions.rs`); `auth_tests::a_new_username_ends_every_session` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-08 | `auth_tests::failed_logins_look_alike` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-09 | `auth_tests::five_failures_make_the_address_wait`, `auth_tests::concurrent_logins_never_pass_more_than_five_failures` (`crates/sofarr-server/src/auth_tests.rs`); `throttle::tests::five_failures_then_doubling_waits`, `throttle::tests::entries_are_forgotten_and_capped` (`crates/sofarr-server/src/throttle.rs`); `auth_net::tests::throttling_counts_ipv6_by_its_64` (`crates/sofarr-server/src/auth_net.rs`); `auth_tests::attempts_during_a_wait_do_not_extend_it`, `auth_tests::a_successful_login_resets_the_count` (`crates/sofarr-server/src/auth_tests.rs`); `auth_tests::unknown_clients_are_throttled_under_the_proxy` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-10 | `auth_net::tests::client_address_behind_trusted_proxies` (`crates/sofarr-server/src/auth_net.rs`); `auth_tests::trusted_networks_and_proxies` (`crates/sofarr-server/src/auth_tests.rs`); `auth_tests::an_invalid_forwarded_entry_is_no_trusted_client` (`crates/sofarr-server/src/auth_tests.rs`); `auth_tests::unknown_clients_are_throttled_under_the_proxy` (`crates/sofarr-server/src/auth_tests.rs`); `auth_net::tests::an_unknown_client_is_throttled_under_its_peer` (`crates/sofarr-server/src/auth_net.rs`) | built |        |
| AC-11 | `auth_tests::trusted_networks_and_proxies` (`crates/sofarr-server/src/auth_tests.rs`); `auth_net::tests::ranges_contain_addresses` (`crates/sofarr-server/src/auth_net.rs`) | built |        |
| AC-12 | `auth_tests::a_bad_auth_config_stops_the_start` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-13 | `hashes_a_piped_password`, `refuses_an_empty_password` (`crates/sofarr-server/tests/cli_auth.rs`); `auth::tests::password_hashes_verify` (`crates/sofarr-server/src/auth.rs`); `works_while_the_auth_config_is_still_incomplete` (`crates/sofarr-server/tests/cli_auth.rs`) | built |        |
| AC-14 | `auth_tests::server_errors_answer_generically_and_log_the_details`, `auth_tests::upstream_errors_name_no_urls` (`crates/sofarr-server/src/auth_tests.rs`); both tests also scan the captured log for the details of each error source (database, file system, upstream HTTP); `api_tests::marking_needs_the_download_client` (`crates/sofarr-server/src/api_tests.rs`) | built |        |
| AC-15 | `auth_tests::secrets_never_reach_the_log` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |
| AC-16 | `auth_tests::health_needs_no_credential` (`crates/sofarr-server/src/auth_tests.rs`) | built |        |

## Decisions

- 2026-11-12: Login plus API key, switchable in `sofarr.toml` (owner; D15).
- 2026-11-12: Optional trusted networks need no credential; trusted proxies decide whose `X-Forwarded-For` counts (owner).
- 2026-11-12 (revision, owner): AC-10 no longer falls back to the peer address on an invalid `X-Forwarded-For` entry. Behind a trusted proxy the peer is the proxy, which often lies in a trusted network: a client could otherwise send `X-Forwarded-For: garbage` and need no credential. An invalid entry before the first non-proxy address (read from the right) makes the client address unknown.
- 2026-11-12 (revision): AC-09 also sends the wait as `Retry-After`, because the login page of spec `web-ui-shell` disables the button for exactly that time.
- 2026-11-12: Tests and code were written together; the red run was the failing build. As a counter-check the core behavior was switched off one piece at a time (no guard, no expiry or fingerprint check, trusted networks ignored, no `HttpOnly`/`SameSite`, failures not counted, `X-Forwarded-For` ignored, error details in 500 answers): 11 tests went red.
- 2026-11-12: `amigo-sofarr hash-password` reads without echo in a terminal through `rpassword` (owner, 2026-11-12: a new dependency beyond the four the spec names, because reading without echo needs the terminal API and `unsafe` is forbidden); piped input is read as one line. The terminal path is not covered by an automated test (no terminal in the test runner).
- 2026-11-12: Logins are checked one at a time (one lock for all addresses): concurrent attempts cannot pass the count before the first wait, and Argon2id makes each check take a few milliseconds only.
- 2026-11-12: Address ranges are parsed by a small own CIDR type (`auth_net.rs`) instead of a new crate such as `ipnet`.
- 2026-11-12: The middleware sets the client address for every request; without a connection address (only in tests that serve without `ConnectInfo`) the peer counts as `0.0.0.0`.
- 2026-11-12: An unexpected server error answers status 500 with the generic "Interner Fehler – Details stehen im Log". An operation whose spec gives it its own failure message (the backup API: "Sicherung … nicht gelöscht – Details stehen im Log" and the like) says what failed, but names no paths, OS errors, SQL or URLs either; the details go to the log and the backup list's last error.
- 2026-11-12: After the first verification round: every request served by a session sets its cookie again with a fresh `Max-Age`, so the browser keeps it as long as the server does (AC-06); `hash-password` runs before the config is read (AC-13); sessions unused for `session_days` are deleted at each login; `GET /api/grabs?status=…` with an unknown status answers 400.
- 2026-11-12: Argon2 and BLAKE2 are compiled optimised in the dev profile, because the tests log in many times.
- 2026-11-12: `AuthConfig` has a hand-written `Debug` that hides username, hash and key, so no debug print of the config can put them into the log.
- 2026-11-12: Marking a grab as failed while the download client is unreachable answers 503 with the fixed text "Download-Client nicht erreichbar – Details stehen im Log"; the upstream error is logged (AC-14).
- 2026-11-12: `[auth] session_days` is limited to 1..=3650, because far larger values overflow the session timestamps (AC-12).

