# AGENTS.md – amigo-sofarr

amigo-sofarr ist ein kombinierter Ersatz für Sonarr + Radarr in Rust (amigo-labs, MIT).
Die Design-Spec steht in `docs/specs/sofarr.md`, die Feature-Specs und ihre Reihenfolge in
`docs/specs/README.md`. Gearbeitet wird spec-driven (Abschnitt „Spec-driven workflow“ unten).

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
- Dasselbe gilt für andere Copyleft-Projekte derselben Domäne, insbesondere **HolyArrs**
  (AGPL-3.0) und **Bazarr** (GPL-3.0): kein Code, keine Tests, keine Regexes.

**Erlaubt** sind:

- Antworten der HTTP-APIs laufender Instanzen (Black-Box-Beobachtung), insbesondere die
  Fixtures unter `fixtures/` – sie sind das Orakel.
- Die öffentliche Nutzer-Doku auf `wiki.servarr.com` (Konzepte, Begriffe, Verhalten) und die
  API-Doku (Endpoints, Feldnamen). Ebenso die Nutzer-Doku von HolyArrs und Bazarr
  (nur Konzepte, nie Code-Ausschnitte daraus).
- Öffentliche Spezifikationen und Provider-Dokus (Matroska/RFC 9559, ISO-BMFF, Newznab,
  TMDB, TVDB, OpenSubtitles.com, IMDb-Datasets, Jellyfin-Benennungsregeln).
- Allgemeines Wissen über Release-Namenskonventionen (Scene-Regeln, Quellen, Codecs).

Die Feldstruktur der Orakel-Antworten lernst du aus den Fixture-Dateien selbst, nicht aus
deren Quellcode. Wenn du unsicher bist, ob etwas erlaubt ist: **anhalten und fragen.**

## Architekturregeln

- Abhängigkeitsrichtung: `parser` ← `core` ← (`indexer`, `metadata`, `download`, `db`) ← `server`;
  `parser` ← `subtitles` ← `server` (D25).
  `sofarr-parity` darf alles nutzen, aber nichts darf von `sofarr-parity` abhängen.
  `sofarr-mediainfo` hängt von nichts aus sofarr ab und wird nur von `sofarr-server` genutzt
  (D22); es arbeitet auf `Read + Seek`, ohne eigenen Dateizugriff.
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
- Abweichungen vom Orakel sind entweder behoben oder als „Bewusste Abweichung“ bzw.
  „Annahme“ in `docs/parity.md` dokumentiert – nie stillschweigend.

## Gates (vor jedem Commit, identisch zur CI)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Die Befehle stehen maschinenlesbar in `.agents/gates.json` (`stop`, `fast`, `full`); die
Claude-Code-Hooks führen sie aus. Eine Feature-Spec ist erst fertig, wenn `spec-check` sie
abnimmt.

## Secrets & private Daten

- `fixtures/private/` und `config/sofarr.toml` sind gitignored und enthalten API-Keys,
  Passwörter und die Bibliothek des Nutzers. Niemals committen, niemals in Tests hart verdrahten.
- Tests, die private Fixtures brauchen, müssen sauber skippen, wenn die Dateien fehlen (CI hat sie nicht).

## Spec workflow profile

Die Projektfakten, die die gemeinsamen Spec-Skills (`spec`, `spec-build`, `spec-check`,
`spec-extract`, `spec-fix`, `qa`) unter diesen Schlüsseln nachschlagen.

- **Design spec** — `docs/specs/sofarr.md` (`status: draft`): Ziel, Scope (§2), Architektur (§3),
  Domänenmodell (§4), Parity-Strategie (§5), Clean Room (§6), Betrieb (§7), Integrationen (§8),
  Entscheidungslog (§9). Weiteres Design-Dokument: `docs/parity.md` (Parity-Matrix, bewusste
  Abweichungen, Stand der Orakel, Annahmen ohne Orakel).
- **Spec index** — `docs/specs/README.md`.
- **Decisions** — eine Zeile im Entscheidungslog `docs/specs/sofarr.md` §9 (fortlaufend `D<n>`);
  eine bewusste Abweichung von Radarr/Sonarr zusätzlich in `docs/parity.md`
  („Bewusste Abweichungen“), eine Annahme ohne Orakel in „Annahmen ab Phase 3“.
