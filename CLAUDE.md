# CLAUDE.md – amigo-sofarr

amigo-sofarr ist ein kombinierter Ersatz für Sonarr + Radarr in Rust (amigo-labs, MIT).
Die Spec steht in `docs/specs/sofarr.md`, der aktuelle Arbeitsauftrag in `PLAN.md`.
Arbeite immer nur an der Phase, die in `PLAN.md` als aktiv markiert ist.

## ⛔ Clean Room – nicht verhandelbar

sofarr ist MIT-lizenziert. Sonarr, Radarr und alle anderen Servarr-Projekte sind GPL-3.0.
Jede Übernahme ihres Codes – auch übersetzt, umformuliert oder „inspiriert“ – würde sofarr
zu einem abgeleiteten Werk machen. Deshalb gilt ohne Ausnahme:

- **Niemals** Quellcode von Sonarr, Radarr, Lidarr, Readarr, Prowlarr, Whisparr oder anderen
  Servarr-Projekten lesen, klonen, fetchen, durchsuchen oder zitieren. Das gilt für GitHub,
  Mirrors, Gists, Blogposts mit Code-Ausschnitten und Stack-Overflow-Antworten mit deren Code.
- **Niemals** deren Regexes, Algorithmen oder Teststrings aus dem Gedächtnis rekonstruieren.
  Wenn dir „die Sonarr-Regex für X“ einfällt: nicht verwenden. Löse das Problem selbst.
- **Niemals** `[TestCase(...)]`-Daten aus deren Testsuites übernehmen.

**Erlaubt** sind:

- Antworten der HTTP-APIs laufender Instanzen (Black-Box-Beobachtung), insbesondere die
  Fixtures unter `fixtures/` – sie sind das Orakel.
- Die öffentliche Nutzer-Doku auf `wiki.servarr.com` (Konzepte, Begriffe, Verhalten) und die
  API-Doku (Endpoints, Feldnamen).
- Allgemeines Wissen über Release-Namenskonventionen (Scene-Regeln, Quellen, Codecs).

Die Feldstruktur der Orakel-Antworten lernst du aus den Fixture-Dateien selbst, nicht aus
deren Quellcode. Wenn du unsicher bist, ob etwas erlaubt ist: **anhalten und fragen.**

## Architekturregeln

- Abhängigkeitsrichtung: `parser` ← `core` ← (`indexer`, `metadata`, `download`, `db`) ← `server`.
  `sofarr-parity` darf alles nutzen, aber nichts darf von `sofarr-parity` abhängen.
- `sofarr-parser` und `sofarr-core` sind **pure**: kein IO, kein async, kein Netzwerk,
  keine Systemzeit (Zeit wird als Parameter übergeben).
- Tokio-Runtime, HTTP-Server und Scheduler leben nur in `sofarr-server`.
- Fehler: `thiserror` in Libraries, `anyhow` nur in Binaries (`sofarr-server`, `sofarr-parity`).
- Kein `unsafe` (workspace-weit `forbid`).
- Neue Dependencies immer über `[workspace.dependencies]` im Root-`Cargo.toml`.

## Konventionen

- Code, Bezeichner, Kommentare und Log-Messages: Englisch.
- Nutzersichtbare Texte (CLI-Ausgaben, Fehlermeldungen an den Nutzer, später die Web-UI): Deutsch.
- Commits: Conventional Commits, Scope = Crate ohne Präfix, z. B. `feat(parser): detect remux`,
  `test(parity): add rss collector fixtures`.

## Gates (vor jedem Commit, identisch zur CI)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Eine Phase ist erst fertig, wenn alle Gates grün sind **und** die phasenspezifischen Gates aus
`PLAN.md` erfüllt sind.

## Secrets & private Daten

- `fixtures/private/` und `config/sofarr.toml` sind gitignored und enthalten API-Keys,
  Passwörter und die Bibliothek des Nutzers. Niemals committen, niemals in Tests hart verdrahten.
- Tests, die private Fixtures brauchen, müssen sauber skippen, wenn die Dateien fehlen (CI hat sie nicht).
