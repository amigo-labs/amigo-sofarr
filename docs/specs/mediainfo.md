---
status: todo
created: 2026-11-12
size: L
design: [sofarr.md §3, sofarr.md §5, sofarr.md §6, sofarr.md §9 D22]
media: both
oracle: GET /api/v3/moviefile, GET /api/v3/episodefile (mediaInfo)
---

# Media info from files

## Context

The owner's naming formats use `{.MediaInfo.Simple}` (movies and episodes) and
`{.MediaInfo.VideoDynamicRange}` (episodes). Radarr and Sonarr fill them from the file
(`enableMediaInfo = true` in both). sofarr reads nothing from files and guesses these tokens from
the release name (`docs/parity.md`, "enableMediaInfo: offen"), so a file whose name lacks the codec
or HDR marker is named differently than Radarr would name it. Spec `subtitles` also needs to know
which subtitle and audio languages a file already contains.

Radarr and Sonarr report what they read per file as `mediaInfo` (video codec, audio codec,
channels, dynamic range, audio and subtitle languages). That is the oracle: sofarr reads the same
files and compares (sofarr.md §5). The reader is sofarr's own, written from the public container
specifications (Matroska, RFC 9559; ISO base media file format) — no third-party tool, no Servarr
code (sofarr.md §6).

Terms used below:

- **Supported container** – Matroska/WebM and the MP4 family (MP4, M4V, QuickTime MOV), recognized
  by the file's content, not its extension.
- **Media info** – what sofarr records per file: container, video codec and encoder (e.g. x265,
  where the file names it), video resolution, dynamic range (SDR, HDR10, HDR10+, HLG, Dolby
  Vision, or a combination), each audio track (codec incl. object audio such as Atmos, channel
  layout, language), each subtitle track (language, format, forced, default, hearing impaired
  where marked), and a read status. Video fields come from the first video track in file order.
  An unknown codec is recorded by the container's raw codec id.
- **Read status** – `complete`: the headers of every track were read; `partial`: the file ended
  or the read limit (AC-05) was reached before every track header was read; `damaged`: the
  container structure is invalid; `unsupported`: the content is no supported container;
  `unreadable`: the file exists but cannot be opened or read (permission, IO error). Languages are
  stored as ISO 639-2/B codes (`ger`, `eng`, `und` for unknown).
- **Header data** – the container's headers plus the first frames of each track, within the read
  limit of AC-05; HDR10+, Dolby Vision in older files and Atmos are marked there, not always in
  the headers alone.
- **Oracle value** – the `mediaInfo` Radarr or Sonarr reports for the same file.
- **Label** – the text sofarr derives from one media info field, in the vocabulary below; the
  naming tokens are built from labels: `{MediaInfo Simple}` = video label + space + audio label of
  the first audio track in file order (e.g. `x265 EAC3 Atmos`); `{MediaInfo VideoDynamicRange}`
  = dynamic range label (empty for SDR).
- **Starting vocabulary** – video label: the encoder when it is `x264` or `x265`, otherwise the
  codec (`h264`, `h265`, `AV1`, `VP9`, `MPEG2`, `VC1`, `XviD`, `DivX`); audio label: `AAC`, `AC3`,
  `EAC3`, `DTS`, `DTS-ES`, `DTS-HD HRA`, `DTS-HD MA`, `DTS-X`, `TrueHD`, `FLAC`, `Opus`, `Vorbis`,
  `MP3`, `PCM`, with ` Atmos` appended for object audio; channels as `2.0`, `5.1`, `7.1`;
  dynamic range label: `HDR`, `HDR10+`, `HLG`, `DV`, `DV HDR10`, `DV HDR10+`, `DV HLG`, empty
  for SDR. The oracle comparison may replace words of this vocabulary (AC-07); the replaced
  words are recorded in the spec's Decisions.
- **Oracle field** – the per-file `mediaInfo` value of Radarr/Sonarr compared with a label:
  `videoCodec` ↔ video label, `audioCodec` ↔ audio label (first track), `audioChannels` ↔
  channels, `videoDynamicRangeType` ↔ dynamic range label, `audioLanguages` / `subtitles` ↔ the
  languages of all audio / subtitle tracks.

## Scope

### In scope