- **Code map** — `crates/sofarr-parser` (Release-Namen: Tokenizer, Klassifikation, Titel,
  Episoden), `crates/sofarr-core` (Qualities, Profile, Custom Formats, Decision Engine,
  Upgrades, Verfügbarkeit, Naming, Serien-Regeln), `crates/sofarr-indexer` (Newznab),
  `crates/sofarr-metadata` (TMDB, TVDB), `crates/sofarr-download` (`DownloadClient`, nzbget),
  `crates/sofarr-db` (SQLite, Migrationen unter `migrations/`), `crates/sofarr-mediainfo`
  (Media-Infos aus Matroska- und MP4-Headern), `crates/sofarr-subtitles` (OpenSubtitles.com-Client,
  Movie-Hash, Ranking der Kandidaten, UTF-8, Jellyfin-Untertitelnamen), `crates/sofarr-server`
  (Binary `amigo-sofarr`: Config `config.rs`, Scheduler und Pipeline `app.rs`, API `api.rs`,
  RSS `rss.rs`, Import `importer.rs`, Festplatten-Scan `rescan.rs`, Serien `series.rs`,
  Media-Infos `media.rs`, Untertitel `subtitles.rs`, Import Lists, Orakel-Abgleiche `compare.rs` und `compare_media.rs`),
  `crates/sofarr-parity` (Orakel-Fixtures sammeln,
  Matrix, Diff), `fixtures/` (Orakel), `Dockerfile`, `compose.example.yml`, CI unter
  `.github/workflows/`.
- **Spec questions** — Filme, Serien oder beide? Welches Orakel belegt das Verhalten (Endpoint
  aus `sofarr.md` §5), oder ist es eine Annahme ohne Orakel? Verhalten im Shadow-Mode gegenüber
  dem Live-Modus? Neue Config-Keys in `sofarr.toml` (unbekannte Keys sind ein Fehler) mit
  Default? Neue API-Endpunkte oder CLI-Befehle? Nutzersichtbare Texte auf Deutsch. Vorschlag
  der Crate-Aufteilung gemäß Abhängigkeitsrichtung, z. B. „Regel pure in `sofarr-core`, IO in
  `sofarr-server`“. Front matter: `media: {movies | series | both | none}`,
  `oracle: {<endpoint> | none}`.
- **AC subject** — „sofarr …“.
- **Performance AC example** — „Ein RSS-Sync entscheidet 1.000 Releases gegen 2.500 Filme in
  unter 2 s (`cargo test -p sofarr-server --release rss_sync_large_library`, gemessene Dauer
  im Testoutput)“.
- **Tests** — Unit-Tests inline (`#[cfg(test)]`); Integrationstests unter `crates/*/tests/`;
  Orakel-Harness mit Ratchet (`sofarr-parser/tests/oracle.rs`, `episode_oracle.rs`,
  `sofarr-parity/tests/release_oracle.rs`; Baselines `fixtures/*-baseline.toml` dürfen nur
  steigen); End-to-End-Tests im Server mit Fake-Diensten als axum-Router (Newznab, nzbget
  JSON-RPC; Muster in den Tests von `app.rs`); Tests mit privaten Fixtures skippen sauber.
  Roter Einzellauf: `cargo test -p <crate> <test_name>`. `manual:` nur für Betrieb gegen
  echte Instanzen (Shadow-/Live-Betrieb, `compare-*` gegen laufendes Radarr/Sonarr, Anzeige
  in Jellyfin), festgehalten als `manual: {wer} prüfte {was}, {Datum}`.
- **CI-only checks** — der Docker-Build (Job `docker` in `.github/workflows/ci.yml`). Ein AC,
  das nur er belegt, trägt `CI: docker` hinter seiner Form; lokal prüft `docker build .`, wo
  Docker installiert ist.
- **Build check** — `cargo check --workspace --locked`.
- **Conventions** — die Abschnitte „Clean Room“, „Architekturregeln“, „Konventionen“ und
  „Secrets & private Daten“ dieser Datei. Fertig heißt: Gates grün, jedes AC mit Evidence,
  Orakel-Abweichungen behoben oder in `docs/parity.md` dokumentiert.
