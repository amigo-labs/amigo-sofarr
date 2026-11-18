---
status: todo
created: 2026-11-17
size: M
design: [sofarr.md §9 D22, sofarr.md §9 D31]
media: both
oracle: none
---

# Re-encode lossless audio

## Context

Spec `reencode-file` re-encodes the video of a library file and copies every audio track.
In remuxes, lossless audio tracks take 2–6 GB each – after the video shrinks they are a
large part of the file. The owner decided (2026-11-17) that lossless tracks without object
audio (Dolby Atmos, DTS:X) become E-AC-3 in the same encode; object audio stays as it is,
because ffmpeg cannot encode its object metadata and the owner wants to keep the spatial
sound. Radarr and Sonarr have no such feature, so there is no oracle.

Terms used below:

- **Attempt**, **result**, **good result**, **size limit** – as in spec `reencode-file`.
- **Convertible track** – an audio track of the original whose codec, as the media info
  reader of spec `mediainfo` names it, is `truehd` without object audio, `pcm` or `flac`.
  Not convertible: TrueHD with Atmos (the reader marks it as object audio), every DTS track
  including DTS-HD MA (the reader cannot tell DTS:X from plain DTS-HD MA in the header data
  it reads, and a converted DTS:X track would lose its objects), and all lossy codecs (AC-3,
  E-AC-3, DTS core, AAC, Opus …), because encoding them again only loses quality.
- **Converted track** – a convertible track in the result: E-AC-3 with at most 6 channels
  (an original with more is mixed down to 5.1, as ffmpeg's E-AC-3 encoder writes at most
  5.1; one with 6 or fewer keeps its channels), and the language, title and default/forced
  flags of the original. Its bit rate is the configured one for 3 and more channels and 256
  kbit/s for 1 or 2 channels, because a higher rate adds nothing to stereo.
- **Efficient video** – the first video track is already HEVC (the reader names it `h265`).

## Scope

### In scope

- Converting convertible tracks to E-AC-3 in the encode of spec `reencode-file`
- Copying an efficient video instead of encoding it again when the file has a convertible
  track
- Checking the converted tracks in the result
- The config switch and bit rate

### Out of scope

- Converting object audio (Atmos, DTS:X) or any DTS track — because the owner wants to keep spatial sound and sofarr cannot rule out DTS:X in a DTS-HD MA track
- Dropping audio tracks or languages — because the owner keeps every language
- Choosing the codec (AAC, Opus) — because E-AC-3 plays on every client the owner uses and passes through to AV receivers
- A separate rule for "too large because of audio" — because the size per minute of spec `reencode-rule` already counts the audio

## Acceptance criteria

- [ ] AC-01 (always): Where `[reencode] convert_lossless_audio` is `true` (the default), sofarr writes every convertible track of the original as a converted track and copies every other audio track unchanged, in the original's order.
- [ ] AC-02 (always): sofarr never converts an audio track that is not convertible: TrueHD with Atmos, every DTS track (DTS-HD MA, DTS-HD HRA, DTS-ES, DTS) and every lossy track stay unchanged.
- [ ] AC-03 (option): Where `[reencode] convert_lossless_audio = false` is set, every audio track is copied unchanged, as before this spec.
- [ ] AC-04 (option): Where `[reencode] audio_bitrate_kbps` is set (one of 384, 448, 640, 768, 1024 – common E-AC-3 rates for 5.1 and 7.1-to-5.1; default 640), the encoder is asked for that bit rate for converted tracks with 3 and more channels (and 256 kbit/s for 1 or 2); any other value stops sofarr at the start with a German message naming the key.
- [ ] AC-05 (always): A result is good only when, besides the checks of spec `reencode-file`, each converted track is E-AC-3 with at most 6 channels and the original track's language, and each copied audio track has the original's codec and channel count; otherwise the result is discarded and the attempt is `failed` with the reason.
- [ ] AC-06 (event): Where `convert_lossless_audio` is `true`, when the original's video is efficient and it has at least one convertible track, then sofarr copies the video unchanged and only converts the audio; the result must still be at most the size limit, or the attempt is `no_gain`.
- [ ] AC-07 (event): When the original's video is efficient and it has no convertible track (or `convert_lossless_audio` is `false`), then sofarr encodes the video again, as before this spec (spec `reencode-file`).
- [ ] AC-08 (event, manual: real ffmpeg in the container): When the owner re-encodes a file of the own library with a TrueHD 7.1 track without Atmos and a TrueHD Atmos track, then the first plays as E-AC-3 5.1 and the second still as Atmos in Jellyfin.

## Technical notes

- ffmpeg: per convertible track `-c:a:<n> eac3 -b:a:<n> <rate>k`, for 7.1 also `-ac:a:<n> 6`;
  the copy default of spec `reencode-file` stays for all other tracks. With an efficient
  video and a convertible track: `-c:v:0 copy` instead of libx265.
- Audio track numbers in ffmpeg (`a:<n>`) follow the order of the audio tracks, which is the
  order the media info reader lists them in.
- Config: `[reencode] convert_lossless_audio` (default `true`), `audio_bitrate_kbps` (default
  640); `config/sofarr.example.toml` shows both.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| 4K remux: TrueHD Atmos (de), TrueHD 7.1 (en), AC-3 (commentary) | Atmos copied, 7.1 → E-AC-3 5.1 640k, AC-3 copied |
