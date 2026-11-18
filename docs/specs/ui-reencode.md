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
| AC-01 | `is in the navigation and shows the empty states (AC-01, AC-07)` (`ui/e2e/reencode-page.spec.ts`); `ui/src/lib/nav.test.ts` | built |        |
| AC-02 | `reencode::page::tests::the_page_shows_job_candidates_states_saved_space_and_history` (`crates/sofarr-server/src/reencode/page/tests.rs`); `reencode::tests::recent_attempts_are_limited_and_keep_files_that_are_gone` (`crates/sofarr-db/src/reencode.rs`); `shows job, saved space, candidates with their state, and the history (AC-02, AC-03, AC-06)` (`ui/e2e/reencode-page.spec.ts`); `name sizes and the saved space in German` (`ui/src/lib/reencode/reencode.test.ts`) | built |        |
| AC-03 | `reencode::page::tests::the_page_shows_job_candidates_states_saved_space_and_history` (`crates/sofarr-server/src/reencode/page/tests.rs`); `reencode::job::tests::the_state_names_why_a_file_is_not_due` (`crates/sofarr-server/src/reencode/job/tests.rs`); `shows job, saved space, …` (`ui/e2e/reencode-page.spec.ts`); `name every job state`, `list the reasons of left-out files that have any` (`ui/src/lib/reencode/reencode.test.ts`) | built |        |
| AC-04 | `starts an encode and keeps a file out of the job (AC-04, AC-05)`, `shows job, saved space, …` (disabled while another encode runs) (`ui/e2e/reencode-page.spec.ts`); `reencode::page::tests::exclusions_keep_files_out_of_the_job_and_can_be_undone` (an excluded file encoded on request) (`crates/sofarr-server/src/reencode/page/tests.rs`); `say why "Jetzt encodieren" is not available` (`ui/src/lib/reencode/reencode.test.ts`) | built |        |
| AC-05 | `reencode::page::tests::exclusions_keep_files_out_of_the_job_and_can_be_undone` (`crates/sofarr-server/src/reencode/page/tests.rs`); `reencode::job::tests::excluded_files_are_never_due` (`crates/sofarr-server/src/reencode/job/tests.rs`); `reencode::tests::exclusions_follow_their_file` (`crates/sofarr-db/src/reencode.rs`); `starts an encode and keeps a file out of the job (AC-04, AC-05)` (`ui/e2e/reencode-page.spec.ts`) | built |        |
| AC-06 | `reencode::page::tests::the_page_shows_job_candidates_states_saved_space_and_history` (`crates/sofarr-server/src/reencode/page/tests.rs`); `shows job, saved space, …` (`ui/e2e/reencode-page.spec.ts`); `name outcomes, runners and sizes of the history` (`ui/src/lib/reencode/reencode.test.ts`) | built |        |
| AC-07 | `is in the navigation and shows the empty states (AC-01, AC-07)` (`ui/e2e/reencode-page.spec.ts`) | built |        |
| AC-08 | `reencode::page::tests::shadow_mode_changes_nothing` (`crates/sofarr-server/src/reencode/page/tests.rs`); `is read-only in shadow mode (AC-08)` (`ui/e2e/reencode-page.spec.ts`) | built |        |
| AC-09 | `follows the state without a reload and says when sofarr is gone (AC-09)` (`ui/e2e/reencode-page.spec.ts`) | built |        |
| AC-10 | `reencode::page::tests::reencode_page_large_library` (`crates/sofarr-server/src/reencode/page/tests.rs`) | built |        |

## Decisions

- 2026-11-17: Excluding works per library file, not per series; "Jetzt encodieren" ignores the exclusion, because a request on the page is the owner's decision (like spec `reencode-file` AC-13).
- 2026-11-18: The job state of a file comes from one pure function (`reencode::job::state_of`), which the job's due check now uses as well; the page adds "läuft" (the slot's path) and "ausgeschlossen". Exclusions live in `reencode_exclusions` (migration 0014), keyed on the movie's file record (`movie_files.movie_id`) and the episode file, both `ON DELETE CASCADE`.
- 2026-11-18: The page reloads through the shared `createResource` (every 15 s, the last state stays on a failure); it remembers when it last loaded and shows "Stand von {Zeit} – sofarr ist nicht erreichbar". The navigation entry uses Octicon `zap`.
- 2026-11-18: New API: `GET /api/reencode` (`ReencodePage`), `POST`/`DELETE /api/reencode/exclusions` (204; 404 unknown; 400 wrong body; 409 "Ausschluss nicht geändert: im Shadow-Mode"), `Db::recent_reencodes`, `reencode_savings`, `exclude_from_reencode`, `include_in_reencode`, `reencode_exclusions`.
- 2026-11-18: The DB functions were red against stubs first; the page API, the page and its tests were written together.
- 2026-11-18: The navigation lists of the shell tests (`ui/e2e/shell.spec.ts`, `ui/e2e/layout.spec.ts`) name the new entry; they check the order of every built page.