- **Stopping points** — jeder Zweifel am Clean Room; eine neue Dependency; eine Änderung der
  Crate-Abhängigkeitsrichtung; eine Ratchet-Baseline, die sinken müsste; eine Abweichung vom
  Orakel, die weder behebbar noch als bewusst begründbar ist; eine Config- oder API-Änderung,
  die eine bestehende `sofarr.toml` oder einen Client bricht; eine DB-Migration, die Daten
  verwirft; eine Entscheidung, die in `sofarr.md` §9 fehlt; ein AC, das sich als falsch erweist.
- **Sizing** — eine Session baut etwa einen M-Block (~400 geänderte Zeilen); was größer ist,
  wird vorher in mehrere Specs geteilt.
- **Commits** — Conventional Commits, Scope = Crate ohne Präfix (`parser`, `core`, `indexer`,
  `metadata`, `download`, `db`, `server`, `parity`), dazu `spec`, `ui`, `docker`. Ein Block, der
  mehrere Crates berührt, ist eine Commit-Serie, eine pro Scope. Fixtures und Baselines in
  eigenen `test(...)`-Commits; private Fixtures nie. Beispiel:
  `feat(db): blocklist for failed releases (AC-01, AC-02)`.
- **Language** — Specs und Commit-Messages auf Englisch (Default der Skills), ebenso Code,
  Kommentare und Log-Messages; nutzersichtbare Texte (CLI, Fehlermeldungen, Web-UI) auf Deutsch.
- **Branches** — `main` ist geschützt (CI auf Pull Requests); Feature-Branch `feat/<spec-name>`.

<!-- amigo-skills:begin spec-workflow -->
<!-- Managed by .agents/sync.mjs from amigo-labs/skills (agents-md/). Edit it there, not here. -->

## Spec-driven workflow

Features are built spec-driven: first a feature spec `docs/specs/<name>.md` with testable acceptance criteria, then the implementation with tests, then an independent acceptance check. The project specifics (design spec, where decisions go, code map, tests, commits) are in the **spec workflow profile** of this file; the skills read it.

