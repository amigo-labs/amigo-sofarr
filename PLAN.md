# PLAN – amigo-sofarr

> Arbeitsauftrag für Claude Code. Spec: `docs/specs/sofarr.md`. Regeln: `CLAUDE.md`.
> Nur an der als **AKTIV** markierten Phase arbeiten. Phasen-Gates sind hart.
> 👤 = Daniel, 🤖 = Claude Code.

---

## Phase 0 – Gerüst ✅ (Scaffold geliefert)

Workspace mit allen Crates (Stubs), `sofarr-server` mit Config, `GET /api/health`,
`healthcheck`-Subcommand, Graceful Shutdown, Dockerfile (`scratch`), CI, Export-Script.

- [ ] 👤 Push auf `main`, CI grün (fmt, clippy, test, Docker-Build)
- [ ] 👤 `compose.example.yml` in die eigene Compose übernehmen, Container startet, Status `healthy`

**Gate:** CI grün, Container läuft neben Radarr.

---

## Phase 1 – Parity-Fundament + Parser v0 ✅

Ziel: Ein wachsendes, echtes Orakel-Fixture-Set und ein eigener Parser, der dagegen gemessen wird.

### 1.1 Config-Export und Parity-Matrix

- [x] 👤 `scripts/export-arr-config.sh` ausführen → `fixtures/private/config/`
- [x] 🤖 `docs/parity.md` aus den Exporten erstellen (Generator fertig: `cargo run -p sofarr-parity -- matrix`;
      Gerüst committet – erzeugt wird lokal, weil die Exporte Secrets enthalten): Welche Features nutzt das Setup tatsächlich?
      (Quality Profiles inkl. Cutoff/Upgrade, Custom Formats inkl. verwendeter Bedingungstypen,
      Naming-Tokens, Indexer-Typen, Download-Client, Notifications, Delay Profiles, Tags.)
      Spalten: Feature · genutzt in · Phase · Status. **Keine Secrets, keine URLs, keine Keys**
      in `parity.md` – die Datei wird committet.

### 1.2 `sofarr-parity collect`

- [x] 🤖 Konfiguration über `fixtures/private/parity.toml` (gitignored, Beispiel als
      `config/parity.example.toml` committen): Radarr-URL + Key, Liste von Newznab-Indexern (URL + Key).
- [x] 🤖 Quellen:
  - **RSS:** pro Indexer die neuesten Releases der Film-Kategorien (Newznab `t=search`, `cat=2000`-Familie)
    → `fixtures/public/radarr-parse.jsonl`
  - **History:** `sourceTitle` aus Radarrs `/api/v3/history` (paginiert)
    → `fixtures/private/radarr-parse-history.jsonl`
- [x] 🤖 Für jeden neuen Titel `GET /api/v3/parse?title=…` gegen Radarr, Antwort **roh** speichern.
      Eine JSON-Zeile pro Titel:
      ```json
      {"title": "...", "origin": "rss", "collected_at": "2026-10-27T12:00:00Z",
       "oracle": {"app": "radarr", "version": "<aus /api/v3/system/status>"}, "response": { ... }}
      ```
- [x] 🤖 Idempotent: Deduplizierung über `title`, nur anhängen, Rate-Limit (Default 2 req/s),
      sauberer Abbruch bei Fehlern ohne halbe Zeilen.
- [x] 🤖 In `public`-Fixtures keine Indexer-Namen oder URLs (`origin` nur `rss` bzw. `history`).
- [x] 🤖 Tests: Newznab-XML-Parsing gegen einen **selbst geschriebenen** Beispiel-Feed, Dedupe-Logik.
- [x] 👤 Collector regelmäßig laufen lassen (z. B. täglich per Cron/Compose-Job), bis ≥ 1.000 öffentliche Fixtures da sind.
      (4.952 Fixtures; Titel 99,8 %, Jahr 99,9 %, Source 99,8 %, Resolution 99,9 %)

Vorschlag für Dependencies (nur `sofarr-parity`): `reqwest` (rustls), `roxmltree`, `serde_json`, `jiff` oder `time`.

### 1.3 `sofarr-parser` v0

- [x] 🤖 Öffentliche API: `pub fn parse(title: &str) -> ParsedRelease` – infallibel,
      Unbekanntes wird `None`/`Unknown`, nie Panic.
