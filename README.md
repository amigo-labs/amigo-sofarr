# amigo-sofarr

Manage your movies and TV shows from the sofa: one lightweight service that finds new releases
on Usenet indexers, decides based on your quality rules, hands them to your download client,
and imports them into your library.

Written in Rust. Single static binary, single container, SQLite.

> **Status:** early development. Not usable yet. See [`PLAN.md`](PLAN.md).

## Not affiliated with Servarr

amigo-sofarr is an **independent project**. It is not part of, endorsed by, or derived from
Sonarr, Radarr, or any other Servarr project. It is developed as a **clean-room implementation**:
no Servarr source code or test data is used. Existing instances are only observed through their
public HTTP API to verify behaviour. See [`CLAUDE.md`](CLAUDE.md) for the rules.

## Quick start (development)

```sh
cargo run -p sofarr-server -- --config config/sofarr.example.toml
curl localhost:7676/api/health
```

Docker:

```sh
docker build -t amigo-sofarr .
docker run --rm -p 7676:7676 amigo-sofarr
```

See [`compose.example.yml`](compose.example.yml) for running it next to an existing setup.

## Parity tooling

sofarr is measured against a running Radarr instance as a black-box oracle (HTTP API only).
Run these locally, where Radarr and your indexers are reachable:

```sh
# 1. Export your Radarr/Sonarr config (contains secrets – lands in gitignored fixtures/private/)
RADARR_URL=http://<host>:7878 RADARR_API_KEY=... ./scripts/export-arr-config.sh
# 2. Generate the secret-free parity matrix docs/parity.md from it
cargo run -p sofarr-parity -- matrix
# 3. Collect parser fixtures (config: copy config/parity.example.toml to fixtures/private/parity.toml)
cargo run -p sofarr-parity -- collect
# 4. Compare the parser against the oracle
cargo test -p sofarr-parser --test oracle -- --nocapture
```

## Docs

- [`docs/specs/sofarr.md`](docs/specs/sofarr.md) — specification and decision log
- [`PLAN.md`](PLAN.md) — phased implementation plan

## License

[MIT](LICENSE) © amigo-labs
