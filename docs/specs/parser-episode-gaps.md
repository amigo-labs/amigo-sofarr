---
status: todo
created: 2026-11-15
size: M
design: [sofarr.md §2 (v2 episode parsing), sofarr.md §5, sofarr.md §6]
media: series
oracle: GET /api/v3/parse?title=… (Sonarr, fixtures/public/sonarr-parse.jsonl)
---

# Parser: episode gaps from the episode oracle

## Context

The episode parser reads the series title and the episode numbers from a release title. Against
the Sonarr parse oracle (6,754 titles in `fixtures/public/sonarr-parse.jsonl`) four forms of
title fail completely or partly. They are listed in the backlog of `docs/specs/README.md`
(three rows) plus one closely related anime form found while writing this spec:

1. **Year in parentheses before an absolute number** — `[SubsPlease].Magic.Knight.Rayearth.(2026).-.06.(1080p).[96B7C6E9]`.
   Today nothing is recognised; Sonarr reads the series title "Magic Knight Rayearth (2026)" and
   the absolute episode 6 (9 oracle titles).
2. **Episode number without a season** — `Jyuou.Mujin.Dandivine.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST`.
   Today nothing is recognised; Sonarr reads season 1, episode 6 (4 oracle titles).
3. **Anime batch in parentheses** — `[SubsPlease].Yamato.yo.Towa.ni.-.Rebel.3199.(23-26).(1080p).[Batch]`.
   Today the title swallows `23` and only 26 is found; Sonarr reads the title
   "Yamato yo Towa ni - Rebel 3199" and the absolute episodes 23, 24, 25, 26 (3 oracle titles).
4. **Absolute number in square brackets after a dotted title** —
   `[Studio.GreenTea].Kusuriya.no.Hitorigoto.[52][WebRip][HEVC-10bit.1080p.AAC.ASSx2]`.
   Today nothing is recognised; Sonarr reads the title "Kusuriya no Hitorigoto" and the absolute
   episode 52 (9 oracle titles). The form with a bracketed title (`[Grp][Chii.Fuyo][06][1080p]`)
   already works.

The parser is pure (no IO) and clean-room (`AGENTS.md`): the rules here are derived only from
the oracle answers above and general knowledge of release naming, never from Servarr code.
Movie titles must stay movies: the movie oracle (`fixtures/public/radarr-parse.jsonl`) already
limits how many movie titles look like episodes (below 0.2 %), and that limit stays.

Terms used below:

- **Episode oracle** — the comparison of `parse_episode` with Sonarr's `parsedEpisodeInfo` over
  `fixtures/public/sonarr-parse.jsonl`, per field: series title (compared as lowercase letters
  and digits only, so `Magic Knight Rayearth (2026)` equals `magicknightrayearth2026`), season,
  episode numbers, absolute numbers, air date, full season.
- **Movie oracle** — the titles in `fixtures/public/radarr-parse.jsonl`.
- **Leading group** — the first token of a title when it is in square brackets (`[SubsPlease]`).
  In `[Grp][Chii.Fuyo][06]` the group is `[Grp]`; `[Chii.Fuyo]` is a bracketed title.
- **Separators** — dot, space and underscore separate words alike (`.-.` equals ` - `).
- **Year** — a four-digit number from 1900 to 2099.
- **Season marker** — `S<n>` with the digits joined to the `S` (with or without episodes;
  `S 1` with a space is none), `<n>x<n>`, or a season word followed by a number (`Season 2`,
  `Staffel 2`).
- **Marker** — what ends the series title: a season marker, a season-less episode marker, an
  air date, or an absolute number (` - 06` and the forms of this spec). Years, resolutions and
  other words are no markers. The leftmost marker after the title wins; in `X.E5.Y.S01E02` that
  is `E5`.
- **Season-less episode marker** — a word `E<n>` with one to three digits (any case), alone or
  continued as `E06E07` or `E06-E08`, with no season marker before it.

## Scope

### In scope

- The four title forms above, for series release titles and file names (the same parser reads
  both).
- Keeping the movie oracle limit and updating the episode-oracle ratchet baseline to the new
  rates.

### Out of scope

- `[Erai-raws].Kikansha.no.Mahou.wa.Tokubetsu.desu.2nd.Season.-.06…` being read as a season
  pack (4 oracle titles) — because it is a different rule (season word inside an anime title)
  and stays in the backlog to keep this spec at size M.
- `[Nekomoe.kissaten][Sasaki.to.Pii-chan.S2][05]…` — because Sonarr answers "full season 2"
  together with an episode number, which needs its own look before it is copied.
