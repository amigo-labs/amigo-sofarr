---
status: todo
created: 2026-11-19
size: S
design: [sofarr.md §5, sofarr.md §6]
media: movies
oracle: GET /api/v3/release?movieId=… (Radarr, fixtures/public/radarr-release.jsonl), GET /api/v3/history/movie?movieId=…
---

# Release oracle: releases already grabbed and imported

## Context

The release oracle compares sofarr's decision engine with Radarr's grab decisions
(`fixtures/public/radarr-release.jsonl`, ratchet `fixtures/decision-baseline.toml`). Two
rejections of Radarr are listed as "Noch nicht abgedeckt" in `docs/parity.md` and in the backlog
of `docs/specs/README.md`:

1. **"Has same release name as a grabbed and imported release"** (1 public fixture,
   `Papillon.2017.German.AC3.DL.1080p.BluRay.x265-FuN`). sofarr's RSS sync and search already
   skip a release whose name equals an imported grab of the same item (ignoring case, spec
   `fail-handling` and `Exclusions` in the server). The decision engine itself does not know the
   history, and a release fixture does not record it, so the oracle cannot check the rule: the
   fixture counts as a mismatch on `rejections`.
2. **"Raw Bluray/DVD release"** (6 public fixtures). Radarr decides this from indexer data that a
   fixture does not hold; the same title patterns appear with and without the rejection. It stays
   unmodelled and becomes a deliberate deviation (see Decisions).

Terms:

- **Imported release names** of a film — the release names (`sourceTitle`) of the film's
  history entries of type `grabbed` whose download id also appears in an entry of type
  `downloadFolderImported` of that film (Radarr API documentation, history resource).
- **Release fixture** — one line of `radarr-release.jsonl`: release title, the film as the
  decision sees it (`movie`), Radarr's answer (`response`).

## Scope

### In scope

- The decision engine rejects a release whose title equals an imported release name of the film.
- `sofarr-parity collect --source releases` records the imported release names with each film;
  the public fixture keeps only the release's own title when it is one of them.
- The release diff passes the names to the decision and compares the rejection; fixtures
  collected before have no names and leave the rejection out of the comparison.
- "Raw Bluray/DVD release" as a deliberate deviation in `docs/parity.md`.

### Out of scope

- Changing how RSS sync and search skip imported names (`Exclusions`) — because they already
  behave like the oracle; moving them into the decision engine would change no observable result.
- Series (Sonarr release decisions) — because there is no Sonarr release oracle.
- Recollecting the existing fixtures — because that needs the owner's live Radarr; new runs of
  `collect --source releases` add the names.

## Acceptance criteria

- [ ] AC-01 (event): When the decision for a film gets its imported release names and the
  release title equals one of them ignoring case, then sofarr rejects the release with the
  rejection `already_imported`, in addition to every other rule's rejection — for every release
  that passes the year rule (spec `decision-engine` AC-04); a release that fails it keeps only
  `unparsable` or `unknown_movie`.
- [ ] AC-02 (state): While a film has no imported release names, or none equals the release
  title, sofarr gives no `already_imported` rejection.
- [ ] AC-03 (event): When `sofarr-parity collect --source releases` records the releases of a
  film, then the private fixture's film carries all imported release names of the film
  (`importedReleases`, from Radarr's movie history), and the public fixture's film carries
  `importedReleases` with only the release's own title when it is one of them, else an empty
  list; `rebuild-public` derives the same public list from the private file.
- [ ] AC-04 (event): When the release diff evaluates a fixture whose film has
  `importedReleases`, then the decision gets these names and `already_imported` is compared like
  every other rejection.
- [ ] AC-05 (state): While a fixture's film has no `importedReleases` (collected before this
  spec), the release diff leaves `already_imported` out of the expected rejections, and the
  report states how many fixtures were compared without history.
- [ ] AC-06 (error): If Radarr's movie history cannot be read during collection, then the
  collection of that film fails with a German error message naming the film id, and no partial
  lines of that film are written.
- [ ] AC-07 (always): The release-oracle ratchet over the public fixtures does not fall below its
  baseline, which is rewritten to the new rates.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| Imported name differs only in case (`papillon.2017…-fun`) | `already_imported` |
| Grab without an import (download failed or still running) | Not an imported release name |
| Import entry without a matching grab (manual import) | Not an imported release name |
| Same name imported for another film | No rejection — names are per film |
| Release title with a trailing `.nzb` or spaces vs. the history name | Titles are compared after trimming spaces only; no other normalisation |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Radarr's movie history (`GET /api/v3/history/movie?movieId=…`) lists `eventType`, `sourceTitle` and `downloadId` per entry, with `grabbed` and `downloadFolderImported` as event types | Radarr API documentation; `compare-history` already reads `eventType`/`sourceTitle` from `/api/v3/history` | Other field names would show as zero imported names in new fixtures; the collector test pins the shape |

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
| AC-07 |      | open   |        |

## Decisions

- 2026-11-19 (spec): "Raw Bluray/DVD release" is a deliberate deviation, because Radarr decides
  it from indexer metadata a release title does not carry (six public fixtures share their title
  patterns with releases Radarr accepts); guessing from the title would reject good releases.
  sofarr rejects full discs through the quality `BR-DISK` where the title names one. Recorded in
  `docs/parity.md`, "Bewusste Abweichungen".
- 2026-11-19 (spec): The public fixture keeps only the release's own title from the imported
  names, because the full history of a film is private (like `radarr-parse-history.jsonl`),
  while the release's own title is public already.
- 2026-11-19 (check): AC-01 revised: a release that fails the year rule gets only `unparsable` or `unknown_movie`, as decision-engine AC-04 says and as Radarr answers "Unable to parse" without further checks (`parity.md`); the acceptance check found the two specs contradicting each other.
