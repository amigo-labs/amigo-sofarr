---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §5, sofarr.md §7, sofarr.md §9 D18, sofarr.md §9 D22, sofarr.md §9 D31]
media: both
oracle: none
---

# Re-encode one file

## Context

Spec `reencode-rule` lists the library files that are too large for their resolution. D31
decides that sofarr re-encodes them to HEVC with the image's ffmpeg (libx265) and moves the
original to the recycle bin. This spec does that for **one file on request** – through the
CLI and the API – so the owner can try the encode and the swap on single files before
spec `reencode-job` lets a background job work through the candidate list around the clock.
Radarr and Sonarr have no such feature, so there is no oracle.

Terms used below:

- **Library file**, **candidate**, **too large**, **threshold**, **MB** – as in spec
  `reencode-rule`.
- **Original** – the library file being re-encoded, at the path the library records.
- **Original's media info** – the media info sofarr reads from the original when an attempt
  starts (not the stored one, which may be stale). The checks below compare against it.
- **Encode slot** – at most one encode runs at a time on the whole system: in the server or
  in any CLI process. It is held from the start of an attempt to its end.
- **Result** – the new file the encoder writes. It is always Matroska (`.mkv`), because
  Matroska takes every audio and subtitle track along unchanged (MP4 cannot hold PGS or
  ASS subtitles).
- **Work file** – the result while it is written and checked: a hidden file in the
  original's folder (name starting with `.`, ending in `.reencode-partial`), so no video
  is copied across file systems and no disk scan or import ever takes it for a video.
- **Final name** – the original's path with the extension `.mkv` (unchanged for a
  Matroska original).
- **Gain** – how much smaller the result is than the original, in percent of the
  original's size.
- **Size limit** – the largest size a result may have and still be good (a result exactly
  at it is good, one byte more is not):
  `(100 − min_saving_percent) %` of the original's size.
- **Good result** – a result that the media info reader of spec `mediainfo` reads with
  status `complete`, whose video is HEVC with the width and height of the original's media
  info, that has as many audio and as many subtitle tracks as it, whose duration (the header
  duration the media info carries, spec `reencode-rule` AC-01) differs from the original's
  by at most 1 second where the original has one – a result without one then is not good
  (container durations differ by up to a frame after an encode; more means the result is
  cut off),
  that has HDR10 or HLG where the original has it, and that is at most the size limit.
- **Outcome** – what one attempt ended with:
  - `replaced` – the result took the original's place;
  - `no_gain` – the result grew above the size limit, while being written or when finished;
  - `failed` – with a German reason: encoder error, timeout, not a good result, the
    original changed, the swap failed, interrupted;
  - `refused` – nothing was started, with a German reason.

## Scope

### In scope