- Oracle titles where Sonarr reads a year or a resolution as season and episode
  (`WWE.Crown.Jewel.2026.1080p…` → S20E26, `Fawlty Towers (1975) Complete 1496x1148` → S1496E1148)
  — because following them would turn ordinary movie and event titles into episodes.
- Multi-episode releases of the form `Seinfeld.3x17.3x18…` and repeated titles
  (`Show.S01.DVDRip-G.show.s01e01…`) — because they are separate gaps with their own rules.
- Forms 1, 3 and 4 without a leading group — because every oracle title of these forms has one,
  and without it movie titles such as `The.Godfather.Trilogy.(1-3).1080p` or
  `Title (2019) - 01 …` would turn into episodes.

## Acceptance criteria

- [ ] AC-01 (event): When a title behind a leading group has a year in parentheses directly before ` - <number>`, then sofarr reads the series title including the year and the number as an absolute episode without a season, with the same number rules as the existing ` - 06` anime form (two to four digits, not a year, an optional version suffix `v2`) — `[SubsPlease].Magic.Knight.Rayearth.(2026).-.06.(1080p).[96B7C6E9]` gives the title "Magic Knight Rayearth (2026)", absolute episode 6, no season, no season pack.
- [ ] AC-02 (event): When the first marker after the title is a season-less episode marker, then sofarr reads season 1 with its episodes (a dash between two markers is a range) and the title before the marker — `Jyuou.Mujin.Dandivine.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST` gives "Jyuou Mujin Dandivine", season 1, episode 6; `Magic.Knight.Rayearth.2026.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST` gives "Magic Knight Rayearth 2026", season 1, episode 6; `Show.E06-E08.1080p.WEB-G` gives season 1, episodes 6, 7, 8.
- [ ] AC-03 (event): When a title behind a leading group is directly followed by a rising number range in parentheses whose ends are not years and whose end minus start is below 50, then sofarr reads every number of the range as an absolute episode without a season and keeps the range out of the title — `[SubsPlease].Yamato.yo.Towa.ni.-.Rebel.3199.(23-26).(1080p).[Batch]` gives "Yamato yo Towa ni - Rebel 3199" and absolute episodes 23, 24, 25, 26, not a season pack.
- [ ] AC-04 (event): When a dotted or spaced title behind a leading group is directly followed by a number of two or three digits in square brackets other than 480, 576 and 720 (optionally with a version suffix `v2`), then sofarr reads that number as an absolute episode without a season — `[Studio.GreenTea].Kusuriya.no.Hitorigoto.[52][WebRip][HEVC-10bit.1080p.AAC.ASSx2]` gives "Kusuriya no Hitorigoto", absolute episode 52.
- [ ] AC-05 (always): sofarr reads no absolute episode from `The Dam Busters (1955) - 1080p HDR - H265 BRRip - NLsub`, `[Grp].Some.Title.[1080p][WebRip]`, `[Grp].Some.Title.[2026][WebRip]`, `[Grp].Some.Title.[720][WebRip]`, `[Grp].Some.Title.[96B7C6E9]`, `[Grp].Some.Title.(26-23).[Batch]`, `[Grp].Some.Title.(01-120).[Batch]`, `The.Godfather.Trilogy.(1-3).1080p.BluRay.x264-G` and `Unforgettable (2011-2015) S 1-4`, and no episode at all from `Show.2026.-.06.1080p`, `E06.1080p.WEB-G` and `[Grp].[52][WebRip]` (no title before the marker).
- [ ] AC-06 (always): Of the movie-oracle titles, fewer than 0.2 % are read as an episode with an air date, with an absolute number, or with a season although the title contains no season marker.
- [ ] AC-07 (always): In the episode oracle, each of the 25 oracle titles listed under Technical notes matches Sonarr on all six compared fields, no field's pass rate falls below `fixtures/parser-episode-baseline.toml`, and the baseline file records the new rates.

## Technical notes

The 25 oracle titles of the four forms (AC-07), exactly as in `fixtures/public/sonarr-parse.jsonl`:

