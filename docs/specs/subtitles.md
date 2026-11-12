---
status: todo
created: 2026-11-12
size: L
design: [sofarr.md §6, sofarr.md §8, sofarr.md §9 D12, sofarr.md §9 D17, sofarr.md §9 D25]
media: both
oracle: none
---

# Subtitles

## Context

Many releases in the owner's library are German with an original audio track ("German DL"), but
not all carry subtitles, and English-only releases have no German ones. Jellyfin shows external
subtitles that sit next to the video under its naming rules (`Film.de.srt`, `Film.de.forced.srt`,
`Film.en.sdh.srt`), but its OpenSubtitles plugin is unreliable (limits, no control). D17 decides
that sofarr fetches subtitles itself through the official OpenSubtitles.com REST API. Bazarr is not
used, and its code is off limits (AGENTS.md, Clean Room). Spec `mediainfo` tells which audio and
subtitle languages a file already contains.

Terms used below:

- **Wanted languages** – `[subtitles] languages`, default `["de", "en"]` (ISO 639-1).
- **Covered language** – a language the video file already has as an audio track, as an embedded
  non-forced subtitle track, or as an external non-forced subtitle file next to it. Languages are
  compared after mapping the file's ISO 639-2 codes (spec `mediainfo`) to ISO 639-1; `und` and
  codes without an ISO 639-1 equivalent cover nothing. An external file next to the video
  counts by Jellyfin's naming rules: `<video name>.<language>[.<flags>].<extension>`, where the
  language is an ISO 639-1 or 639-2 code or an English or German language name and the flags
  include `forced`/`foreign` and `sdh`/`cc`/`hi`; a file without a language part, and files in
  subfolders, cover nothing.
- **Needed subtitle** – for a monitored movie or episode with a file: a normal subtitle for each
  wanted language that is not covered; and a forced subtitle in the language of the first audio
  track, when that language is a wanted language and the file has no forced subtitle in it
  (embedded or external) — the audio track itself does not cover a forced subtitle. A file whose
  first audio language is unknown needs no forced subtitle.
- **Candidate** – a subtitle the provider offers for the file and language.
- **Match quality** – from best to worst: the provider's file hash matches the video; then the
  number of release attributes that match the video's release (group, source, resolution,
  edition; more is better); then only the movie or episode matches. A candidate whose release
  name, parsed by sofarr, names another season or episode than the video is discarded. A hearing-impaired (SDH)
  candidate is taken only when no normal candidate exists at all.
- **Own subtitle** – an external subtitle file that sofarr wrote; every other subtitle file is
  someone else's.
- **Quota** – the provider's download limit per day for the account.

## Scope

### In scope

- Finding needed subtitles for movies and episodes, after an import and on a schedule
- Searching, ranking and downloading from OpenSubtitles.com within the quota
- Storing them next to the video by Jellyfin's naming rules, in UTF-8
- Keeping own subtitles with their video when the video is renamed, moved, upgraded or removed
- API and UI: subtitles per file, a list of needed subtitles, a manual search per file

### Out of scope

- Providers other than OpenSubtitles.com — because one reliable provider covers German and English well (D17)
- Upgrading a subtitle to a better match later — because a found subtitle of the right movie or episode serves its purpose; the owner can delete it to search again
- Timing correction (sync) or translation of subtitles — because they need audio analysis or a language model
- Subtitles for unmonitored items — because the owner only cares about monitored ones in day-to-day use; a manual search covers others
- Extracting embedded subtitles into files — because Jellyfin reads embedded tracks itself

## Acceptance criteria