- [x] 🤖 Felder: Titel, Jahr, Source, Resolution, Modifier (z. B. Remux), Revision
      (Version, Proper, Repack, Real), Edition, Release Group, Sprachen.
- [x] 🤖 Eigener Ansatz, empfohlen: Tokenizer (Trenner `.`, `_`, Leerzeichen, `-`, Klammern)
      → Token-Klassifikation → Bestimmung der Grenze zwischen Titel und Jahr bzw. Metadaten.
      Keine monolithischen Regexes. **Clean Room beachten (`CLAUDE.md`).**
- [x] 🤖 Unit-Tests mit selbst formulierten Beispielen pro Feld (inkl. Randfälle: Jahreszahl im
      Titel, Titel nur aus Zahlen, fehlende Gruppe, Klammer-Jahre).

### 1.4 Orakel-Harness

- [x] 🤖 `crates/sofarr-parser/tests/oracle.rs`: liest alle `fixtures/public/*.jsonl`
      (und `fixtures/private/*.jsonl`, falls vorhanden), bildet die Orakel-Antwort auf einen
      vergleichbaren Erwartungswert ab und vergleicht **pro Feld**.
      Die Struktur der Antwort wird aus den Fixtures selbst abgeleitet – nicht aus Radarr-Code.
- [x] 🤖 Normalisierung: Titel case-insensitiv, Satzzeichen/Trenner vereinheitlicht.
- [x] 🤖 Report auf stdout: pro Feld `pass/total` und Prozent, plus die ersten 20 Fehlschläge
      pro Feld mit Titel, erwartet, erhalten.

### 1.5 Ratchet

- [x] 🤖 `fixtures/parser-baseline.toml`: Pass-Rate pro Feld (Promille), nur über **public** Fixtures.
- [x] 🤖 Test schlägt fehl, wenn eine Rate unter die Baseline fällt.
- [x] 🤖 `SOFARR_UPDATE_BASELINE=1 cargo test -p sofarr-parser --test oracle` schreibt die neue
      Baseline – nur nach oben, nie nach unten. Ausnahme: Hat sich das Fixture-Set geändert
      (Anzahl/Fingerprint in der Baseline), darf neu – auch niedriger – geschrieben werden.
- [x] 🤖 Private Fixtures erscheinen nur im Report, nicht in der Baseline (CI hat sie nicht).

**Gate Phase 1:**

- Standard-Gates grün (`CLAUDE.md`)
- ≥ 1.000 öffentliche Fixtures committet
- Baseline committet; **Titel, Jahr, Source, Resolution ≥ 95 %** bevor Phase 2 beginnt
- 👤 Review: kein Servarr-Code, keine übernommenen Regexes

---

## Phase 2 – Decision Engine + Release-Orakel · **AKTIV**

Ziel: `sofarr-core` trifft für einen Release dieselbe Grab-Entscheidung wie Radarr –
gemessen gegen das Release-Orakel. Umfang laut `docs/parity.md` (nur Filme).

### 2.1 Öffentliche Config-Fixtures

- [x] 🤖 `sofarr-parity export-public`: liest `fixtures/private/config/radarr/` und schreibt
      `qualitydefinition`, `qualityprofile`, `customformat` per **Whitelist** nach
      `fixtures/public/config/radarr/` (keine Keys, URLs, Pfade, Indexer). Test gegen Fake-Export mit Secrets.
- [ ] 👤 `export-public` ausführen, Ergebnis prüfen und committen.

### 2.2 Qualities

- [x] 🤖 Benannte Qualities (id, Name, Source, Resolution, Modifier) in `sofarr-core`,
      nur aus Black-Box-Daten (Parse-Fixtures, Quality Definitions).
- [x] 🤖 Mapping Parser-Quality → benannte Quality; Orakel-Test gegen die Parse-Fixtures (100 %).

### 2.3 Profile, Definitions, Custom Formats

- [x] 🤖 Domänenmodell + Import aus dem v3-JSON (pure, `thiserror`).
- [x] 🤖 Custom Formats: `LanguageSpecification`, `ReleaseGroupSpecification`,
      `ReleaseTitleSpecification` inkl. `negate`/`required`; Score aus dem Profil.

### 2.4 Decision Engine

- [x] 🤖 `decide(…) -> Decision` (approved, Rejections mit permanent/temporär, Quality, CFs, Score);
      Zeit als Parameter.