| 1080p remux: DTS-HD MA 5.1 + PCM 2.0 | DTS-HD MA copied (maybe DTS:X), PCM → E-AC-3 2.0 |
| HEVC file with FLAC 5.1 | Video copied, FLAC → E-AC-3; replaced if at most the size limit |
| HEVC file with only E-AC-3 tracks | Video encoded again (AC-07) |
| An MP4 original with ALAC | Not convertible (not in the list); copied |
| The reader could not tell the channels of a PCM track | Converted without a mix-down; if it has more than 6 channels the encoder fails and the attempt is `failed` with ffmpeg's reason |
| A bloated HEVC file (70 MB/min) with a small PCM commentary track | Video copied, only the PCM converted; most likely `no_gain`, then settled (see Assumptions) |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| 640 kbit/s E-AC-3 is transparent enough | The rate of streaming services' best 5.1 tracks | A listener with a high-end system may hear a difference; the rate is a key |
| The reader's Atmos marker on TrueHD is right | It is read from the TrueHD headers of the first frames (spec `mediainfo`), which every Atmos track carries | A TrueHD Atmos track whose first frames lack the marker would be converted and lose its objects |
| Copying an HEVC video when there is audio to convert is the better trade | A second HEVC encode loses quality for little gain; lossless audio is where the bytes are | An HEVC file bloated by its video and with a small lossless track ends `no_gain` and is settled; the owner can encode it by hand with the switch off |
| A DTS-HD MA track may carry DTS:X | sofarr reads only the first frames' headers; DTS:X metadata sits in extension assets | DTS-HD MA tracks without DTS:X keep their size |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `reencode::audio::tests::only_lossless_tracks_without_object_audio_are_converted` (`crates/sofarr-server/src/reencode/audio.rs`); `reencode::run::tests::audio::lossless_tracks_without_atmos_become_eac3` (`crates/sofarr-server/src/reencode/run/tests.rs`); `reencode::ffmpeg::tests::converted_audio_and_a_copied_video_are_asked_for` (`crates/sofarr-server/src/reencode/ffmpeg.rs`) | built |        |
| AC-02 | `reencode::audio::tests::only_lossless_tracks_without_object_audio_are_converted` (`crates/sofarr-server/src/reencode/audio.rs`); `reencode::run::tests::audio::lossless_tracks_without_atmos_become_eac3` (`crates/sofarr-server/src/reencode/run/tests.rs`) | built |        |
| AC-03 | `reencode::audio::tests::the_switch_and_the_rate_apply` (`crates/sofarr-server/src/reencode/audio.rs`); `reencode::run::tests::audio::switched_off_every_track_is_copied` (`crates/sofarr-server/src/reencode/run/tests.rs`); `config::tests::reencode_audio_settings_have_defaults_and_are_checked` (`crates/sofarr-server/src/config.rs`) | built |        |
| AC-04 | `reencode::audio::tests::the_switch_and_the_rate_apply` (`crates/sofarr-server/src/reencode/audio.rs`); `reencode::ffmpeg::tests::converted_audio_and_a_copied_video_are_asked_for` (`crates/sofarr-server/src/reencode/ffmpeg.rs`); `config::tests::reencode_audio_settings_have_defaults_and_are_checked` (`crates/sofarr-server/src/config.rs`) | built |        |
| AC-05 | `reencode::run::tests::audio::a_track_not_converted_as_planned_fails_the_result`, `lossless_tracks_without_atmos_become_eac3` (`crates/sofarr-server/src/reencode/run/tests.rs`) | built |        |
| AC-06 | `reencode::audio::tests::an_hevc_video_is_copied_only_when_audio_is_converted` (`crates/sofarr-server/src/reencode/audio.rs`); `reencode::run::tests::audio::an_hevc_video_is_copied_when_audio_is_converted` (`crates/sofarr-server/src/reencode/run/tests.rs`); `reencode::ffmpeg::tests::converted_audio_and_a_copied_video_are_asked_for` (`crates/sofarr-server/src/reencode/ffmpeg.rs`) | built |        |
| AC-07 | `reencode::audio::tests::an_hevc_video_is_copied_only_when_audio_is_converted` (`crates/sofarr-server/src/reencode/audio.rs`); `reencode::run::tests::audio::an_hevc_video_is_copied_when_audio_is_converted` (`crates/sofarr-server/src/reencode/run/tests.rs`) | built |        |
| AC-08 | manual: Daniel Rück checked a film with TrueHD 7.1 without Atmos and TrueHD Atmos in the container; the first played as E-AC-3 5.1, the second still as Atmos in Jellyfin, 2026-11-18 | built |        |

## Decisions

- 2026-11-17: Lossless tracks without object audio become E-AC-3 640k (owner).
- 2026-11-17: DTS-HD MA stays untouched, because the reader cannot rule out DTS:X and the owner wants to keep it.
- 2026-11-17: An HEVC video is copied when audio is converted, so an efficient video does not lose quality a second time.
- 2026-11-17: Spec `reencode-file` is revised alongside: its AC-03 and its out-of-scope line on audio make room for this spec.
- 2026-11-18: The plan per audio track is pure (`reencode::audio::plan`, `copies_video`) and was red against stubs first; the encoder asks per converted track for `-c:a:<n> eac3 -b:a:<n> <rate>k` (and `-ac:a:<n> 6` above 6 channels) and leaves out every libx265 option when it copies the video. The result check compares every audio track with its plan: copied tracks keep codec and channels, converted ones are E-AC-3 with at most 6 channels and the same language.
- 2026-11-18: A track with an unknown channel count gets the configured rate and no mix-down (the spec's edge case); stereo and mono get 256 kbit/s.
- 2026-11-18: Checked by hand with ffmpeg 6.1 of the development container through `amigo-sofarr reencode`: an H.264 file with PCM 7.1 (de) and FLAC 5.1 (en) became HEVC with two E-AC-3 5.1 tracks at 640 kbit/s with their languages, 76 % smaller; an HEVC file with FLAC 5.1 got its video copied and ended `no_gain` (its FLAC of a test tone was already small). This is not AC-08, which needs the owner's library and Jellyfin.
