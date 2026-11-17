---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §5, sofarr.md §7, sofarr.md §9 D31]
media: both
oracle: none
---

# Re-encode job

## Context

Spec `reencode-rule` lists the library files that are too large for their resolution,
largest first; spec `reencode-file` re-encodes one of them on request and records every
attempt. D31 decides that a background job works through the whole library. The owner chose
that it runs **around the clock**, one file after the other at the lowest CPU priority, the
largest files first, and that it is off until switched on. Radarr and Sonarr have no such
feature, so there is no oracle.

Terms used below:

- **Candidate**, **candidate list** – as in spec `reencode-rule` (largest file first).
- **Attempt**, **outcome**, **encode slot**, **interrupted attempt** – as in spec
  `reencode-file`; an interrupted attempt is one recorded without an end that no running
  encode holds. Attempts count here however they were started (job, CLI, API).
- **Library file** – the file record of a movie or of an episode file; an upgrade of a movie
  keeps its record with a new size, an upgrade of an episode gets a new record. Attempts
  belong to the record they were made on.
- **Settled file** – a library file with an attempt that ended `replaced` or `no_gain` at
  the size the file has now (for `replaced` the size after, for `no_gain` the size before),
  whatever came after it. A settled file stays settled when the configuration changes or a
  later attempt by hand fails, because encoding a re-encoded file again loses quality a
  second time and a `no_gain` file would mostly end the same way; the owner can still start
  it by hand.
- **Failure streak** – the attempts on a library file at its current size (size before
  equal to the size it has now) that ended `failed`, counted back from the newest to the
  latest `replaced` or `no_gain`. `refused` attempts and attempts that ended "abgebrochen"
  (interrupted or stopped with the server) neither count nor break the streak, because they
  say nothing about the file. A file that changed starts with an empty streak.
- **Resting file** – a library file that either has a failure streak of one or two whose
  last attempt ended less than 7 days ago, or has a streak of three or more; or whose latest
  attempt was `refused` less than 24 hours ago for another reason than a taken encode slot.
- **Due file** – a candidate that is neither settled nor resting.
- **Round** – the job takes the due files in the order of the candidate list and starts an
  attempt on the first one that is not refused; a refused file rests (24 hours) and the
  round goes on with the next. A round ends with one finished attempt, or with no due file
  left.

## Scope

### In scope

- The background job: rounds around the clock, largest due file first, one at a time
- Skipping settled and resting files, retrying failures later
- Waiting while the CLI holds the encode slot, and when nothing is due
- Repairing interrupted attempts when the server starts
- Stopping a running encode when the server stops
- The switch `[reencode] enabled` and the job on the system page

### Out of scope

- A page in the web UI, the history of attempts, excluding single titles — because that is spec `ui-reencode`
- Re-encoding audio — because that is spec `reencode-audio` (lossless tracks without Atmos/DTS:X to E-AC-3, owner 2026-11-17)
- A time window — because the owner chose around the clock at the lowest priority
- Several encodes at the same time — because one libx265 encode already uses every core

## Acceptance criteria