- Re-encoding one library file to HEVC (libx265) in Matroska, all other tracks copied
- Checking the result and swapping it for the original, the original to the recycle bin
- Updating the library: path, size and media info of the file
- Recording the outcome of every attempt
- Starting it by CLI (in the foreground) and by API (in the server's background)

### Out of scope

- The background job, the order of the candidate list, skipping files already done — because that is spec `reencode-job`
- A page in the web UI — because that is spec `ui-reencode`
- Hardware encoders, AV1 — because the owner chose HEVC by software encoder (D31)
- Re-encoding audio or dropping tracks — because audio is a small part of the size and dropping tracks loses languages the owner wants
- Renaming the file by the naming scheme (e.g. a `{MediaInfo VideoCodec}` token) — because a rename is the job of the existing rename feature; only the extension may change
- Changing the file's quality or custom format score in the library — because a lower quality would make the decision engine grab the original release again (an upgrade loop)
- Several encodes at the same time — because one libx265 encode already uses every CPU core
- Dolby Vision and HDR10+ files — because spec `reencode-rule` excludes them; a request for such a file is refused

## Acceptance criteria

- [ ] AC-01 (event): When the user runs `amigo-sofarr reencode` for a movie (`<movie id>`) or an episode (`--series <id> --season <n> --episode <n>`), then sofarr re-encodes that library file in the foreground, prints one German line when it starts and one with the outcome (for `replaced`: size before and after in GB and the gain in percent), and exits with 0 for `replaced` and `no_gain` and with an error otherwise.
- [ ] AC-02 (event): When a client posts `POST /api/reencode/encode` with a movie id or an episode id, then sofarr checks the request at once: a refusal (AC-10) is answered with `409` and the German reason (an unknown id with `404`), otherwise with `202`, and the encode runs in the server's background. `GET /api/reencode/encode` answers the running encode (title, path, start time, whether it runs in the server or a CLI) or none, and the latest recorded attempt, whichever way it was started.
- [ ] AC-03 (always): The result holds the original's first video track encoded as HEVC and every other video, audio, subtitle and attachment track of the original unchanged, in the original's order – except that MP4 text subtitles (`tx3g`/mov_text), which Matroska cannot hold, become SRT subtitles with the same text, language and flags, and MP4 data and timecode tracks, which no player shows, are left out.
- [ ] AC-04 (always): Nothing from the library reaches a shell or the encoder's options: a file whose name contains spaces, quotes, `;`, `$(…)`, or starts with `-`, is encoded and replaced under exactly that name.
- [ ] AC-05 (always): Where the system has the `nice` program, the encoder runs at niceness 19 (the lowest CPU priority); elsewhere at normal priority.
- [ ] AC-06 (event): When the result is a good result, then sofarr puts it in place under the final name, records the new path and size for the file in the library, reads and stores its media info (a failure there is only logged; the next disk scan reads it), then moves the original into the recycle bin (`recycleBin` of the movie or series media management settings, read at this moment) or, without one, deletes it. The file's quality, custom format score, scene name and episode links stay unchanged.
- [ ] AC-07 (error): If the result grows above the size limit, while being written or when finished, then sofarr stops the encoder, removes the work file, keeps the original untouched, and records `no_gain` with the size before and – where the result was finished – the size after.
- [ ] AC-08 (error): If the encoder fails, runs longer than `[reencode] timeout_hours` (default 24), or a finished result within the size limit is not a good result, then sofarr stops the encoder, removes the work file, keeps the original untouched, and records `failed` with the reason.
- [ ] AC-09 (error): If, between the start of the attempt and the swap, the original's size or modification time changed (e.g. an upgrade was imported), the original is gone, or the library no longer records it at the same path, or the final name is taken by another file, then sofarr discards the result, touches no other file, and records `failed` with the reason.
- [ ] AC-10 (error): sofarr refuses to start, records `refused` with a German reason (where the request names a library file) and changes nothing, when: it runs in shadow mode; ffmpeg cannot be run or has no libx265; the id is unknown or has no library file; the library file is missing on disk; the original's media info is not complete or has no video width and height; the original carries Dolby Vision or HDR10+; the free space in its folder is below the size limit plus the import's minimum (`minimumFreeSpaceWhenImporting`, unless `skipFreeSpaceCheckWhenImporting`); or the encode slot is taken.
- [ ] AC-11 (error): If any step of the swap up to and including the library update fails, then the original is back at its path, the library still records it with its old size, no work file is left, and the outcome is `failed` with the reason; at no point are both files gone. If only the last step fails (moving the original to the recycle bin or deleting it), the outcome is `replaced`, the original stays in the folder under a hidden name ending in `.reencode-original`, and the reason names it.
- [ ] AC-12 (event): When an attempt starts, then sofarr first cleans up what an interrupted attempt on the same library file left, before any check of AC-10: an original left under its hidden name is renamed back to its path where that path is empty, or else (the swap was complete) moved to the recycle bin or deleted; then any work file is removed; and every recorded attempt without an end that no running encode holds is closed as `failed` ("abgebrochen").
- [ ] AC-13 (always): A request for a file that is not a candidate (fine by its size, or in a switched-off resolution class) is carried out too: the request is the user's decision; only the refusals of AC-10 apply.
- [ ] AC-14 (event): When the result of an MP4 original takes its place, then the library records the `.mkv` path, and sofarr's own subtitles of the video keep their files and records, still named after the video (the name without extension does not change).
- [ ] AC-15 (always): sofarr records every attempt on a library file with the file, its title and path, start and end time, outcome, reason, size before, and size after where there was a finished result; the records stay when the file is later replaced or removed.
- [ ] AC-16 (option): Where `[reencode] crf` (0–51, default 22), `preset` (one of x265's presets `ultrafast` … `placebo`, default `medium`), `threads` (0 = all cores, default 0), `min_saving_percent` (1–99, default 20) or `timeout_hours` (at least 1, default 24) is set, sofarr uses it; an invalid value stops sofarr (server and CLI) before it does anything, with a German message naming the key, and `config/sofarr.example.toml` shows the keys with their defaults.
- [ ] AC-17 (event, manual: real ffmpeg in the container): When the owner runs `amigo-sofarr reencode` in the container on an SDR Matroska file, an HDR10 file and an MP4 file with subtitles of the own library, then each ends `replaced` (or `no_gain`), and Jellyfin plays the results with all audio and subtitle tracks and, for the HDR10 file, in HDR.

## Technical notes

- Encoder call (the shape, for the argument-list test): `ffmpeg -nostdin -hide_banner -n -i
  <original> -map 0 [-map -0:d] -c copy [-c:s srt] -c:v:0 libx265 -preset <preset> -crf <crf> -pix_fmt
  yuv420p10le [-x265-params pools=<threads>] -f matroska <work file>` (the pools
  parameter only where `threads` is above 0; the bracketed parts for MP4 originals: data and
  timecode tracks dropped, text subtitles to SRT), prefixed with `nice -n
  19` where `nice` can be run. ffmpeg passes HDR10/HLG colour information and the mastering
  and content light metadata to libx265; whether it arrived is part of the good-result check. 10-bit output for every file: it keeps HDR10 and encodes SDR
  more efficiently; every HEVC client the owner uses plays Main10.
- ffmpeg is found on `PATH` like for the trailers (spec trailers, no config key); the check
  is `ffmpeg -version` plus `libx265` in `ffmpeg -hide_banner -encoders`.
- The swap, in this order: the original is renamed aside to a hidden name in its folder
  (`.<name>.reencode-original`); the work file is renamed to the final name; the library is
  updated; then the original goes to the recycle bin (`bin/<item folder name>/<path below
  it>`, like an upgrade) or is deleted. A failure up to the library update renames both back.
- The encode slot is a lock file in the database folder, created exclusively and touched
  every minute while the encode runs; one not touched for 5 minutes is left over from a
  crash and is taken over.
- Running the encode reuses the trailers' time-limited process runner (std::process,
  polled, killed on timeout); the size limit kills the encoder as soon as the work file
  outgrows `(100 − min_saving_percent) %` of the original, which cannot become a good result.
- New table for the attempts (AC-15), with nullable links to the movie or episode file
  (`ON DELETE SET NULL`) and the title and path copied in.
- The API's encode runs in a server task; the CLI runs in its own process. Both take the
  encode slot.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A 60 GB 2160p HDR10 remux | Encoded 10-bit with HDR10 kept; replaced if at least 20 % smaller |
| An x265 file at 70 MB/min whose encode grows past 80 % of its size | Stopped early: `no_gain` without a size after; the original stays |
| `Film.mp4` with a `Film.mkv` next to it (the owner's own copy) | `failed`: final name taken; nothing touched |
| An upgrade is imported while the encode runs | The original changed: `failed`, result discarded, the new file stays |
| The server restarts during an API encode | The encoder dies with it; the next attempt removes the work file and closes the record as "abgebrochen" (AC-12) |
| Ctrl+C on the CLI | The encoder stops with it; the next attempt cleans up as after a restart |
| The movie is renamed while it is encoded | The library path changed: `failed`, the result is discarded (AC-09) |
| A Dolby Vision file below the threshold is requested | Refused (AC-10), whatever the candidate list says |
| The CLI is started while the server encodes another file | Refused: the encode slot is taken |
| An unknown movie id | CLI error and API `404`, nothing recorded |
| A file with PGS subtitles and a font attachment | Both copied into the result |
| The recycle bin is on another file system | The original is copied there, then removed from the library folder |
| No recycle bin set | The original is deleted after the result is in place |
| A double episode (`S01E02-E03`) | Its one file is re-encoded; both episodes stay linked to it |
| ffmpeg is missing (local development on Windows) | `refused`: "ffmpeg mit libx265 nicht gefunden" |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| CRF 22, preset `medium` | Visually close to the source at about a third to a half of a remux's size; `medium` takes some hours per 4K film | Too soft or too slow for the owner's taste; both are keys |
| Jellyfin treats `Film.mkv` after `Film.mp4` as the same item | Jellyfin matches the movie or episode by folder and name; only the extension changed | The watched state of former MP4 files may be lost |
| A good result needs no full decode check | ffmpeg's exit code plus the header check (tracks, size, duration) catch broken output | A result damaged inside the stream would only show while playing; the original stays in the recycle bin for its cleanup days |
| A lock not touched for 5 minutes is left over | The runner touches it every minute | A system paused for longer could start a second encode |
| Free space of the size limit plus the import minimum is enough | The work file is stopped at the size limit; the original is moved aside, not copied | A recycle bin on the same file system keeps using the space until its cleanup |

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
| AC-11 |      | open   |        |
| AC-12 |      | open   |        |
| AC-13 |      | open   |        |
| AC-14 |      | open   |        |
| AC-15 |      | open   |        |
| AC-16 |      | open   |        |
| AC-17 |      | open   |        |

## Decisions

- 2026-11-17: The result is always Matroska; a good result needs a gain of at least 20 % (configurable); the job will run around the clock at low CPU priority, one file at a time; this spec comes before the job so single files can be tried first (owner).
- 2026-11-17: The original is renamed aside before the result takes its place and goes to the recycle bin only after the library is updated, so a failure in between can always restore it.
- 2026-11-17: MP4 text subtitles become SRT and MP4 data and timecode tracks are left out, because ffmpeg's Matroska muxer cannot copy them and the result is always Matroska; the track count check compares audio and subtitle tracks only.
- 2026-11-17: An interrupted swap is repaired at the start of the next attempt on the same file, before its checks, so a hidden original is never lost (AC-12); `reencode-job` repairs every file at its start.