| Form | Title |
| ---- | ----- |
| 1 | `[AsukaRaws].Magic.Knight.Rayearth.(2026).-.06.(WEB-DL.1280x720.x264.AAC)` |
| 1 | `[SubsPlease].Magic.Knight.Rayearth.(2026).-.06.(1080p).[96B7C6E9]` |
| 1 | `[SubsPlease].Magic.Knight.Rayearth.(2026).-.06.(720p).[2DA08BB3]` |
| 1 | `[SubsPlease].Magic.Knight.Rayearth.(2026).-.06.(480p).[ED116078]` |
| 1 | `[Erai-raws].Magic.Knight.Rayearth.(2026).-.06.[480p.CR.WEB-DL.AVC.AAC][MultiSub][0C8F4306]` |
| 1 | `[Erai-raws].Magic.Knight.Rayearth.(2026).-.06.[720p.CR.WEB-DL.AVC.AAC][MultiSub][B923BEA0]` |
| 1 | `[Erai-raws].Magic.Knight.Rayearth.(2026).-.06.[1080p.CR.WEB-DL.AVC.AAC][MultiSub][3329235B]` |
| 1 | `[Erai-raws].Magic.Knight.Rayearth.(2026).-.06.[1080p.CR.WEBRip.HEVC.AAC][MultiSub][0AE67E21]` |
| 1 | `[ASW].Magic.Knight.Rayearth.(2026).-.06.[1080p.HEVC][688DE02C]` |
| 2 | `Jyuou.Mujin.Dandivine.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST` |
| 2 | `Magic.Knight.Rayearth.2026.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST` |
| 2 | `Sekai.Saikyou.no.Majo.Hajimemashita.E06.SUBFRENCH.1080p.WEB.H264-SLEEPINGFOREST` |
| 2 | `The.Human.Spark.E02.So.Human.So.Chimp.HDTV.XviD-FQM` |
| 3 | `[SubsPlease].Yamato.yo.Towa.ni.-.Rebel.3199.(23-26).(1080p).[Batch]` |
| 3 | `[SubsPlease].Yamato.yo.Towa.ni.-.Rebel.3199.(23-26).(720p).[Batch]` |
| 3 | `[SubsPlease].Yamato.yo.Towa.ni.-.Rebel.3199.(23-26).(480p).[Batch]` |
| 4 | `[KitaujiSub].FX.Senshi.Kurumi-chan.[03][WebRip][HEVC AAC][CHS JP&CHT JP]` |
| 4 | `[Studio.GreenTea].FX.Senshi.Kurumi-chan.[06][WebRip][HEVC-10bit.1080p.AAC][JPTC]` |
| 4 | `[Studio.GreenTea].Toaru.Anbu.no.Item.[03][WebRip][HEVC-10bit.1080p.AAC.ASSx2]` |
| 4 | `[Studio.GreenTea].Kusuriya.no.Hitorigoto.[52][WebRip][HEVC-10bit.1080p.AAC.ASSx2]` |
| 4 | `[Studio.GreenTea].Mahou.Shoujo.Ikusei.Keikaku.Restart.[06][WebRip][HEVC-10bit.1080p.AAC][JPSC]` |
| 4 | `[Studio.GreenTea].Mahou.Shoujo.Ikusei.Keikaku.Restart.[06][WebRip][HEVC-10bit.1080p.AAC][JPTC]` |
| 4 | `[Sakurato].Koori.no.Jouheki.[20][HEVC-10bit.1080P.AAC][CHS&CHT]` |
| 4 | `[Sakurato].Koori.no.Jouheki.[20][AVC-8bit.1080P.AAC][CHS]` |
| 4 | `[Sakurato].Koori.no.Jouheki.[20][AVC-8bit.1080P.AAC][CHT]` |

