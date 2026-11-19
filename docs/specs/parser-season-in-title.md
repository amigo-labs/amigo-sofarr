---
status: implemented
created: 2026-11-19
size: S
design: [sofarr.md §2 (v2 episode parsing), sofarr.md §5, sofarr.md §6]
media: series
oracle: GET /api/v3/parse?title=… (Sonarr, fixtures/public/sonarr-parse.jsonl)
---

# Parser: a season inside an anime title

## Context

Two anime title forms from the episode oracle (`fixtures/public/sonarr-parse.jsonl`) were left
out of [parser-episode-gaps](parser-episode-gaps.md) and stand in the backlog of
`docs/specs/README.md`:

1. **Season word, then ` - <number>`** —
   `[Erai-raws].Kikansha.no.Mahou.wa.Tokubetsu.desu.2nd.Season.-.06.[1080p.CR.WEBRip.HEVC.AAC][MultiSub][BFA1EA3B]`
   (4 oracle titles). Today sofarr reads a season pack of season 6 with the title
   "Kikansha no Mahou wa Tokubetsu desu 2nd". Sonarr reads the series title
   "Kikansha no Mahou wa Tokubetsu desu 2nd Season" and the absolute episode 6. Reading a single
   episode as a season pack is harmful: the decision engine treats it as a whole season.
2. **Bracketed title that names a season** —
   `[Nekomoe.kissaten][Sasaki.to.Pii-chan.S2][05][1080p][JPTC]` (2 oracle titles). Today sofarr
   recognises nothing. Sonarr reads the series title "Sasaki to Pii-chan", season 2 and a full
   season (no episode), although the release holds only episode 5.

Terms are those of [parser-episode-gaps](parser-episode-gaps.md) (episode oracle, movie oracle,
leading group, bracketed title, separators, season marker, marker). In addition:

- **Season word** — `Season`, `Staffel`, `Saison`, `Temporada`, `Stagione` (any case).
- **Spaced dash** — a dash with a separator on at least one side (`.-.`, ` - `), unlike the
  tight dash of `Season-2` or `E01-03`.

The parser is pure and clean-room (`AGENTS.md`): the rules come from the oracle answers above
and general knowledge of anime release names.

## Scope

### In scope

- The two forms above, for release titles and file names.
- A deliberate deviation from Sonarr for form 2 (see Decisions), recorded in `docs/parity.md`.
- Updating the episode-oracle ratchet baseline to the new rates.

### Out of scope

- Matching a series title such as "… 2nd Season" to a series of the library (aliases, TVDB
  season mapping) — because that is the series matcher, not the parser.
- Form 2 with more than one season (`[Grp][Title.S1-S2][05]`) — because no oracle title has it.
- An episode marker inside a bracketed title (`[Grp][Title.S2E05][1080p]`, recognised as nothing
  today) — because no oracle title has it.
- Season words in titles without a following ` - <number>` (`Title.2nd.Season.1080p`) —
  because no oracle title has them and the form is ambiguous.

## Acceptance criteria

- [x] AC-01 (event): When a title has a season word followed by a spaced dash and a number that
  qualifies as an absolute episode (two to four digits, not a year, followed by a bracket, a dash
  or nothing — the rule of ` - 06` in parser-episode-gaps), then sofarr reads the season word as
  part of the series title, the number as the absolute episode, and no season and no season
  pack (`[Erai-raws].Kikansha.no.Mahou.wa.Tokubetsu.desu.2nd.Season.-.06.[1080p…]` →
  "Kikansha no Mahou wa Tokubetsu desu 2nd Season", absolute 6).
- [x] AC-02 (event): When a leading group is followed by a bracketed title whose last word is a
  season marker `S<n>` without episodes, and then by an episode number alone in square brackets
  (`[Nekomoe.kissaten][Sasaki.to.Pii-chan.S2][05][1080p][JPTC]`), then sofarr reads the series
  title without the season marker ("Sasaki to Pii-chan"), season n, episode number from the
  bracket (5), and no season pack.