- Reading media info from supported containers, offline and without external programs
- Storing it for movie and episode files at import, at the disk scan, and for existing files
- Filling the `{MediaInfo…}` naming tokens of the owner's formats from it, with the release-name guess as fallback
- Exposing media info in the API for each movie and episode file
- An oracle comparison command against Radarr's and Sonarr's `mediaInfo`

### Out of scope

- AVI, MPEG-TS/M2TS and other containers — because current releases are almost all MKV or MP4 (owner); those files keep the release-name guess
- Decoding or checking that frames play — because only the container's track headers are needed
- Running ffprobe or MediaInfo — because the image stays a single static binary and the reader must work offline
- `{MediaInfo…}` tokens the owner's formats do not use (e.g. `{MediaInfo AudioLanguages}`) — because parity targets the own setup (sofarr.md §2); the stored data allows adding them later
- Using media info in grab decisions or custom formats — because the owner's custom formats only use language, release group and release title conditions (`docs/parity.md`)

## Acceptance criteria

- [ ] AC-01 (event): When sofarr imports a file or the disk scan finds a new or changed file (size or modification time differs from when it was last read), then sofarr reads and stores its media info; an unchanged file is not read again, whatever its read status — except `unreadable`, which is retried at every scan.
- [ ] AC-02 (event): When the disk scan meets a library file in a supported container that has no media info yet, then sofarr reads and stores it, so existing libraries get media info without re-importing.
- [ ] AC-03 (always): For a supported container sofarr records, as far as the header data shows them, the video codec and encoder, resolution, dynamic range, every audio track with codec, channel layout and language, and every subtitle track with language, format and the forced, default and hearing-impaired flags; a file without a video track has the video fields unknown.
- [ ] AC-04 (error): If a file is cut short, damaged, not a supported container, or cannot be opened (permission, IO error), then sofarr never panics and never blocks the import or the scan: it records what it could read with the status `partial`, `damaged`, `unsupported` or `unreadable`, and the rest as unknown.
- [ ] AC-05 (always): Reading media info of a file of any size reads at most 16 MiB of its bytes in total; bytes skipped by seeking do not count.
- [ ] AC-06 (event): When a naming format uses `{MediaInfo Simple}` or `{MediaInfo VideoDynamicRange}` (in any of their separator forms), then sofarr fills each part of the label (video codec, audio, dynamic range) from the file's media info where that field is known, and only the parts with an unknown field from the guess from the release name.
- [ ] AC-07 (always, manual: needs the owner's library and running Radarr/Sonarr): In the owner's oracle comparison, the video, audio and dynamic range labels equal the oracle field for at least 95 % of the compared files per field; the remaining mismatches are listed by cause in `docs/parity.md`. Where the oracle consistently uses another word than the starting vocabulary, the vocabulary takes the oracle's word.
- [ ] AC-08 (event): When the user runs the oracle comparison against Radarr or Sonarr, then sofarr reads every library file the oracle reports and compares each label with its oracle field (languages mapped from the oracle's language names to ISO 639-2/B; a name that cannot be mapped counts as a mismatch showing the raw name), and prints pass/total per field plus the first 20 mismatches per field with file name, expected and found value.
- [ ] AC-09 (error): If a file of the oracle cannot be read by sofarr (missing, no permission, unsupported container) or the oracle has no `mediaInfo` for it, then the comparison counts it as skipped with the reason, not as a mismatch, and continues; if the oracle itself cannot be reached or refuses the API key, the command stops with a German message (exit code ≠ 0).
- [ ] AC-10 (event): When the user requests the file of a movie or of an episode through the API, then the answer contains its media info and read status (fields in Technical notes); an item without a file answers status 404.
- [ ] AC-11 (always): Reading media info never writes to the file and never changes its modification time.
- [ ] AC-12 (state): While sofarr runs in shadow mode, the disk scan still reads and stores media info, because it only reads files.

## Technical notes

- A new pure crate `sofarr-mediainfo` working on `Read + Seek`, without file-system access of its
  own; IO stays in `sofarr-server`. It depends on nothing of sofarr and is used by `sofarr-server`
  (D22).
- Matroska: EBML header, `Segment` → `SeekHead`, `Info`, `Tracks` (`TrackEntry`: `CodecID`,
  `Language`/`LanguageBCP47`, `FlagForced`, `FlagDefault`, `FlagHearingImpaired`, `Video` →
  `Colour` (`TransferCharacteristics`, `MasteringMetadata`), `BlockAdditionMapping` for Dolby
  Vision). The tracks sit near the start; `SeekHead` points to them when they don't.
- MP4: `moov` → `trak` → `mdia` → `hdlr`/`mdhd` (language)/`minf` → `stbl` → `stsd` sample entries
  (`avc1`/`hvc1`/`hev1`/`av01`, `dvcC`/`dvvC`, `colr`, `mp4a`/`ac-3`/`ec-3`/`dac3`/`dec3`). `moov`
  may sit at the end of the file: seek there instead of reading through `mdat`.
- Storage: new columns or a JSON column for movie and episode files (migration), plus the size and
  modification time it was read at (AC-01).
- API: `GET /api/movies/{id}/file`, `GET /api/series/{id}/episodes/{episode-id}/file`; the answer
  has `path`, `size`, `quality` and `media_info` with `status`, `container`, `video`
  (`codec`, `encoder`, `width`, `height`, `dynamic_range`), `audio[]` (`codec`, `channels`,
  `object_audio`, `language`), `subtitles[]` (`language`, `format`, `forced`, `default`,
  `hearing_impaired`) and `labels` (`video`, `audio`, `dynamic_range`).
- AC-06's per-part fallback splits today's name guess (`media_info_from_name`,
  `dynamic_range_from_name`) into a video part and an audio part.
- Encoder: Matroska tags (`ENCODER`, writing application) and, for H.264/HEVC, the encoder
  string in the first frames' user-data SEI; HDR10+ from the dynamic-metadata SEI of the first
  frames; Atmos from the E-AC-3 extension (JOC) or the TrueHD stream header of the first frames.
- The oracle data is the owner's library (private); the tests use synthetic byte fixtures and a
  fake Radarr answering `moviefile`/`episodefile` for the comparison command.
- Oracle command: `amigo-sofarr compare-mediainfo [--app radarr|sonarr]`, using the existing
  Radarr/Sonarr clients and path mapping of the `compare-*` commands.
- Byte fixtures: minimal MKV and MP4 headers built by the tests themselves (no copyrighted media),
  including truncated and corrupted variants.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| MP4 with `moov` at the end (not "fast start") | Read via seeking; still within the read limit (AC-05) |
| MKV whose `Tracks` element is not at the start | Found through `SeekHead` |
| A Matroska track without a `Language` element | Recorded as English (`eng`), the element's default in RFC 9559 |
| A track with language `und` | Recorded as unknown language |
| A subtitle track with a three-letter (`ger`) or BCP 47 (`de-DE`) language | Recorded as the same language (German) |
| Dolby Vision with an HDR10 fallback layer | Dynamic range Dolby Vision plus HDR10; label as the oracle shows it (AC-07) |
| A file with two audio tracks (German first, English second) | `{MediaInfo Simple}` uses the German one (first in file order); all tracks are stored |
| A file is replaced on disk with the same name | Size or modification time differs, media info is read again (AC-01) |
| A file disappears between the scan listing and reading it | Skipped for this scan; no media info or status is stored, because the disk scan handles the missing file |
| A file is still being written while it is read | Recorded as read; its size or modification time changes, so the next scan reads it again (AC-01) |
| A codec the vocabulary does not know | Recorded and labelled by its raw codec id |
| A `.mkv` that is actually an MP4 (wrong extension) | Detected by its content, read as MP4 |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 16 MiB per file are enough for the headers | Track headers are kilobytes; `SeekHead` and a trailing `moov` are reached by seeking, not reading | A file with unusually large headers is recorded as `partial` |
| Labels follow the oracle where it is observable | The owner's file names must match Radarr's/Sonarr's (parity) | Codec names the oracle never showed are sofarr's own choice |
| The oracle fields can be derived from track headers and first frames | Radarr/Sonarr also name codec, channels, HDR and languages, which the containers carry in these places | A field the oracle takes from deep inside the stream misses the 95 % (AC-07) and is listed as a deviation |
| HDR10+, older Dolby Vision and Atmos are visible within the first 16 MiB | Their markers sit in the first frames of a track | A file whose first frames lack them is recorded without; the comparison shows how often |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `media_tests::the_disk_scan_reads_new_and_changed_files_only`, `media_tests::an_import_names_and_records_from_the_file`, `media_tests::broken_files_get_a_status_and_unreadable_ones_are_retried` (`crates/sofarr-server/src/media_tests.rs`); `media_import_tests::an_episode_import_names_and_records_from_the_file` (`crates/sofarr-server/src/media_import_tests.rs`); `media_refresh_tests::an_unchanged_file_is_not_read_again_whatever_its_status`, `media_refresh_tests::a_change_of_only_size_or_only_time_reads_again`, `media_refresh_tests::a_file_written_during_the_read_is_read_again_at_the_next_scan` (`crates/sofarr-server/src/media_refresh_tests.rs`); `media::tests::media_info_follows_its_file` (`crates/sofarr-db/src/media.rs`) | built |  |
| AC-02 | `media_tests::the_disk_scan_reads_new_and_changed_files_only` (`crates/sofarr-server/src/media_tests.rs`) | built |  |
| AC-03 | `matroska::tests::tracks_languages_and_flags`, `matroska::tests::colour_marks_hdr10_and_hlg`, `matroska::tests::dolby_vision_from_block_addition_mapping`, `matroska::tests::x265_encoder_from_sei_in_first_simple_block`, `matroska::tests::encoder_from_hvcc_sei_and_from_tags`, `matroska::tests::hdr10_plus_from_t35_sei`, `matroska::tests::eac3_atmos_from_joc`, `matroska::tests::dts_profiles_and_truehd_atmos_from_first_frames`, `matroska::tests::legacy_codec_ids`, `matroska::tests::tracks_reachable_only_through_seek_head`, `matroska::tests::a_file_without_video_track_has_the_video_fields_unknown` (`crates/sofarr-mediainfo/src/matroska/tests.rs`); `mp4::tests::aac_channels_language_and_x264_sample`, `mp4::tests::quicktime_brand_and_chapter_text_tracks`, `mp4::tests::content_decides_not_the_extension` (`crates/sofarr-mediainfo/src/mp4/tests.rs`); `robustness_tests::complete_fixtures_read_completely` (`crates/sofarr-mediainfo/src/robustness_tests.rs`); `matroska::tests::webm_is_its_own_container`, `matroska::tests::header_stripping_is_undone_before_frame_analysis`, `matroska::tests::tracks_after_sized_clusters_are_found_without_seek_head` (`crates/sofarr-mediainfo/src/matroska/tests.rs`) | built |  |
| AC-04 | `robustness_tests::truncated_files_are_partial_or_damaged_at_every_cut`, `robustness_tests::corrupted_and_random_bytes_never_panic`, `robustness_tests::reader_errors_are_unreadable`, `robustness_tests::empty_and_foreign_inputs_are_unsupported` (`crates/sofarr-mediainfo/src/robustness_tests.rs`); `mp4::tests::invalid_box_sizes_are_damage_and_missing_moov_is_partial` (`crates/sofarr-mediainfo/src/mp4/tests.rs`); `matroska::tests::other_doc_types_and_invalid_structures` (`crates/sofarr-mediainfo/src/matroska/tests.rs`); `media_tests::broken_files_get_a_status_and_unreadable_ones_are_retried` (`crates/sofarr-server/src/media_tests.rs`); `app::tests::live_pipeline_grabs_and_imports` (`crates/sofarr-server/src/app/tests.rs`, an unsupported `.mkv` is imported) | built |  |
| AC-05 | `robustness_tests::huge_input_reads_at_most_the_limit` (`crates/sofarr-mediainfo/src/robustness_tests.rs`); `mp4::tests::moov_after_a_huge_mdat_is_reached_by_seeking`, `mp4::tests::moov_beyond_the_limit_is_partial_without_reading_it` (`crates/sofarr-mediainfo/src/mp4/tests.rs`); `media_refresh_tests::reading_a_file_reads_at_most_the_limit_from_the_underlying_reader` (`crates/sofarr-server/src/media_refresh_tests.rs`) | built |  |
| AC-06 | `media_tests::naming_takes_each_part_from_the_file_where_known`, `media_tests::an_import_names_and_records_from_the_file` (`crates/sofarr-server/src/media_tests.rs`); `media_import_tests::an_episode_import_names_and_records_from_the_file` (`crates/sofarr-server/src/media_import_tests.rs`); `naming::tests::media_info_parts_come_from_the_file_where_known` (`crates/sofarr-core/src/naming.rs`) | built |  |
| AC-07 |      | open   |        |
| AC-08 | `compare_media_tests::the_comparison_reports_per_field_and_skips_with_reasons`, `compare_media_tests::sonarr_episode_files_are_compared`, `compare_media_tests::an_unknown_language_name_is_a_mismatch_with_its_raw_name`, `compare_media_tests::at_most_twenty_mismatches_are_listed_per_field`, `compare_media_tests::oracle_codes_map_like_names_and_unknown_languages_are_left_out_like_the_oracle` (`crates/sofarr-server/src/compare_media_tests.rs`); `language::tests::names_map_to_codes` (`crates/sofarr-mediainfo/src/language.rs`) | built |  |
| AC-09 | `compare_media_tests::the_comparison_reports_per_field_and_skips_with_reasons`, `compare_media_tests::unreadable_and_unsupported_files_are_skipped_with_their_reason`, `compare_media_tests::an_unreachable_oracle_stops_with_a_german_message`, `compare_media_tests::a_refused_api_key_stops_with_a_german_message` (`crates/sofarr-server/src/compare_media_tests.rs`); `an_unreachable_or_refusing_oracle_ends_with_a_german_error` (`crates/sofarr-server/tests/cli_compare_mediainfo.rs`) | built |  |
| AC-10 | `media_tests::the_file_endpoints_answer_with_media_info`, `media_tests::the_episode_file_endpoint` (`crates/sofarr-server/src/media_tests.rs`) | built |  |
| AC-11 | `robustness_tests::reading_leaves_the_bytes_unchanged` (`crates/sofarr-mediainfo/src/robustness_tests.rs`); `media_tests::the_disk_scan_reads_new_and_changed_files_only` (`crates/sofarr-server/src/media_tests.rs`) | built |  |
| AC-12 | `media_tests::the_disk_scan_reads_new_and_changed_files_only` (`crates/sofarr-server/src/media_tests.rs`) | built |  |

## Decisions

- 2026-11-12: Matroska/WebM and MP4/M4V/MOV only; other containers keep the release-name guess (owner).
- 2026-11-12: An oracle comparison against Radarr's/Sonarr's `mediaInfo` belongs to this spec; running it against the live instances is a manual step of the owner (owner).
- 2026-11-12: Media info lives in its own table `media_info` (one row per movie file or episode file, JSON of `sofarr-mediainfo` plus size, modification time and read time); it is deleted by foreign key when its file row is deleted. A file replaced under the same row (`set_file` is an upsert on the movie) keeps the old row until it is read again: an import stores the new file's media info right away, the disk scan reads it because size or modification time differ. A JSON column keeps the reader's model free to grow without migrations.
- 2026-11-12: A file is read again when its size or modification time (RFC 3339, UTC) differs from the stored ones or its stored status is `unreadable`; a file that cannot even be looked at (metadata error other than "not found") is stored as `unreadable` with size 0.
- 2026-11-12: At import the videos are read in the download folder before they are moved, so the name of the target file already uses the file's media info; the stored row is keyed to the recorded library file.
- 2026-11-12: The API answer's `labels` also carries `channels` (e.g. `5.1`), because the comparison and the web UI show it and it costs nothing; the fields of the Technical notes are all present.
- 2026-11-12: `compare-mediainfo` reuses the Radarr/Sonarr client of the other `compare-*` commands and maps library paths through sofarr's own movie and series folders (matched by `radarr_id` / `sonarr_id`); an oracle file sofarr does not know is skipped with the reason "nicht in sofarrs Bibliothek".
- 2026-11-12: A file without a video track keeps the release-name guess for `{MediaInfo VideoDynamicRange}`, because SDR cannot be told from a missing track.
- 2026-11-12: `a_file_without_video_track_has_the_video_fields_unknown` was green at once: the reader already left the video fields empty; the test pins the behavior.
- 2026-11-12: A file cut inside Matroska `Tracks` or inside a trailing MP4 `moov` keeps the complete `TrackEntry`/`trak` children before the cut, with status `partial`; a damaged complete child is still `damaged`. A `Tracks`/`moov` larger than the remaining read budget is still not read (`partial` without tracks), as `moov_beyond_the_limit_is_partial_without_reading_it` pins.
- 2026-11-12: The server reads files unbuffered (`media::read_file`): a `BufReader` refilled 8 KiB after every seek and read about 200 MB from disk for a 24 MiB file of small skipped elements; the reader already asks for whole elements.
- 2026-11-12: The disk scan stores the size and modification time taken before the read, so a file that changes while it is read is read again at the next scan; only an import takes them from the library file after the move or copy (`media::store_moved`).
- 2026-11-12: The Radarr/Sonarr API client names the app it talks to (`Radarr::sonarr`), so a failing Sonarr says "Sonarr: …"; HTTP 401/403 stop with "API-Key abgelehnt".
- 2026-11-12: The comparison tests moved to `compare_media_tests.rs` and the refresh tests live in `media_refresh_tests.rs`, to keep the test files below 800 lines.
- 2026-11-12: Green at once, pinning existing behavior (each checked by a mutation that turns it red): `an_episode_import_names_and_records_from_the_file`, `an_unchanged_file_is_not_read_again_whatever_its_status`, `a_change_of_only_size_or_only_time_reads_again`, `sonarr_episode_files_are_compared`, `an_unknown_language_name_is_a_mismatch_with_its_raw_name`, `at_most_twenty_mismatches_are_listed_per_field`, `unreadable_and_unsupported_files_are_skipped_with_their_reason`, and the extended field checks of `the_file_endpoints_answer_with_media_info` and `the_episode_file_endpoint`.
- 2026-11-12: The encoder is recorded only where the file names it as x264 or x265 (the H.264/HEVC user-data SEI or the Matroska `ENCODER` tag); the writing application (`WritingApp`) is a muxer, not the encoder, so it is not read.
- 2026-11-12: A library file whose media info was not read yet answers `media_info: null` in the file endpoints; the read status exists only once the file was read.
- 2026-11-12: `cli_compare_mediainfo::an_unreachable_or_refusing_oracle_ends_with_a_german_error` was green at once: it pins the exit code and messages the command already had.
- 2026-11-12: `compare-mediainfo --root <folder>` reads the files below the root folder as the comparing machine sees it (e.g. a network share), because the owner compares from another machine than the library's; sofarr replaces the item's root folder (the imported `rootfolder` / `sonarr_rootfolder`) by it and keeps the path below, so letter folders like `/movies/A/…` work too. `scripts/compare-acceptance.ps1` runs the comparisons of AC-07 and of web-ui-library AC-23 in one go and keeps their logs under `target/acceptance/`.
- 2026-11-12: The owner's comparison against Sonarr (3,533 files) showed the oracle's dynamic range words consistently: `HDR10` for `HDR`, `HDR10Plus` for `HDR10+`, `DV HDR10Plus` for `DV HDR10+`; the vocabulary takes them (AC-07), also for the `{MediaInfo VideoDynamicRangeType}` naming token. The release-name fallback without media info keeps `HDR` and `DV HDR10`.
- 2026-11-12: A Matroska track without a `Language` element is English: RFC 9559 gives the element the default `eng`, and muxers omit it for English tracks. The owner's comparison against Radarr (2,341 files) showed it: sofarr reported `und` where Radarr said `eng` for 59 % of the audio and 32 % of the subtitle language lists. The edge case "an audio track without language" is revised accordingly; an explicit `und` and MP4's `mdhd` language stay as they are.
- 2026-11-12: The channel count also comes from the first frame where the header can be wrong (Radarr comparison: 22 of 2,341 mismatches). A DTS core frame gives AMODE plus LFE (ETSI TS 102 114); some muxers write the core's channels without the LFE into the Matroska header (`5` for 5.1), and because extensions (ES, HD) only add channels, the larger of header and core count wins. An AC-3 syncframe gives `acmod` plus `lfeon` (ETSI TS 102 366) and replaces the header's count, because AC-3 has no extensions; a broadcast recording's header claimed 5.1 for a stereo stream. Three channels stay `2.1`: the stored count does not say whether one of them is the LFE.
- 2026-11-12: The comparison maps the oracle's language codes like its names (some files show `deu`, which is `ger`), and leaves undetermined languages out on both sides, because the oracle lists only tracks of known language (Radarr comparison: `ger` against `ger/und`).