Baseline today (‰): `series_title` 988, `season` 996, `episodes` 996, `absolute` 992,
`air_date` 999, `full_season` 998. The movie-oracle test today counts air dates and absolute
numbers; AC-06 adds the season read without a season marker.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `The Dam Busters (1955) - 1080p HDR - H265 BRRip - NLsub` (movie oracle) | No absolute episode: `1080p` is a resolution, not a number |
| `Unforgettable (2011-2015) S 1-4` (movie oracle) | `(2011-2015)` is a year range, not an absolute batch |
| `The.Godfather.Trilogy.(1-3).1080p.BluRay.x264-G` | No leading group: no batch |
| `[Grp].Some.Title.[1080p][WebRip]`, `[Grp].Some.Title.[2026][WebRip]`, `[Grp].Some.Title.[720][WebRip]` | No absolute episode: resolution, year, bare vertical resolution (480, 576, 720) |
| `[Grp].Some.Title.[96B7C6E9]` (checksum) | No absolute episode |
| `[Grp][Chii.Fuyo][06][1080p][JPSC]` (already passing) | Unchanged: "Chii Fuyo", absolute 6 |
| `[Grp].Some.Title.[06v2][WebRip]` | Absolute 6, like ` - 06v2` |
| `Show.E06-E08.1080p.WEB-G` | Season 1, episodes 6, 7, 8 |
| `Show.S02E06.1080p` | Unchanged: the season marker wins, season 2 |
| Falling, empty or long range `(26-23)`, `(23-23)`, `(01-120)` | Not a batch, no absolute episode |
| `Show.2026.-.06.1080p` (bare year) | No episode, as today |
| `E06.1080p.WEB-G`, `[Grp].[52][WebRip]`, `[Grp].(2026).-.06` | No title before the marker: no episode |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| A season-less `E06` means season 1 | All 4 oracle titles of this form give season 1 | A series with a single season numbered otherwise gets the wrong season; the import matches by TVDB order anyway |
| Season-less markers continue like `S01E06E07` (`E06E07`, dash = range) | The oracle has only single episodes; using the existing multi-episode rules keeps one behavior for both | A rare title reads more episodes than Sonarr would |
| Only a year in parentheses may sit between title and ` - <number>` (form 1) | The oracle has no title with a bare year there, and `Title.2026.-.07…` must not turn a movie into an episode | Anime releases with a bare year stay unrecognised |
| A batch range must rise and its end minus start must be below 50 | A falling or one-number range is no plausible span; 50 is the limit the parser already uses for episode ranges | Long batches (`(01-120)`) give no episode |
| Bracket numbers (form 4) have two or three digits and are not 480, 576 or 720 | All 9 oracle titles have two digits; three digits cover long series; one digit is too weak a marker; 480/576/720 are resolutions written without `p` | `[3]` and four-digit episodes (`[1000]`) stay unrecognised |
| Forms 1, 3 and 4 need a leading group | All oracle titles of these forms have one; it is what separates anime releases from movie titles | The same forms without a group stay unrecognised |
| The movie limit stays at 0.2 % | It is the existing limit; measured on 2026-11-15, no movie-oracle title contains the new forms (`E<n>`, `[nn]` behind a title) and 0 of 4,952 are read with a season but without a season marker | A future fixture refresh brings such titles |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `year_in_parentheses_before_absolute_number` (`crates/sofarr-parser/src/episode.rs`) | built |        |
| AC-02 | `season_less_episode_marker` (`crates/sofarr-parser/src/episode.rs`) | built |        |
| AC-03 | `anime_batch_in_parentheses` (`crates/sofarr-parser/src/episode.rs`) | built |        |
| AC-04 | `bracket_number_after_title` (`crates/sofarr-parser/src/episode.rs`) | built |        |
| AC-05 | `new_forms_reject_non_episodes` (`crates/sofarr-parser/src/episode.rs`) | built |        |
| AC-06 | `movies_are_rarely_episodes`, `season_marker_detection` (`crates/sofarr-parser/tests/episode_oracle.rs`) | built |        |
| AC-07 | `episode_gap_titles_match_oracle`, `sonarr_oracle_ratchet` (`crates/sofarr-parser/tests/episode_oracle.rs`); baseline `fixtures/parser-episode-baseline.toml` | built |        |

## Decisions

- 2026-11-15 (build): The edge case `[Grp][Chii.Fuyo][06][1080p][JPSC]` was marked "already
  passing", but the parser returned nothing for it. It is now built along with AC-04 (group,
  title and number each in their own square brackets), because the edge case table asks for
  "Chii Fuyo", absolute 6. A bracketed title that names a season (`[Grp][Title.S2][05]`) stays
  unrecognised, because that form is out of scope.
- 2026-11-15 (build): Tokens now record whether a dash was among their separators and which
  bracket encloses them, because `(2026).-.06` (the bracket wins the separator kind) and the
  `[03]`/`(23-26)` distinction cannot be seen otherwise.
- 2026-11-15 (build): An absolute number after ` - ` must no longer sit inside brackets, because
  otherwise the end of a range such as `(01-120)` became an absolute episode (AC-05).
- 2026-11-15 (build): `[Onalrie].Yamato.yo.Towa.ni.Rebel.3199.-.E23…` (4 titles) and
  `[Onalrie].Shin.Tennis…U-17…-.E07…` now give season 1 with the `E` number, as AC-02 says.
  Sonarr reads an absolute number out of the title there (3199, 17), which sofarr does not
  copy. They are recorded in `docs/parity.md` as deliberate deviations.
- 2026-11-15 (build): The AC-06 count was already 0 of 4,952 before the change and stays 0,
  because the movie oracle contains none of the new forms. The test was therefore green from
  the start; `season_marker_detection` shows that the new part of the count tells titles with
  and without a season marker apart.
- 2026-11-15 (build): `episode_gap_titles_match_oracle` (AC-07) was written after the parser
  change. Its red run comes from the parser of the previous commit
  (`git checkout HEAD~1 -- crates/sofarr-parser/src`), where 25 titles fail on series title,
  season, episodes or absolute numbers. Rates after the change (‰): `series_title` 988 → 992,
  `absolute` 992 → 996; the other fields are unchanged.