- [ ] AC-01 (event): When sofarr imports a video file of a monitored movie or episode, then it determines the file's needed subtitles and searches for them.
- [ ] AC-02 (event): When the subtitle job runs (every `interval_hours`), then sofarr searches the needed subtitles of the library that are due (AC-05), newest imported files first, as far as the quota allows.
- [ ] AC-03 (always): sofarr never searches a normal subtitle for a covered language, and a file never has more than one own subtitle per language and kind (normal or forced) at a time.
- [ ] AC-04 (event): When the provider offers candidates for a needed subtitle, then sofarr downloads the candidate of the best match quality as SRT, stores it next to the video as `<video name>.<language>[.forced][.sdh].srt` in UTF-8, and records it as an own subtitle.
- [ ] AC-05 (state): While candidates exist but none was downloaded (quota used up, shadow mode), the needed subtitle stays due for the next run. While no candidate of a needed subtitle exists, it is due again one day after its last search during the first 7 days after the import, then one week after its last search; a forced subtitle the provider does not have keeps being searched on that schedule but is not listed as needed in AC-11.
- [ ] AC-06 (always): sofarr never overwrites or deletes a subtitle file that is not its own; if the target name is taken by another subtitle, that language counts as covered.
- [ ] AC-07 (error): If the quota is used up, then sofarr stops downloading until the provider's reset time and continues there, newest files first; searches that need no download continue.
- [ ] AC-08 (error): If the provider cannot be reached, answers a server error, or refuses the login or the API key (a failing download of a single candidate is AC-14; a used-up quota is AC-07), then sofarr stops the run — subtitles already stored in it stay — logs the reason (without credentials), shows it on the system page (spec `web-ui-shell`), and tries again at the next run.
- [ ] AC-09 (event): When a video is renamed or its folder moved (spec `web-ui-library`), then its own subtitles get the matching new names and place.
- [ ] AC-10 (event): When a video is replaced by an upgrade or removed, then its own subtitles go with it (to the recycle bin or deleted like the video), and the new video gets its needed subtitles determined afresh.
- [ ] AC-11 (event): When the user opens the subtitle overview, then it lists every needed subtitle with item, language, kind, last search and next search, and the subtitles found in the last 7 days; per file the user can start a search at once.
- [ ] AC-12 (event): When the user opens a movie or episode file in the UI, then it shows the file's subtitles — embedded, external, own — with language and kind.
- [ ] AC-13 (state): While sofarr runs in shadow mode, it searches but downloads and writes nothing, and the overview shows what it would have downloaded.
- [ ] AC-14 (error): If downloading a single candidate fails (e.g. it was removed), or the downloaded subtitle is empty, not text, or cannot be converted to UTF-8, then sofarr discards it, keeps no file of it, does not record it as an own subtitle, and tries the next candidate.
- [ ] AC-15 (option): Where `[subtitles] enabled = false` is set or `[opensubtitles]` is missing, sofarr searches and downloads no subtitles; the overview says why.

## Technical notes

- Config: `[subtitles] enabled` (default true when `[opensubtitles]` is set), `languages`
  (ISO 639-1), `interval_hours` (default 12); `[opensubtitles] api_key`, `username`, `password`
  (secrets, masked on the system page).
- OpenSubtitles.com REST API (public documentation): `POST /login` → token, `GET /subtitles`
  with `moviehash`, `imdb_id`/`tmdb_id` or `parent_imdb_id` + `season_number` + `episode_number`,
  `languages`, `foreign_parts_only`, `hearing_impaired`; `POST /download` → a link, `remaining`
  and `reset_time` (AC-07). The required `Api-Key` and a `User-Agent` naming sofarr go with
  every request.
- Movie hash: OpenSubtitles' published algorithm (file size plus checksums of the first and last
  64 KiB), tested against the example values in its documentation where it gives some, otherwise
  against synthetic files whose hash follows from the documented algorithm by hand.
- Release-name match uses `sofarr-parser` on the candidate's release name and on the video's
  scene name or file name. A video that cannot be read for the hash is searched without it.
- Episodes: `parent_imdb_id`, else `parent_tmdb_id` (the series' TMDB id, D26), else the series
  title with `season_number` and `episode_number`.
- Table `subtitles` (file, language, kind, path, provider id, match quality, created) and
  per needed subtitle the last and next search time.
- A new IO crate `sofarr-subtitles` for the provider client, used by `sofarr-server` (D25).
- Language codes in file names are ISO 639-1 (`de`, `en`), as in Jellyfin's examples.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| A German DL release with German and English audio | de and en are covered; only a German forced subtitle may be needed |
| An English release with English subtitles embedded | en covered; a German subtitle is needed |
| A file with an external `Film.de.srt` from elsewhere | de covered; the file is never touched (AC-06) |
| A season pack of 10 episodes imported at once with 5 downloads left today | The newest 5 needed subtitles now, the rest after the reset (AC-07) |
| Only an SDH subtitle exists for German | It is downloaded as `.de.sdh.srt` |
| The provider returns a subtitle for the wrong episode (mislabelled) | Ranked by match quality; a hash match wins; a wrong episode without any match is not offered |
| The video's media info is unknown (unsupported container) | Only external files count as covered; audio and embedded tracks are unknown, so subtitles may be fetched although the file has them |
| The owner deletes an own subtitle | It is needed again and searched at the next run |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| The language of the first audio track is the language for forced subtitles | Releases put the main language first; Jellyfin plays it by default | A file whose first track is the original language gets forced subtitles in that language |
| Retry daily for 7 days, then weekly | New releases get subtitles within days; older titles rarely gain new ones | A subtitle published weeks later arrives up to a week late |
| A free OpenSubtitles account is enough at first | The owner imports a few files a day | A large initial library backlog takes weeks within a small quota |

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

## Decisions

- 2026-11-12: Wanted languages de and en, fetched only when neither an audio nor a subtitle track nor an external file covers them (owner).
- 2026-11-12: A forced subtitle in addition when the provider has one; SDH only when nothing else fits (owner).
