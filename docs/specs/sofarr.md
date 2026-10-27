# Spec: amigo-sofarr

> Lebendes Dokument. Entscheidungen werden im Entscheidungslog (§9) nachgetragen, nicht überschrieben.

## 1. Ziel

amigo-sofarr ersetzt Sonarr und Radarr in **einem** Dienst: Bibliothek von Filmen und Serien
verwalten, neue Releases über Usenet-Indexer finden, nach Qualitätsregeln entscheiden, an einen
Download-Client übergeben, fertige Downloads importieren und umbenennen.

Erfolgskriterium: sofarr ersetzt im eigenen Home-Setup (Docker auf Mini-PC, nzbget, Jellyfin)
erst Radarr, dann Sonarr – ohne Verschlechterung im Alltag.

## 2. Scope

**Parity-Ziel ist das eigene Setup, nicht Sonarr/Radarr als Ganzes.** Was in der eigenen
Konfiguration nicht vorkommt (siehe `docs/parity.md`, erzeugt aus `scripts/export-arr-config.sh`),
ist bis auf Weiteres out of scope.

| Stufe | Inhalt |
|---|---|
| v1 | Filme: Parser, Decision Engine, Newznab-RSS, TMDB, nzbget, Import/Rename, Jellyfin-Refresh |
| v2 | Serien: Episoden-Parsing, Staffeln, Specials, Serien-Metadaten |
| später | Web-UI, amigo-downloader als Download-Client, Integrationen mit v3-API-Kompatibilität |

**Nicht-Ziele:** Musik, Bücher, Torrent (Usenet only, bis ein echter Bedarf entsteht),
Multi-User, vollständige Sonarr/Radarr-API-Kompatibilität.

## 3. Architektur

Ein Binary (`amigo-sofarr`), ein Container, SQLite. Cargo-Workspace:

| Crate | Aufgabe | IO? |
|---|---|---|
| `sofarr-parser` | Release-Name → `ParsedRelease` | nein |
| `sofarr-core` | Domänenmodell, Qualities, Profile, Custom Formats, Decision Engine | nein |
| `sofarr-indexer` | Newznab: Caps, RSS, Suche | ja |
| `sofarr-metadata` | TMDB (Serien-Provider: siehe §10) | ja |
| `sofarr-download` | `DownloadClient`-Trait, nzbget-Implementierung | ja |
| `sofarr-db` | SQLite via sqlx, Migrations | ja |
| `sofarr-server` | axum-API, Scheduler, Config, Binary `amigo-sofarr` | ja |
| `sofarr-parity` | CLI: Orakel-Fixtures sammeln, Verhalten diffen | ja |

Abhängigkeitsrichtung: `parser` ← `core` ← IO-Crates ← `server`.
Die Kernlogik (Parsing, Entscheidungen) ist dadurch vollständig ohne Netzwerk testbar.

## 4. Domänenmodell (Entwurf)

Begriffe orientieren sich an der Servarr-Nutzerdoku, damit Configs übertragbar bleiben.
Felder werden in den jeweiligen Phasen präzisiert.

- **Movie** – `tmdb_id`, `imdb_id?`, Titel, Originaltitel, Jahr, `monitored`, Quality Profile, Pfad, Status.
- **Release** – Rohdaten vom Indexer: GUID, Titel, Größe, Indexer, Veröffentlichungszeit, Download-URL, Kategorien.
- **ParsedRelease** – Titel-Kandidaten, Jahr, Quality (Source, Resolution, Modifier), Revision
  (Version, Proper/Repack/Real), Edition, Release Group, Sprachen.
- **Quality / QualityDefinition** – benannte Qualitätsstufe mit Größengrenzen pro Minute.
- **QualityProfile** – geordnete erlaubte Qualities, Cutoff, Upgrades erlaubt?,
  Custom-Format-Scores, Mindest-Score, Cutoff-Score.
- **CustomFormat** – benannte Menge von Bedingungen (Typ, Wert, `negate`, `required`).
- **Decision** – Release + ParsedRelease + `approved` + Liste von Rejections
  (Grund, permanent vs. temporär) + Custom-Format-Score.

## 5. Parity-Strategie

Sonarr/Radarr dienen als **Black-Box-Orakel** über ihre HTTP-API. Es wird nie ihr Code
gelesen (siehe §6).