The skills live in `.agents/skills/` (read by Codex, Gemini CLI, Copilot, Cursor, OpenCode and others) and are mirrored to `.claude/skills/` for Claude Code. They are vendored from [amigo-labs/skills](https://github.com/amigo-labs/skills): change them there and run `node .agents/sync.mjs`, never edit the copies here. Skills only this repository has live in `.agents/skills/` too and are mirrored the same way.

| Skill | Purpose |
| ----- | ------- |
| `spec <name>` | Write or revise a feature spec with acceptance criteria and a clarification pass |
| `spec-build <name>` | Implement a feature spec test-first, one commit series per AC block, independent verifier loop |
| `spec-check <name>` | Independent acceptance against a full gate run; only it sets `status: implemented` |
| `spec-extract` | Spec from existing code, or current-state analysis of an open spec |
| `spec-fix <bug>` | Bug fix: root cause, regression test that is red first, minimal fix |
| `qa` | Full gate run (`.agents/gates.json`, mirrors CI) |

An agent that cannot invoke skills by name reads `.agents/skills/<name>/SKILL.md` and follows it. At the start of a task, an agent without a session-start hook runs `node .agents/skills/qa/scripts/open-specs.mjs` to see the open feature specs.

Two kinds of spec live in `docs/specs/`:

| Kind | Status | Owns |
| ---- | ------ | ---- |
| Design spec (named in the profile) | `status: draft`, no acceptance criteria | Architecture, data model, the decisions |
| Feature spec `docs/specs/<name>.md` | `status: todo` / `implemented`, `## Acceptance criteria` | One feature: what it does, how we prove it |

A feature spec references the design-spec sections it builds on and never contradicts them. A feature that needs an architectural decision (cross-crate or cross-package boundary, file format, new dependency, public API shape) gets a decision where the profile says, in the same commit as the spec. The spec index (profile **Spec index**, default `docs/specs/README.md`) lists the feature specs: order, dependencies, done, and the backlog of gaps not yet written as specs.

### Triage

Classify every new input before touching code:

| Input | Approach |
| ----- | -------- |
| New feature or changed behavior ("I want …", "it should …") | Start `spec`, no code before the spec. If the user says "just do it", implement directly. |
| Request touches an existing feature spec | Revise that spec in `spec`'s revision mode, never create a second one |
| Unclear whether small change or feature | Exactly one question: "Capture as spec" / "Implement directly" |
| Bug fix | `spec-fix`: directly, always with a regression test that is red before the fix. If the fix changes specified behavior, revise the spec too. |
| Refactoring, tooling, docs, questions | Directly, no spec |
| "Carry on", "build X", "implement the spec" | `spec-build <name>` |
| "Is X done?", acceptance after manual implementation | `spec-check <name>` |
| Document existing behavior | `spec-extract <area>` |

Only `spec-check` sets a spec to `status: implemented` — never by hand.

### Keeping knowledge

Project knowledge belongs in the repository, not on one machine:

- **Pitfalls and non-obvious knowledge** → `## Gotchas` in this file. Only what cannot be read from code or git history; no noise.
- **Architecture decisions** → where the profile's **Decisions** entry says.
- **Feature-local decisions** → `## Decisions` of the feature spec, with the reason.
- **Progress of a feature** → `## Evidence` of the feature spec (there is no separate status file).
- **Never** into an agent's private memory (e.g. Claude Code auto memory): it stays on one machine.

### Quality gates

- **Run the `qa` skill before finishing any coding task** and report the result. The gate commands are in `.agents/gates.json`: `stop` (seconds: formatting, clippy on the changed crates — the Claude Code Stop hook runs it after every turn that changed code), `fast` (before every commit — the Claude Code commit gate refuses a commit while one is red; other agents run `node .agents/skills/qa/scripts/gates.mjs fast` before committing) and `full` (mirrors CI, `node .agents/skills/qa/scripts/gates.mjs full`).
- **Fix the cause, never lower the gates.** No suppressions (the list is `suppressions` in `.agents/gates.json`), no weakened, skipped or ignored tests. The Claude Code guard hook refuses new suppressions; a justified one is added by the user.
- **Don't loosen the gate configuration**: `.agents/gates.json`, `.agents/skills/qa/scripts/`, `.claude/settings.json` and `.claude/hooks/` change only on the user's explicit request.
- Where a gate names a `fix` command (formatting, lint autofix), that fix is allowed; everything else is fixed in the code. Where a gate has a `note`, follow it.
- `STOP_GATE=off` / `COMMIT_GATE=off` in the environment switch the Claude Code Stop hook / commit gate off, e.g. for a bulk migration — only on the user's request.

<!-- amigo-skills:end spec-workflow -->

## Gotchas

- Lokal wird unter Windows entwickelt, CI und Container laufen unter Linux. Plattformcode
  braucht eine Crate, die beides kann (freier Speicher: `fs4`, nicht `rustix::fs`).
- Relative Pfade in der DB (`relative_path`) sind immer `/`-getrennt wie die aus Radarr/Sonarr
  importierten (`importer::relative_path`); `Path::to_string_lossy` liefert unter Windows `\`.
- Tests, die Host-Pfade in TOML einsetzen, brauchen Literal-Strings (`'…'`), in JSON escapte
  Pfade – sonst sind Windows-Backslashes Escape-Sequenzen.
- `*.jsonl` sind per `.gitattributes` LF, damit angehängte Fixture-Zeilen byte-identisch bleiben.
- Custom-Format-Regexes mit Lookbehind am Anfang einer Alternative wertet `sofarr_core::pattern`
  selbst aus; fancy-regex matcht z. B. `(?<=\bGerman\b.*)` nie.
- GitHub Code Scanning (CodeQL) meldet jedes String-Literal, das als Passwort oder Schlüssel
  benutzt wird, als kritisches „hard-coded cryptographic value“ – auch in Tests. Test-Passwörter
  entstehen deshalb zur Laufzeit (`crates/sofarr-server/src/test_secret.rs`).
