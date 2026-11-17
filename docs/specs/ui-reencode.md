---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §9 D24, sofarr.md §9 D30, sofarr.md §9 D31]
media: both
oracle: none
---

# Re-encode page

## Context

Specs `reencode-rule`, `reencode-file` and `reencode-job` find the library files that are too
large, re-encode one on request and work through them in the background. The owner can
only see this today through the CLI, the API and one line on the system page. This spec adds
the page "Neu encodieren" to the web UI (D30 look): what is too large, what the job will do
next, what it did, how much space it saved — and lets the owner start a file at once or keep
a title out of the job.

Terms used below:

- **Candidate**, **candidate list**, **summary** – as in spec `reencode-rule` (largest file
  first; the summary counts candidates, their size, fine files and excluded files per
  reason).
- **Attempt**, **outcome**, **running encode** – as in spec `reencode-file`.
- **Settled**, **resting**, **due** – as in spec `reencode-job`, plus: an **excluded** file is
  one the owner kept out of the job on this page. Exclusion belongs to the job, not to the
  rule of spec `reencode-rule`: an excluded file stays a candidate (its forward note on a
  user exclusion reason is not taken up), because the owner wants to see it in the list.
- **Job state of a candidate** – exactly one of, in this order: "läuft" (it is the running
  encode), "ausgeschlossen", "erledigt" (settled after `replaced`), "kein Gewinn" (settled
  after `no_gain`), "ruht bis {Datum, Uhrzeit}" (resting after a failure or a refusal, with
  the time it is due again – the later one where both apply), "ruht, bis sich die Datei ändert" (third failure in a row),
  "fällig".
- **Saved space** – the sum of size before minus size after over all attempts that ended
  `replaced`, with their number.
- **History** – the latest 50 attempts, newest first.

## Scope

### In scope

- The page "Neu encodieren" with its entry in the navigation (group "Verwaltung")
- The job's state, the running encode, the saved space
- The candidate list with the job state of each file, and the summary
- Starting one file at once; excluding a file from the job and taking it back
- The history of attempts
- One API answer for the page, and the exclusions in the database

### Out of scope

- Changing thresholds or encoder settings on the page — because every setting lives in `sofarr.toml` (sofarr.md §7) and the system page shows them
- Progress in percent of a running encode — because sofarr does not read ffmpeg's progress (spec `reencode-file`)
- Paging the candidate list — because the list holds at most the oversized files of one library (a few hundred), like the other lists without paging
- Excluding by series or by movie folder — because the owner asked for single titles; a series' files are listed one by one
- Audio — because that is spec `reencode-audio`

## Acceptance criteria

- [ ] AC-01 (event): When the user opens "Neu encodieren" in the navigation (group "Verwaltung", path `/neu-encodieren`), then sofarr shows the page with the heading "Neu encodieren".
- [ ] AC-02 (state): While the page is open, it shows the job's state as on the system page (spec `reencode-job` AC-08: "aktiv", "aktiv – encodiert {Titel}", "aus – {Grund}"), the running encode with its title, start time and whether the server or the CLI runs it (whoever holds the encode slot; the job state then reads "aktiv – encodiert {Titel}" too, as on the system page), and the saved space as "{n} Dateien, {x,y} GB gespart".
- [ ] AC-03 (state): While the page is open, it lists the candidates largest first, each with its title (a link to its movie or series page), file size in GB, size per minute against the threshold in MB, resolution class, video codec and job state; above the list it shows the number of candidates and their size, and for each exclusion reason of spec `reencode-rule` that has files the number of such files.
- [ ] AC-04 (event): When the user presses "Jetzt encodieren" at a candidate, then sofarr starts its re-encode at once (spec `reencode-file` AC-02, also for a settled, resting or excluded file) and the page says "Encode gestartet: {Titel}"; a refusal shows its German reason as a notice; while an encode runs or sofarr is in shadow mode the button is disabled and names why.
- [ ] AC-05 (event): When the user presses "Ausschließen" at a candidate, then the job never takes that library file again (it is not due), the file shows "ausgeschlossen" and the button becomes "Wieder aufnehmen", which undoes it. The exclusion belongs to the library file: it stays when a movie's file is upgraded (the movie keeps its file record, and the owner excluded the title, e.g. a remux kept on purpose), and goes when the file is removed from the library or an episode file is replaced by a new one (a new file is a new decision). For a file with several episodes any of its episode ids names it. If the movie or episode is unknown or has no file, the API answers 404 and the page shows the German message as a notice.
- [ ] AC-06 (state): While the page is open, it lists the history: per attempt the title, start time, who ran it ("Server" for the job and API starts, "CLI"), the outcome in German ("ersetzt", "kein Gewinn", "fehlgeschlagen", "abgelehnt", "läuft"), the reason, and the sizes before and after in GB with the gain in percent for "ersetzt".
- [ ] AC-07 (state): While no file is too large, the page shows "Keine Datei ist zu groß." instead of the list; while there is no attempt yet, the history says "Noch nichts encodiert."
- [ ] AC-08 (state): While sofarr runs in shadow mode, the page shows everything read-only: "Jetzt encodieren", "Ausschließen" and "Wieder aufnehmen" are disabled with the reason "im Shadow-Mode", and the API refuses to start an encode or to change an exclusion with `409` and that reason.
- [ ] AC-09 (event): When an encode started on the page or by the job ends while the page is open, then the page shows the new state within 30 seconds without a reload; if sofarr cannot be reached meanwhile, the page keeps the last state and says "Stand von {Uhrzeit} – sofarr ist nicht erreichbar".
- [ ] AC-10 (always): The page's API answer for 2,500 movies and 20,000 episode files with 10,000 recorded attempts comes in under 2 s (`cargo test -p sofarr-server --release reencode_page_large_library`, measured duration in the test output).

## Technical notes

- API: `GET /api/reencode` answers job state, running encode, saved space, summary,
  candidates with job state (and the time a resting file is due again), history (50) and
  whether the page may act (`shadow`). `POST /api/reencode/exclusions` / `DELETE
  /api/reencode/exclusions` with a movie id or an episode id.
- DB: table of excluded library files (movie id or episode file id, `ON DELETE CASCADE`).
- The page polls `GET /api/reencode` every 15 seconds while it is open (AC-09).
- Types through ts-rs as for the other pages; texts in `ui/src/lib/reencode/`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The owner excludes a 4K remux and later presses "Jetzt encodieren" at it | It is encoded (the request is the owner's decision); it stays excluded from the job |
| A double episode | One row "Insel – S01E02-E03"; excluding it excludes the one file |
| A movie is upgraded after it was excluded | Still excluded (the library file of a movie keeps its record) |
| An episode is upgraded after it was excluded | The new file is not excluded (an episode upgrade is a new record) |
| The job switched off in the config | State "aus – in der Konfiguration ausgeschaltet"; "Jetzt encodieren" still works |
| The CLI encodes while the page is open | "läuft" at that file, the running encode says "CLI"; "Jetzt encodieren" disabled |
| A refusal ("zu wenig Speicherplatz …") | Notice with the reason; the file shows "ruht bis …" (24 hours) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 50 attempts are enough history | The owner looks at recent work; older attempts stay in the database | Older outcomes are not visible on the page |
| Polling every 15 s | An encode takes hours; the state changes rarely | A finished encode shows up to 15 s late |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

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
| AC-08 |      | open   |        |
| AC-09 |      | open   |        |
| AC-10 |      | open   |        |

## Decisions

- 2026-11-17: Excluding works per library file, not per series; "Jetzt encodieren" ignores the exclusion, because a request on the page is the owner's decision (like spec `reencode-file` AC-13).