- [x] AC-03 (always): sofarr reads season packs and episodes that use a season word or `S<n>`
  the same as before: `Show.Name.Season.2.1080p.WEB-GRP` (season 2 pack),
  `Show.Season.1-3.1080p` (seasons 1 to 3), `Kleine Insel Season 1 Episode 4 720p` (S01E04),
  `[Grp].Sky.Pirates.Season.2.-.07.[720p.WEB-DL]` (absolute 7, title "Sky Pirates Season 2"),
  `[Nekomoe.kissaten][Chii.Fuyo][06][1080p][JPSC]` (absolute 6).
- [x] AC-04 (error): If a bracketed title with a season marker is followed by a bracket that is
  no episode number (`[Grp][Title.S2][1080p][WEB]`, `[Grp][Title.S2][2026]`), then sofarr
  recognises no episode of form 2; if the number after the spaced dash is a year or a
  resolution (`[Grp].Title.Season.-.2026.[1080p]`, `[Grp].Title.Season.-.1080p`), then it reads
  no absolute episode of form 1.
- [x] AC-05 (always): In the episode oracle, the four titles of form 1 match Sonarr on every
  compared field, and the two titles of form 2 match Sonarr on series title and season; no
  field's pass rate falls below the ratchet baseline, which is rewritten to the new rates; the
  movie oracle stays below its limit of movie titles read as episodes.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `[Grp] Show 2nd Season - 06 (1080p)` (spaces instead of dots) | Title "Show 2nd Season", absolute 6 |
| `[Grp].Show.Season.-.06v2.[1080p]` | Absolute 6 (version suffix ignored as in ` - 06v2`) |
| `[Grp].Show.Season.-.06-07.[1080p]` | Absolute 6 and 7 (continued like ` - 06-07`) |
| `Show.Season-2.1080p.WEB-GRP` (tight dash) | Season 2 pack, unchanged — a tight dash is no spaced dash |
| `Show.Season.2.-.07.[720p]` without a leading group | Season 2 pack as before (the season word is followed by a number, not by a dash) |
| `[Grp][Title.S2][05v2][1080p]` | Season 2, episode 5 |

## Open questions

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `episode::tests::season_word_before_spaced_dash_number` (`crates/sofarr-parser/src/episode.rs`) | verified | 5eec838 |
| AC-02 | `episode::tests::bracketed_title_with_season` (`crates/sofarr-parser/src/episode.rs`) | verified | 5eec838 |
| AC-03 | `episode::tests::season_forms_stay_unchanged`, `episode::tests::anime_season_in_title_is_no_pack`, `episode::tests::season_packs` (`crates/sofarr-parser/src/episode.rs`) | verified | 5eec838 |
| AC-04 | `episode::tests::season_in_title_rejects_non_episodes` (`crates/sofarr-parser/src/episode.rs`) | verified | 5eec838 |
| AC-05 | `season_in_title_matches_oracle`, `sonarr_oracle_ratchet`, `movies_are_rarely_episodes` (`crates/sofarr-parser/tests/episode_oracle.rs`); baseline `fixtures/parser-episode-baseline.toml` | verified | 06d0f38 |

## Summary

> Completed: 2026-11-19

- A season word followed by a spaced dash and an absolute number (`…2nd.Season.-.06`) stays
  in the series title; the number is the absolute episode, no season pack.
- A bracketed title ending in `S<n>` followed by a bracketed episode number
  (`[Grp][Title.S2][05]`) gives season n, episode 5 without the marker in the title — a
  deliberate deviation from Sonarr's full season, recorded in `docs/parity.md`.
- Existing season packs, `Season 1 Episode 4` and anime cours behind a leading group read as
  before; years and resolutions after the dash or in the bracket give no episode.
- Episode-oracle baseline raised (series_title 993 ‰, full_season 999 ‰); no movie title
  reads as an episode.

## Decisions

- 2026-11-19 (spec): Form 2 deviates from Sonarr deliberately: sofarr reads season 2, episode 5
  instead of a full season 2, because the release holds one episode; taken as a season pack, the
  decision engine would grab a single episode in place of a whole season and an import would miss
  every other episode. In the episode oracle the two titles then differ from Sonarr on the
  fields episodes and full season. Recorded in `docs/parity.md`, "Bewusste Abweichungen".
