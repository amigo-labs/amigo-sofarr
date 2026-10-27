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

## Docs

- [`docs/specs/sofarr.md`](docs/specs/sofarr.md) — specification and decision log
- [`PLAN.md`](PLAN.md) — phased implementation plan

## License

[MIT](LICENSE) © amigo-labs
