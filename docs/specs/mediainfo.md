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
| An audio track without language, or with `und` | Recorded as unknown language |
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

## Decisions

- 2026-11-12: Matroska/WebM and MP4/M4V/MOV only; other containers keep the release-name guess (owner).
- 2026-11-12: An oracle comparison against Radarr's/Sonarr's `mediaInfo` belongs to this spec; running it against the live instances is a manual step of the owner (owner).
