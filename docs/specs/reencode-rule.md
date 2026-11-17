---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §3, sofarr.md §5, sofarr.md §9 D22, sofarr.md §9 D31]
media: both
oracle: none
---

# Re-encode rule

## Context

The owner's library holds files that are far larger than their resolution needs (remuxes, very
high bitrates). D31 decides that sofarr re-encodes such files to HEVC with ffmpeg in a background
job and replaces the original. Radarr and Sonarr have no such feature, so there is no oracle.

The feature is split into three specs: this one decides **which** files are too large and shows
them; `reencode-job` encodes and swaps them; `ui-reencode` adds the page. This spec changes no
library file and therefore behaves the same in shadow and live mode: in shadow mode its list is
the answer to "what would sofarr re-encode".

Terms used below:

- **Library file** – the video file sofarr has recorded for a movie or for one or more episodes.
  A file that holds several episodes is one library file and appears once.
- **Stored media info** – what spec `mediainfo` records per library file at the import and the
  disk scan (which runs in shadow mode too): the header data of the first video track and the
  file's size and modification time at that read. The rule works on this stored data only and
  never opens a file; a file that changed or vanished since is seen at the next disk scan.
- **File size** – the size stored with the media info, in bytes.
- **Duration** – the playing time of a library file. Taken from the file's own header (stored
  with its media info); where the header carries none, from the metadata: a movie's runtime, or
  a series' episode runtime times the number of episodes in the file. A header duration that is zero, negative, not a number or MP4's marker for unknown
  (all bits set) counts as none; a metadata runtime of 0 is unknown. The list
  names which of the two sources was used.
- **Resolution class** – from the pixel width and height of the first video track: `2160p` when
  the width is above 1920 or the height above 1080; otherwise `1080p` when the width is above
  1280 or the height above 720; otherwise `720p`. A 1920×800 movie is `1080p`. SD falls into
  `720p`, because the owner's library has too few SD files to justify a fourth threshold.
- **MB, GB** – in this feature always binary: 1 MB = 1,048,576 bytes, 1 GB = 1,024 MB. Config
  keys, API and texts say MB and GB.
- **Size per minute** – the file size in MB divided by the duration in minutes (seconds / 60,
  not rounded).
- **Threshold** – the largest allowed size per minute of a resolution class, from
  `[reencode] max_mb_per_minute_720p` (default 30), `max_mb_per_minute_1080p` (default 60) and
  `max_mb_per_minute_2160p` (default 150). 0 switches the class off, so the owner can leave a
  whole resolution (e.g. 4K) untouched.
- **Verdict** – every library file gets exactly one, the first that applies in this order:
  1. excluded, "keine Media-Infos" – no stored media info, or it has no video width and height;
  2. excluded, "Auflösung ausgeschaltet" – the threshold of its resolution class is 0;
  3. excluded, "Laufzeit unbekannt" – no duration from header or metadata;
  4. **fine** – its size per minute is at or below the threshold;
  5. excluded, "Dolby Vision/HDR10+ würde verloren gehen" – the video track carries Dolby
     Vision or HDR10+ (plain HDR10 and HLG do not exclude);
  6. **candidate** – everything else: too large and nothing speaks against encoding it.
- **Candidate list** – all candidates, largest file first (by file size, movies and episodes
  mixed; equal sizes by path), because the owner wants the biggest files handled first.

## Scope

### In scope

- The duration of Matroska and MP4 files in the media info
- The pure rule that classifies a library file as candidate, fine, or excluded with a reason
- The `[reencode]` thresholds in `sofarr.toml`
- The candidate list through the API and a CLI command, in both modes

### Out of scope

- Encoding, replacing files, the schedule, the switch that turns the job on — because that is spec `reencode-job`; this spec stays read-only and safe to ship alone
- The page in the web UI — because that is spec `ui-reencode`
- Thresholds per movie, series or quality profile — because one rule per resolution is what the owner asked for; excluding single titles comes with `ui-reencode`
- Judging by bitrate of the video track alone — because the file size is what fills the disk, and audio tracks are part of it
- Comparing the duration with Radarr/Sonarr (`compare-mediainfo`) — because the duration serves only this rule; the comparison covers what naming and custom formats use

## Acceptance criteria