- [ ] AC-01 (state): While `[reencode] enabled = true` is set, sofarr runs in live mode and ffmpeg with libx265 can be run, the job runs rounds one after the other, each re-encoding a due file (spec `reencode-file`, runner `server`), the largest due file first.
- [ ] AC-02 (always): sofarr never starts an attempt in a round on a file that is not a candidate, that is settled, or that is resting.
- [ ] AC-03 (event): When an attempt of a round is refused (spec `reencode-file` AC-10, e.g. too little free space or Dolby Vision found), then that file rests for 24 hours and the job goes on in the same round with the next due file; a refusal because the encode slot was taken meanwhile ends the round without a rest, and the job waits as in AC-04.
- [ ] AC-04 (state): While no file is due, the job starts nothing and looks again after 60 minutes; while the encode slot is held (by the CLI or an API start), it starts no attempt, and looks again within one minute after the slot is free.
- [ ] AC-05 (event): When the server starts in live mode, then before any round it repairs every interrupted attempt (what spec `reencode-file` AC-12 does for one file: hidden originals back or to the recycle bin, work files removed, the record closed as `failed` "abgebrochen"), whether the job is switched on or not.
- [ ] AC-06 (event): When the server stops (signal) during an encode it runs (by the job or an API start), then the encoder is stopped within 10 seconds (Docker's default before it kills the container), its work file is removed, the original stays untouched, and the attempt is recorded as `failed` "abgebrochen: sofarr wurde beendet"; a swap already under way is finished first.
- [ ] AC-07 (option): Where `[reencode] enabled` is not set or `false` (the default), or sofarr runs in shadow mode, or ffmpeg with libx265 cannot be run, the job starts no attempt; requests by CLI and API (spec `reencode-file`) do not depend on `enabled`. Like every key, `enabled` is read when sofarr starts; whether ffmpeg with libx265 can be run is checked when sofarr starts and again before each round and each hourly look, so an installed ffmpeg is used without a restart.
- [ ] AC-08 (event): When the user opens the system page, then the job list shows the job "Neu encodieren" with its last run and, while it waits, its next look, and the page says "Neu encodieren: aktiv" with the file it encodes, or "Neu encodieren: aus – {reason}" with one of the reasons "in der Konfiguration ausgeschaltet", "im Shadow-Mode", "ffmpeg mit libx265 nicht gefunden".
- [ ] AC-09 (state): While a file rests after the first or second failure of its streak, it is not taken; 7 days after that failure it is due again and is taken when it is the first due file of a round; after the third failure it rests until it changes.
- [ ] AC-10 (always): A round decides which file is due from the database alone, in under 2 s for a library of 2,500 movies and 20,000 episode files with 10,000 recorded attempts (`cargo test -p sofarr-server --release reencode_job_round_large_library`, measured duration in the test output).

## Technical notes

- Config: `[reencode] enabled` (default `false`), next to the keys of `reencode-rule` and
  `reencode-file`; `config/sofarr.example.toml` shows it.
- The job is one more entry of the scheduler's job list (`status.rs`, German label "Neu
  encodieren"); its interval is "after each attempt", the hourly look is the fallback.
- A stopping server needs the process runner to be cancellable: the runner looks at a stop
  flag at every poll (200 ms) besides time and size.
- The system page answer gets `reencode: {active, message, running}` like `trailers`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| The job is switched on with 400 candidates | It runs through them for weeks, largest first, one at a time |
| A 60 GB file and the disk has 20 GB free | Refused for space; the next, smaller candidate is taken in the same round |
| A file ended `no_gain` (already efficient HEVC) | Settled: never taken again while unchanged |
| A movie is upgraded to a new remux after it was replaced | Changed: a candidate again, so due again |
| A file fails three times (e.g. a broken stream) | Rests until it changes; the owner can start it by hand |
| A Dolby Vision file whose stored media info missed it | Refused at its attempt, rests 24 hours, then is refused again once a day |
| The server is restarted three times during the same file's encode | The attempts end "abgebrochen"; they do not count as failures, the file stays due |
| An upgrade of a movie with exactly the same size | Taken for unchanged: stays settled (see Assumptions) |
| The owner starts `amigo-sofarr reencode` while the job is between rounds | The CLI takes the slot; the job waits until it is free |
| The container is stopped during a 6-hour encode | ffmpeg is stopped, the work file goes, the attempt reads "abgebrochen: sofarr wurde beendet" |
| The container is killed hard (no signal handling) | At the next start the interrupted attempt is repaired (AC-05) |
| An import adds a new oversized file | It becomes a candidate at the next round's look (at most 60 minutes) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 7 days between failures, three in a row | Transient causes (a full disk, a file being written) pass within days; a broken file stays broken | A file broken for longer rests until it changes; the owner sees it in the history |
| An upgrade is recognised by a changed size | Two releases of the same movie with the same size in bytes do not occur in practice | Such an upgrade would not be re-encoded automatically |
| Looking again every 60 minutes is often enough | A new import waits at most an hour; a look costs one query | None worth naming |
| Running at niceness 19 leaves the server usable | The kernel gives every other process priority | Disk I/O is not lowered; a busy disk can slow Jellyfin while it streams |

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

- 2026-11-17: Around the clock at the lowest CPU priority instead of a time window; largest files first; off by default (owner; D31).
- 2026-11-17: Audio is re-encoded in a later spec `reencode-audio`: lossless tracks without Atmos/DTS:X become E-AC-3 (owner).
- 2026-11-17: A refused file rests 24 hours and the round goes on, so a refusal never loops; failures caused by a stop or restart of sofarr do not count toward a file's rest.
- 2026-11-17: A file stays settled once an attempt at its current size ended `replaced` or `no_gain`, whatever came after; a changed file starts a new failure streak.
