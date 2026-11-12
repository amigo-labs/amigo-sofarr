# Specs

Zwei Arten von Specs liegen hier (`AGENTS.md`, „Spec-driven workflow“):

- **Design-Spec** – [`sofarr.md`](sofarr.md): Ziel, Scope, Architektur, Parity-Strategie,
  Entscheidungslog (§9). Dazu die Parity-Matrix [`../parity.md`](../parity.md).
- **Feature-Specs** – je ein Feature mit Akzeptanzkriterien und Evidence-Tabelle: geschrieben mit
  `/spec`, gebaut mit `/spec-build`, abgenommen mit `/spec-check` (nur dieser Schritt setzt
  `status: implemented`).

## Reihenfolge

Phase 6 „Betrieb & Komfort“ (Entscheidungen D13–D20). Die Specs sind noch nicht geschrieben;
jede entsteht mit `/spec <name>` unmittelbar bevor sie gebaut wird und rückt dann mit Link in
diese Tabelle. Die Reihenfolge ist verbindlich, weil spätere Specs auf früheren aufbauen.

| # | Spec | Größe | Hängt ab von | Inhalt | Nächster Schritt |
| - | ---- | ----- | ------------ | ------ | ---------------- |
| 1 | fail-handling | M | – | Blocklist, Neusuche nach `autoRedownloadFailed`, Serien-Suche (`t=tvsearch`), manuelle Suche (D13) | `/spec fail-handling` |
| 2 | backups | S | – | `VACUUM INTO` per Scheduler, Snapshot vor Migrationen, `backup`/`restore` (D14) | `/spec backups` |
| 3 | auth | M | – | Abschaltbarer Login (argon2, Session) und API-Key, generische Fehlermeldungen (D15) | `/spec auth` |
| 4 | mediainfo | L | – | Eigener MKV/MP4-Reader: Codecs, HDR, Audio- und Untertitelspuren; echte `{MediaInfo…}`-Tokens | `/spec mediainfo` |
| 5 | web-ui | L | auth | Svelte 5 + SvelteKit statisch, eingebettet; Übersicht, Filme, Serien, Fehlend, Blocklist, Backups (D16) | `/spec web-ui` |
| 6 | discover | L | web-ui | Entdecken/Trending, Personen-Seiten mit Filmografie, Hinzufügen, IMDb-Rating aus den IMDb-Datasets (D19, D20) | `/spec discover` |
| 7 | subtitles | L | mediainfo | OpenSubtitles.com, Ablage nach Jellyfin-Konvention (D17) | `/spec subtitles` |
| 8 | trailers | M | – | TMDB-Trailer per yt-dlp nach `trailers/`, Runtime-Image Alpine (D18) | `/spec trailers` |

Größere Specs (L) werden beim Schreiben geteilt, wenn sie mehr als ein paar Sessions brauchen
(z. B. `web-ui-shell` / `web-ui-library`, `discover-browse` / `discover-imdb`).

## Done

Noch keine Feature-Spec abgenommen.

## Bestehendes Verhalten ohne Feature-Spec

Die Phasen 1–5 (vor diesem Workflow) haben gebaut, was `sofarr.md` §2 für v1 und v2 nennt:
Parser für Filme und Episoden, Decision Engine, Release-Orakel, Bibliothek mit SQLite, TMDB und
TVDB, Newznab-RSS, Shadow- und Live-Modus, nzbget, Import und Benennung, Import Lists,
Festplatten-Scan, Verfügbarkeit, Tags sowie die Orakel-Abgleiche `compare-*`. Der Stand pro
Feature steht in [`../parity.md`](../parity.md). Belegt ist dieses Verhalten durch Tests und die
Orakel-Ratchets, aber nicht als Akzeptanzkriterien erfasst; `/spec-extract <bereich>` holt das
nach (siehe Backlog).

## Backlog

Lücken, die noch nicht als Spec geschrieben sind. Keine Zusagen.

| Lücke | Hinweis |
| ----- | ------- |
| Feature-Specs für das Verhalten aus Phase 1–5 | `/spec-extract` je Bereich: parser, decision-engine, library-import, rss-sync, download-import, naming, import-lists, disk-scan, series |
| Parser: Anime-Titel mit Jahr (`Title (2026) - 06`) | Episoden-Orakel: 9 Fälle ohne Treffer (Serientitel, absolute Nummer) |
| Parser: Titel ohne Staffel (`Title.E06.…`) | Episoden-Orakel: Sonarr nimmt Staffel 1 an |
| Parser: Anime-Batches `(23-26)` | Episoden-Orakel: nur die letzte Nummer erkannt |
| Release-Orakel: „Raw Bluray/DVD release“, „same release name as imported“ | `parity.md`, „Noch nicht abgedeckt“ |
| amigo-downloader als zweiter Download-Client | `sofarr.md` §8, offene Frage §10 |
| v3-API-Shim für Jellyseerr/Prowlarr/Bazarr | `sofarr.md` §8, nur bei Bedarf (D8) |

## Offene Punkte außerhalb des Codes

Nur eine Person kann sie erledigen.

| Punkt | Wie |
| ----- | --- |
| Container in der eigenen Compose betreiben | `compose.example.yml` übernehmen, Status `healthy` |
| Review des Clean Rooms für Phase 2 | kein Servarr-Code, keine übernommenen Regexes |
| Filme im Shadow-Mode | `sofarr.toml` mit `[radarr]`, `[tmdb]`, `[[indexer]]`; `import-radarr`, `refresh-metadata`; Container im Shadow-Mode; `compare-library` grün; nach zwei Wochen `compare-history` ohne unerklärte Abweichung |
| Radarr ablösen | `compare-rename`, `compare-import-lists`, `compare-manual-import --folder …` ohne unerklärte Abweichung; `/data` ohne `:ro`, `[nzbget]`, `mode = "live"`, Radarr pausieren (nicht löschen); vier Wochen Alleinbetrieb, dann Radarr abschalten |
| Serien im Shadow-Mode | `import-sonarr`, `refresh-series`, `compare-series-library` grün; Shadow-Betrieb für Serien |
| Sonarr ablösen | nach stabiler Shadow-Phase Sonarr pausieren, Serien im Live-Modus |