- [ ] AC-01 (always): sofarr reads the duration of a Matroska file and of an MP4/QuickTime file from its header and stores it with the file's media info; a file whose header carries no duration, or one that is zero, negative, not a number or the MP4 marker for unknown (all bits set), has none, and its other media info is the same as before this feature.
- [ ] AC-02 (state): While a library file's stored media info has no duration because it was read before this feature, sofarr leaves it unread as long as the file is unchanged (spec `mediainfo` AC-01 stands) and uses the metadata runtime for it; once the file is read again under that spec's rules, its header duration is stored and used.
- [ ] AC-03 (always): sofarr gives every library file exactly one verdict, the first that applies in the order of the term "Verdict"; a file whose size per minute is exactly the threshold is fine, one above it is not.
- [ ] AC-04 (always): A file that is fine by its size per minute is never reported as excluded for Dolby Vision or HDR10+, and a file in a switched-off resolution class is reported as "Auflösung ausgeschaltet" whatever else applies to it except missing media info.
- [ ] AC-05 (always): The duration from the file's header wins over the metadata runtime, however much they differ; the metadata runtime is used only where the header has none, for a file with several episodes multiplied by their number.
- [ ] AC-06 (event): When a client asks `GET /api/reencode/candidates`, then sofarr answers with the whole candidate list, largest file first, each entry with what it belongs to (movie, or series and its episodes), title, path, file size, duration and its source, resolution class, video codec, size per minute and threshold, plus a summary: the number of candidates, their total size, the number of fine files, and the number of excluded files per reason. The numbers of the summary add up to the number of library files.
- [ ] AC-07 (event): When the user runs `amigo-sofarr reencode-candidates`, then sofarr prints the candidate list in the same order, one line per file with size in GB, size per minute and threshold in MB, and the path, then one closing line with the number of candidates and their total size and one line per exclusion reason that has files; with no candidate, "Keine Datei ist zu groß." stands in place of the list and the closing line, and the reason lines stay. The command reads the database itself and works while the server runs.
- [ ] AC-08 (state): While sofarr runs in shadow mode, AC-06 and AC-07 answer exactly as in live mode; in both modes they change no file in the library and no row in the database.
- [ ] AC-09 (error): If a `[reencode]` threshold is negative or not a number, or the section has an unknown key, then sofarr refuses to start with a German message that names the key.
- [ ] AC-10 (option): Where no `[reencode]` section is set, the defaults 30 / 60 / 150 apply, and `config/sofarr.example.toml` shows the section with these values.
- [ ] AC-11 (always): For a library of 2,500 movies and 20,000 episode files, half of them candidates, sofarr builds the answer of AC-06 – reading the library from the database included – in under 2 s (`cargo test -p sofarr-server --release reencode_candidates_large_library`, measured duration in the test output; the limit holds on the developer's machine and on the CI runner).
- [ ] AC-12 (error): If the stored media info of a library file cannot be parsed, then that file counts as "keine Media-Infos" and the list is still answered.

## Technical notes

- Matroska: `Segment/Info/Duration` (float, in units of `TimestampScale`, default 1,000,000 ns).
  MP4/QuickTime: `moov/mvhd` `duration` / `timescale` (version 0 and 1; a timescale of 0 means
  no duration). Both sit in the header data the reader of spec `mediainfo` already walks (it
  seeks to `moov` wherever it lies and reads at most 16 MiB), so no further read is needed.
- API answer (snake_case like the other answers, no paging like the other lists):
  `{ candidates: [{ kind: "movie" | "episode", movie_id, series_id, episode_ids, title, path
  (absolute), size (bytes), duration_seconds, duration_source: "file" | "metadata",
  resolution: "720p" | "1080p" | "2160p", video_codec, mb_per_minute, threshold }], count,
  total_size (bytes), fine, excluded: { no_media_info, resolution_off, unknown_duration,
  dynamic_metadata } }`. `title` is the movie title, or series title with `SxxEyy` (first and
  last episode for several).
- CLI line: `{size} GB  {size per minute} > {threshold} MB/min  {path}`, numbers with one
  decimal and a German decimal comma; closing line `{n} Dateien, {total} GB`; reason lines
  `{n} × {reason}`.
- The rule is a pure function in `sofarr-core` on plain numbers and flags (size, duration,
  width, height, dynamic range, thresholds); the server feeds it from the library and the stored
  media info. `sofarr-core` gets no dependency on `sofarr-mediainfo`.
- The rule's result leaves room for further exclusion reasons: `reencode-job` adds "already
  re-encoded" and "no gain", `ui-reencode` adds "excluded by the user".
- Config: `[reencode] max_mb_per_minute_720p`, `max_mb_per_minute_1080p`,
  `max_mb_per_minute_2160p` (decimal numbers ≥ 0). `reencode-job` adds its keys to the same section.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 1080p movie, 120 min, 7,200 MiB (60 MiB/min, threshold 60) | Not too large (AC-03) |
| 1920×800 movie at 70 MiB/min | Class `1080p`, candidate |
| 3840×1600 movie at 140 MiB/min | Class `2160p`, not too large |
| Double episode (2 × 45 min by metadata) without a header duration, 5,000 MiB | 90 minutes, 55.6 MiB/min, source metadata |
| Header says 5 min, metadata says 120 min | The header wins (AC-05); 5 minutes are used |
| Movie with runtime 0 and no header duration | Excluded: "Laufzeit unbekannt" |
| 2160p Dolby Vision remux at 400 MiB/min | Excluded with its reason, counted in the summary (AC-06) |
| A file that is already HEVC but above the threshold | Candidate; whether encoding pays off is decided by `reencode-job` |
| Movie without a file | Not in the list, not counted |
| The library is empty | Empty list, all counts 0; the CLI prints "Keine Datei ist zu groß." |
| A 2160p Dolby Vision file below the threshold | Fine, not excluded (AC-04) |
| `max_mb_per_minute_2160p = 0`, a 4K Dolby Vision file without duration | "Auflösung ausgeschaltet" (the first verdict that applies) |
| A file was deleted on disk after the last scan | Still listed from the stored data until the next disk scan forgets it; `reencode-job` checks the file before it encodes |
| A double episode | One entry with both episode ids |
| Header duration 0 or MP4 timescale 0 | No header duration; the metadata runtime is used |
| A movie recorded before this feature, file unchanged | No header duration is stored; the metadata runtime is used, source `metadata` (AC-02) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Defaults 30 / 60 / 150 MiB per minute | About 4 / 8 / 20 Mbit/s: above a good HEVC encode, below remuxes and high-bitrate x264 | Too many or too few candidates; the owner tunes the keys with the list in shadow mode |
| Dolby Vision and HDR10+ files are excluded | A plain libx265 encode does not carry their dynamic metadata over | The largest 4K remuxes stay untouched |
| HDR10 and HLG files are candidates | Their static metadata survives an HEVC encode; `reencode-job` checks the result | If the check fails, the result is discarded and the file stays |
| A header duration is trusted even when it differs widely from the metadata | The file knows its own length; metadata runtimes are often rounded or wrong for extended cuts | A broken header value yields a wrong size per minute for that file |
| The codec plays no role in the rule | A 4K HEVC remux is as oversized as an AVC one | HEVC files are encoded a second time; `reencode-job` discards results without enough gain |
| Files recorded before this feature are judged by the metadata runtime | Reading every file again would break the rule of spec `mediainfo` that unchanged files are not read; `reencode-job` reads the file anyway before it encodes and decides again with the header duration | Extended cuts and wrong runtimes shift the size per minute until then |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `matroska::tests::the_duration_comes_from_the_segment_info`, `an_unusable_duration_is_none_and_changes_nothing_else`, `the_info_is_found_through_the_seek_head` (`crates/sofarr-mediainfo/src/matroska/tests.rs`); `mp4::tests::the_duration_comes_from_the_movie_header`, `an_unusable_movie_duration_is_none_and_changes_nothing_else` (`crates/sofarr-mediainfo/src/mp4/tests.rs`) | built |        |
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

## Decisions

- 2026-11-17: Too large is measured in size per minute per resolution class; the target is HEVC by software encoder; the original goes to the recycle bin; a background job works through the whole library, largest files first (owner; D31).
- 2026-11-17: The feature is split into `reencode-rule`, `reencode-job` and `ui-reencode`; the rule and its list come first so the thresholds can be tuned in shadow mode before anything is encoded.
- 2026-11-17: The duration extends the reader of spec `mediainfo`; that spec stays `implemented`, because none of its acceptance criteria names the set of fields and every value it specifies stays the same (AC-01).
- 2026-11-17: The rule works on stored data only, because opening 20,000 files over the network for a list would take minutes; the disk scan keeps the stored data current.
- 2026-11-17: Verdicts are ordered so that "fine" comes before the Dolby Vision exclusion: the summary then shows how many oversized files the exclusion really leaves untouched.
- 2026-11-17: The duration is stored as whole milliseconds (`duration_ms`), left out of the stored JSON when unknown, so media info stored before stays byte-identical and `MediaInfo` keeps `Eq`. A Matroska `Info` is read where the walk meets it or through the `SeekHead`, at most 1 MiB; a duration below half a millisecond counts as none.