| Was | Orakel-Endpoint | Phase |
|---|---|---|
| Parsing | `GET /api/v3/parse?title=…` | 1 |
| Grab-Entscheidungen | `GET /api/v3/release?movieId=…` (`approved`, `rejections`, Score) | 2 |
| Bibliotheksstand | `GET /api/v3/movie`, `/api/v3/series` | 3 |
| Tatsächliche Grabs | `GET /api/v3/history` | 3 |
| Import-Zuordnung | `GET /api/v3/manualimport?folder=…` | 4 |
| Benennung | `GET /api/v3/rename?movieId=…`, `/api/v3/config/naming` | 4 |

Werkzeuge:

1. **Fixtures:** `sofarr-parity collect` sammelt echte Release-Namen und speichert die
   Orakel-Antwort roh als JSONL. Öffentliche Fixtures (aus RSS) liegen in `fixtures/public/`,
   private (History, Bibliothek) in `fixtures/private/` (gitignored).
2. **Ratchet:** Pass-Raten pro Feld werden in `fixtures/parser-baseline.toml` festgehalten.
   Tests schlagen fehl, wenn eine Rate sinkt. Die Baseline darf nur steigen.
3. **Shadow-Mode:** sofarr läuft neben Radarr mit read-only `/data` und loggt, was es
   grabben würde. Abgleich gegen Radarrs History.
4. **Bewusste Abweichungen** werden in `docs/parity.md` dokumentiert und aus dem Diff ausgenommen.

## 6. Clean Room

sofarr ist MIT, die Servarr-Projekte sind GPL-3.0. Verbindliche Regeln: `CLAUDE.md`.
Kurzfassung: kein Servarr-Code, keine Servarr-Tests, keine aus dem Gedächtnis rekonstruierten
Regexes. Erlaubt: API-Beobachtung, Nutzer-Doku, allgemeines Wissen über Release-Namen.

## 7. Betrieb

- Container: statisches musl-Binary in `scratch`, Port **7676**, UID/GID 1000.
- Volumes: `/config` (Config, SQLite), `/data` (Downloads + Bibliothek unter **einem** Mount
  für Hardlinks und atomare Moves – gleiches Layout wie Radarr).
- Config: TOML unter `/config/sofarr.toml` (`SOFARR_CONFIG`), unbekannte Keys sind ein Fehler.
- Healthcheck: `amigo-sofarr healthcheck` (kein curl im Image nötig), Endpoint `GET /api/health`.
- Logging: `tracing`, `RUST_LOG` überschreibt Config, Format `pretty` oder `json`.

## 8. Integrationen

| Integration | Wann | Wie |
|---|---|---|
| nzbget | Phase 4 | JSON-RPC, hinter `DownloadClient`-Trait |
| Jellyfin | Phase 4 | Library-Refresh nach Import |
| amigo-downloader | nach Radarr-Ablösung | zweite `DownloadClient`-Implementierung (braucht Headless-Modus/Library-Crate) |
| Prowlarr, Jellyseerr, Bazarr | nur bei Bedarf | genau die v3-Endpoints, die diese Tools aufrufen |

## 9. Entscheidungslog

| # | Entscheidung | Begründung |
|---|---|---|
| D1 | Rust-Neuentwicklung statt .NET-10-Fork | Fork = Dauer-Rebase ohne Mehrwert; Neubau = eigenes Modell, Ressourcen, Lernwert |
| D2 | Ein Dienst für Filme + Serien | Gemeinsame Pipeline (Indexer → Decision → Download → Import) |
| D3 | MIT + Clean Room | Freiheit im amigo-labs-Ökosystem; GPL-Code wird nie berührt |
| D4 | SQLite (sqlx) | Single-User, ein Container, Mini-PC |
| D5 | Filme vor Serien | Radarr-Domäne ist deutlich einfacher; Serien bauen auf fertigem Kern auf |
| D6 | nzbget als erster Download-Client | Läuft bereits; nie zwei neue Komponenten gleichzeitig live schalten |
| D7 | Usenet only | Entspricht dem eigenen Setup |
| D8 | Eigene API, v3-Kompatibilität nur für konkrete Integrationen | Keine Altlasten aus Sonarr-API nachbauen |

## 10. Offene Fragen

- Serien-Metadaten: TVDB (Sonarr-Ordering, Parity) vs. TMDB (ein Provider für alles). Entscheidung vor Phase 5.
- Indexer direkt oder über Prowlarr? Abhängig vom eigenen Setup (Config-Export prüfen).
- Web-UI: Stack und Zeitpunkt (Bun + React + TS, eingebettet per `rust-embed`, ist der Favorit).
- amigo-downloader: Library-Crate oder Daemon mit API?
