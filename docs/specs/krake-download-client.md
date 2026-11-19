---
status: todo
created: 2026-11-19
size: M
design: [sofarr.md §3, sofarr.md §8, sofarr.md §9 D6, D32]
media: both
oracle: none
---

# Krake as download client

## Context

`sofarr.md` §8 planned amigo-downloader as a second download client "after Radarr is replaced",
and §10 left open whether it becomes a library crate or a daemon with an API. amigo-downloader is
now **Krake** (`amigo-labs/amigo-krake`, MIT): a download manager that runs as a server with a
Usenet engine and an NZBGet-compatible JSON-RPC API (`POST /jsonrpc`, HTTP Basic Auth), built for
Sonarr/Radarr. sofarr's download client already speaks that protocol for nzbget (D6).

Decision D32 (owner, 2026-11-19): sofarr uses Krake through its NZBGet-compatible API, with the
existing `[nzbget]` configuration — no second `DownloadClient` implementation. Krake's API had
gaps that made it unusable for sofarr (the NZB of `append` was dropped, categories ignored, the
history had no `Name`, all NZBs shared one folder); they are fixed in Krake itself
(amigo-labs/amigo-krake#134). This spec proves that sofarr's whole download path works against Krake's answers,
which differ from nzbget's in details (version text, fields always present but zero, failures
reported as `FAILURE/NONE`, ids derived from Krake's own download ids).

Terms:

- **Krake fake** — an HTTP server in sofarr's tests that answers the JSON-RPC methods sofarr calls
  with the response shapes of Krake's NZBGet-compatible API (from Krake's source,
  `crates/server/src/nzbget_api.rs`, MIT — not a Servarr project).
- **Grab** — a release sofarr handed to the download client (spec `fail-handling`).

## Scope

### In scope

- Grab, queue, completion, import, failure and removal of a download against Krake's answers, for
  movies and episodes, in live mode.
- Telling sofarr's downloads apart from other downloads in Krake by category.
- Documentation: Krake as the download client in the configuration example.

### Out of scope

- A native client for Krake's REST API (`/api/v1`) — because D32 chooses the NZBGet-compatible API.
- Telling release failures from local failures with Krake — because Krake reports every failure
  as `FAILURE/NONE`; sofarr treats such a grab as failed for local reasons (no blocklist), see
  Assumptions.
- Running nzbget and Krake side by side — because `[nzbget]` names one client.

## Acceptance criteria

- [ ] AC-01 (event): When sofarr grabs a release in live mode and `[nzbget] url` points at Krake,
  then sofarr sends the NZB with the category of the movie or series to Krake and records the
  download id Krake returns.
- [ ] AC-02 (state): While Krake lists the download in its queue (queued, downloading or paused),
  sofarr shows the grab as downloading with Krake's size and remaining size.
- [ ] AC-03 (event): When Krake reports the download as successful in its history, then sofarr
  imports the files from the folder Krake names into the library, and afterwards removes the
  entry from Krake without asking Krake to delete files.
- [ ] AC-04 (event): When Krake reports the download as failed (`FAILURE/NONE`), then sofarr marks
  the grab as failed for a local reason, as for the same state from nzbget.
- [ ] AC-05 (state): While Krake also holds downloads of other categories, sofarr neither imports
  nor removes them.
- [ ] AC-06 (error): If Krake rejects the credentials (HTTP 401) or answers an RPC error, then the
  grab fails with the German error message of the download client and no grab is recorded as
  sent.

## Technical notes

- Configuration: `[nzbget] url = "http://krake:1516"`, `username`/`password` = Krake's
  `nzbget_api` credentials. Krake mounts `/jsonrpc` only when its NZBGet API is enabled.
- Krake's answers (after the fix): `append` → positive integer; `listgroups` entries with
  `NZBID`, `NZBName`, `Category`, `Status` (`QUEUED`/`DOWNLOADING`/`PAUSED`), `FileSizeMB`,
  `RemainingSizeMB`, `DestDir`; `history` entries with `NZBID`, `Name`, `NZBName`, `Category`,
  `Status` (`SUCCESS/ALL` or `FAILURE/NONE`), `FileSizeMB`, `DestDir`, `FinalDir`; `version` →
  `"krake-<version>"`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| History entry with `FinalDir` equal to `DestDir` | Import from that folder |
| Queue entry with size 0 (Krake does not know the size yet) | Downloading, sizes 0 |
| Krake restarts and reports the same download under the same id | Same grab, no duplicate |
| Download vanishes from Krake | Vanished-download handling of `fail-handling` applies unchanged |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Every Krake failure counts as a local failure (no blocklist, no automatic re-search of another release) | Krake reports no failure kind; nzbget's `FAILURE/NONE` is treated the same way | A broken release may be grabbed again by the next search; the owner can blocklist it by hand |
| Krake's ids stay stable for a download across restarts | Krake derives the NZBID from its stored download id | Krake hashes the id with Rust's default hasher, stable within one build only: after a Krake update, grabs still downloading would show as vanished |
| A Usenet download Krake could not complete still arrives as `SUCCESS/ALL` | Krake detects no missing articles or failed post-processing yet (amigo-krake#134, known limits) | sofarr finds no video and handles it as a failed import (spec `download-import`), not as a failed release |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 |      | open   |        |
| AC-02 |      | open   |        |
| AC-03 |      | open   |        |
| AC-04 |      | open   |        |
| AC-05 |      | open   |        |
| AC-06 |      | open   |        |

## Decisions