- [x] 🤖 Regeln: Quality nicht erlaubt, Größe (Definition × Laufzeit), Cutoff erreicht, kein Upgrade
      (Quality → Revision laut `downloadPropersAndRepacks` → CF-Score), CF-Mindest-Score,
      Profil-Sprache, Indexer-Settings (`maximumSize`, `minimumAge`, `retention`; 0 = aus).

### 2.5 Release-Orakel

- [x] 🤖 `sofarr-parity collect --source releases --movies N`: `/api/v3/release?movieId=…` für
      überwachte Filme (sparsam, Default 5 Filme pro Lauf). Roh nach
      `fixtures/private/radarr-release.jsonl`, redigiert nach `fixtures/public/radarr-release.jsonl`
      (Titel, Größe, Alter, Quality, approved, Rejections, CFs, Score, Movie-Kontext ohne Pfade;
      **kein** Indexer, keine GUID, keine URLs).
- [ ] 👤 `collect releases` laufen lassen und public Fixtures committen.

### 2.6 Decision-Ratchet

- [x] 🤖 `sofarr_parity::diff` + `crates/sofarr-parity/tests/release_oracle.rs`: Vergleich pro
      Feld (approved, Rejections, CFs, Score, Quality), Report wie beim Parser, Baseline
      `fixtures/decision-baseline.toml` (gleiche Ratchet-Regeln). Ungemappte
      Orakel-Rejection-Texte erscheinen im Report. Liegt in `parity` statt in `core`, damit
      `diff releases` denselben Code nutzt.
- [x] 🤖 `sofarr-parity diff releases`: derselbe Vergleich inkl. privater Fixtures, nur Report.
- [x] 🤖 Engine nachziehen: Film-Zuordnung (`unknown_movie`), Hardcoded Subs (Parser +
      `allowHardcodedSubs`), Größencheck bestätigt (MB = 1024²). Stand: 53/53 in allen Feldern.
- [x] 🤖 Status in `docs/parity.md` nachziehen.
- [ ] 👤 Releases für überwachte Filme **mit** Datei sammeln (Upgrade, Cutoff, Propers sind
      noch unbelegt), außerdem für das Profil „Ultra-HD“.

**Gate Phase 2:**

- Standard-Gates grün (`CLAUDE.md`)
- Release-Fixtures committet, Baseline committet
- keine unerklärten Abweichungen; bewusste Abweichungen in `docs/parity.md`
- 👤 Review: kein Servarr-Code, keine übernommenen Regexes

## Phase 3 – Bibliothek + Shadow-Mode

- `sofarr-db` (SQLite, Migrations), `sofarr-metadata` (TMDB, inkl. deutscher Titel für
  `{Movie Title:DE}`/`{Movie CleanTitle:DE}`), `sofarr-indexer` (Newznab-RSS)
- Import der Bibliothek aus Radarr (Filme, Monitoring, Profile, Dateizuordnung)
- RSS-Sync im Scheduler, **read-only**: loggt „würde grabben“
- **Gate:** importierte Bibliothek deckungsgleich mit Radarr (fehlende Filme, Cutoff Unmet);
  zwei Wochen Shadow-Betrieb ohne unerklärte Abweichung gegenüber Radarrs History

## Phase 4 – Grab + Import (Radarr ablösen)

- nzbget hinter `DownloadClient`-Trait, Download-Überwachung
- Import: Zuordnung (Orakel: `manualimport`), Rename (Orakel: `rename`), Hardlink/Move
  (kein Jellyfin-Refresh – Jellyfin scannt selbst, Spec D12)
- Import Lists: TMDb-Personen-Listen (Orakel: `/api/v3/importlist`; hinzugefügte Filme gegen
  Radarrs Bibliothek abgleichen), Spec D11
- `/data` ohne `:ro` mounten, Radarr parallel pausieren statt löschen
- **Gate:** Import Lists laufen in sofarr; vier Wochen Alleinbetrieb ohne manuellen Eingriff
  → Radarr abschalten

## Phase 5+ – Serien

- Serien-Metadaten über TVDB (Spec D10), Episoden-Parsing, Staffeln, Specials
- Gleicher Ablauf: Fixtures (Sonarr-`parse`) → Ratchet → Decision-Orakel → Shadow → Ablösung
