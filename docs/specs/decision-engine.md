---
status: todo
created: 2026-11-19
size: L
design: [sofarr.md §2 (v1 decision engine, v2 series), sofarr.md §3, sofarr.md §4 (Quality, QualityProfile, CustomFormat, Decision), sofarr.md §5]
media: both
oracle: GET /api/v3/release?movieId=… (Radarr, fixtures/public/radarr-release.jsonl with the config export in fixtures/public/config/radarr)
---

# Decision engine

## Context

For every release an indexer offers, sofarr decides whether to grab it: which named quality it
is, which custom formats it matches and what they score, and — if not — every reason it is
rejected. When several releases are approved, sofarr picks the best one. Automatic grabs also
wait until a movie counts as available.

The engine was built in phase 2 (movies) and phase 5 (series), before the spec workflow
existed. This spec records what it does today (reverse-engineered, `spec-extract`). It lives in
the pure crate `sofarr-core` (no IO, time passed in, sofarr.md §3); the server feeds it with
releases from RSS and searches and with the profiles, custom formats, quality definitions and
settings imported from Radarr/Sonarr. Its rules come from the release oracle (Radarr's
`GET /api/v3/release` answers, `fixtures/public/radarr-release.jsonl`) and the Servarr user
documentation, never from Servarr code (sofarr.md §6). Derived rules and deviations are listed
in `docs/parity.md` ("Stand Release-Orakel", "Annahmen ab Phase 3").

Terms used below:

- **Named quality** — a quality step such as `WEBDL-1080p` or `Remux-2160p` with an id, a name
  and its parts (source, resolution, modifier as the parser reports them, spec `parser`).
- **Quality catalog** — the named qualities known to sofarr: those of the imported quality
  definitions, or, without definitions, the qualities the oracle returned for the public parse
  fixtures. `Unknown` (id 0) is always part of it.
- **Profile** — a quality profile: an ordered list of items from lowest to highest preference,
  each a quality or a group of qualities, each allowed or not; a cutoff item; whether upgrades
  are allowed; custom format scores; minimum score, cutoff score, minimum upgrade score; a
  language.
- **Rank** — the position of a quality's item in the profile; qualities of one group share a
  rank.
- **Revision** — version and `REAL` count of a release (spec `parser`, AC-08).
- **The file** — for a movie, its current file (quality, revision, custom format score, release
  group); for a series release, the weakest file among the covered wanted episodes.
- **Rejection** — a reason with a stable key (`quality_not_wanted`, `too_small`, …) and a
  German text; all are permanent except `too_young`.

## Scope

### In scope

- Named quality, custom formats and score of a release.
- Rejections for movies: identification (year), quality, burnt-in subtitles, language, size,
  minimum score, upgrades against the file, indexer limits.
- Ordering approved releases.
- Availability of a movie for automatic grabs.
- Series: which episodes a release covers and whether one of them is wanted, with the movie
  rules for everything else.
- The release-oracle ratchet.

### Out of scope

- Parsing the release name — because spec `parser` owns it.
- Matching an RSS release to a movie or series by title, and the shadow/live pipeline around
  the decision — because they are server behavior (`docs/parity.md`, "RSS: Film-Zuordnung").
- Reading profiles, custom formats, definitions and settings from the Radarr/Sonarr config
  export — because that is the server's config import; the release oracle exercises it but
  this spec does not specify its fields.
- The blocklist and failed downloads — because spec `fail-handling` owns them.
- Rejecting a release named like an imported one — because spec `release-oracle-history` owns it.
- Rejection kinds the owner's configuration does not use (e.g. Radarr's raw-disc rejection,
  custom format conditions other than language, release group and release title) — because the
  parity target is the owner's setup (sofarr.md §2).

## Acceptance criteria

- [ ] AC-01 (always): sofarr names the quality of a release by the named quality of the catalog with exactly the parsed source, resolution and modifier; without an exact match, by the only quality of the same source and modifier if there is just one (a Telesync at any resolution is `TELESYNC`); otherwise `Unknown` — `Some.Movie.2020.1080p.WEB-DL.DDP5.1.H.264-GRP` is `WEBDL-1080p`, a 720p CAM is `Unknown`, and a WEB-DL at 480p is `Unknown` when several WEB-DL qualities exist.
- [ ] AC-02 (always): sofarr rejects a release whose quality is not listed in the profile or whose profile item is not allowed (`quality_not_wanted`), and ranks qualities by their item's position, the qualities of a group sharing one rank.
- [ ] AC-03 (always): sofarr matches a custom format when, for every kind of condition it has (language, release group, release title), all its required conditions of that kind match and at least one of its other conditions of that kind matches (if it has any), with `negate` inverting a single condition; a format without conditions never matches; release-title and release-group patterns are regular expressions including flags and lookbehind (`(?<=\bGerman\b.*)\bDL\b`); the score is the sum of the profile's scores of the matched formats, and a score below the profile's minimum is rejected (`below_minimum_score`).
- [ ] AC-04 (error): If a movie release has no year and does not contain "german" (any case), then sofarr rejects it only as `unparsable`; if it names a year that is neither the movie's year nor its secondary year (when the movie has years), then only as `unknown_movie` — in both cases with the quality named but no custom formats and score 0; a release without a year but marked German, and one with a matching year, are evaluated whatever their title says.
- [ ] AC-05 (event): When the runtime is known and the quality has a definition, then sofarr rejects a release smaller than the definition's minimum megabytes per minute times the runtime (`too_small`) or larger than its maximum (`too_large`, none without a maximum); with an unknown runtime (0) it checks no size.
- [ ] AC-06 (option): Where the profile names a language, sofarr rejects a release not in it (`language_not_wanted`) — `Any` takes every release, `Unknown` one without a recognised language, `Original` one marked as original or in the movie's original language; where the indexer reports languages for the release, they are used in place of the parsed ones, for the profile and for language conditions of custom formats alike.
- [ ] AC-07 (option): Where burnt-in subtitles are not allowed, sofarr rejects a release with burnt-in subtitles (`hardcoded_subs`, naming the tag if there is one); where they are allowed, it does not.
- [ ] AC-08 (option): Where an indexer maximum size, retention or minimum age is set (non-zero), sofarr rejects a larger release (`maximum_size_exceeded`), an older one (`outside_retention`) and a younger one (`too_young`, the only temporary rejection); each check is skipped when its setting is 0.
- [ ] AC-09 (state): While the movie has a file and the profile allows upgrades, sofarr compares the release with the file by rank, then revision, then score: a lower rank is `lower_quality`; the same rank with a lower revision is `lower_revision`, with a higher revision an upgrade, and otherwise an upgrade only when the score exceeds the file's by at least the profile's minimum upgrade score (at least 1, else `no_better_score`); a higher rank is an upgrade unless the file already reaches the cutoff quality (`quality_cutoff_met`, `lower_revision` if the release's revision is lower) — except, under "prefer and upgrade", a release of higher rank and higher revision; when the file reaches both the cutoff quality and the cutoff score, every release is `cutoff_met` except a higher revision of the file's rank (its quality or one in the same group).
- [ ] AC-10 (state): While the movie has a file and the profile does not allow upgrades, sofarr treats the lowest allowed quality as the cutoff, so a release is `cutoff_met` when the file's score reaches the cutoff score, a release of higher rank is additionally `upgrades_not_allowed`, and a higher revision of the file's rank (its quality or one in the same group) still passes.
- [ ] AC-11 (option): Where propers and repacks are set to "do not prefer", sofarr ignores revisions in upgrades and in ordering (the score decides); where set to "do not upgrade", a higher revision alone is no upgrade but a lower one is still rejected; where a repack would replace the file through its higher revision, it must come from the file's release group (case-insensitive, `repack_other_group`), and is refused when the file's group is unknown (`file_group_unknown`); under "do not prefer" revisions count for nothing, so this check does not apply there.
- [ ] AC-12 (always): sofarr orders approved releases by rank, then score, then revision (version, then `REAL` count; skipped under "do not prefer"), then size, the bigger release first.
- [ ] AC-13 (event): When sofarr checks whether a movie is available for an automatic grab, then "announced" is always available; "released" needs the earlier of digital and physical release plus the availability delay (which may be negative) to be today or earlier, or, without these dates, the status "released"; "in cinemas" needs "released" or the cinema date plus the delay, or, without a cinema date, at least the status "in cinemas"; TMDB's status `Released` counts as in cinemas; a movie with neither status nor dates is available.
- [ ] AC-14 (event): When a series release is evaluated, then sofarr finds the covered episodes by air date, by absolute number, by season (all episodes of a season or season range for a pack) or by season and episode numbers; the file to beat is none if a covered monitored and aired episode has no file, otherwise the weakest such file (lowest rank, then score); sizes count the episode runtime times the covered episodes; no year is checked; a release covering no episode is rejected first as `unknown_episode`, one covering none that is monitored and aired first as `episode_not_wanted`, followed by the movie rules' rejections.
- [ ] AC-15 (always): Against the release oracle (public release fixtures with the public config export), no field's pass rate (approved, custom formats, score, rejections, quality) falls below `fixtures/decision-baseline.toml` while the fixture set is unchanged; `SOFARR_UPDATE_BASELINE=1` raises the baseline (only upwards for the same set, to the current rates after the set changed), and without a public config or release fixtures the check is skipped.

## Technical notes

- Entry points (`sofarr_core`): `decide(config, profile, release, movie) -> Decision` for movies
  (year rule, then `evaluate`), `evaluate(…)` for the shared rules, `compare(profile, propers,
  (a, size), (b, size)) -> Ordering` for the best release, `series::evaluate_episodes(…)` and
  `series::covered(…)` for series, `availability::is_available(minimum, info, delay_days,
  today)`.
- `Decision { parsed, quality, custom_formats, score, rejections }`; `approved()` is "no
  rejections". `Rejection::key()` is the stable identifier used by the oracle diff and in logs;
  `Display` gives the German text shown to the user.
- Rejection order in a decision: already imported, quality, burnt-in subtitles, language, size,
  minimum score, upgrade rules, indexer limits. `already_imported` (a release named like an
  imported one) is owned by spec `release-oracle-history`.
- Pass rates today (‰, `fixtures/decision-baseline.toml`, 1,703 releases from 20 movies):
  approved 1000, custom_formats 998, quality 998, rejections 993, score 998. The remaining
  mismatches are listed by `cargo test -p sofarr-parity --test release_oracle -- --nocapture`
  (e.g. Radarr's raw-disc rejection, which sofarr does not have).

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| `Some.Movie.1080p.BluRay.x264-GOOD` (no year, not German) | Only `unparsable`, quality `Bluray-1080p`, score 0 |
| `Some.Movie.2.2022…` for a movie of 2020 | Only `unknown_movie`, no custom formats |
| Movie without known years | The year check is skipped |
| Release 1 byte, runtime unknown | No size rejection |
| Quality in a group with the file's quality (WEB-DL vs WEBRip 1080p) | Same rank: revision, then score decide |
| File at cutoff quality, score below cutoff score, release of higher quality | `quality_cutoff_met`; a better-scoring release of the same quality is still an upgrade |
| `REPACK` of the file's quality by another group | `repack_other_group` |
| Retention or maximum size set to 0 | No limit |
| Season pack where one episode is at Bluray and one at WEB-DL | The WEB-DL file is the one to beat |
| `S00E01` of an unmonitored special | `episode_not_wanted` first |
| Movie with digital date in 9 days, delay −10 | Available as "released" |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| This spec describes the behavior of 2026-11-19 as found in code and tests, not a target | Reverse-engineered spec (`spec-extract`) for phases 2 and 5 | A rule that deviates from the oracle is recorded as is; the ratchet and `docs/parity.md` drive improvements |
| Ordering of approved releases, availability and the series rules have no oracle | They are documented as assumptions in `docs/parity.md` ("Annahmen ab Phase 3"); the release oracle covers only single-movie decisions | Radarr/Sonarr may pick another release among several approved ones |
| The release ratchet compares only public fixtures with the public config export | Private fixtures and the owner's full config are not in CI (`AGENTS.md`) | A regression visible only in private fixtures is reported but does not fail CI |
| No ratchet rate is named as an AC threshold | The baseline file is the threshold and may only rise | none |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `quality::tests::resolves_parsed_release`, `quality::tests::unmatched_parts_are_unknown`, `quality::tests::unique_source_ignores_resolution`, `quality::tests::new_catalog_keeps_unknown`, `quality::tests::observed_parts_are_unique`, `quality::tests::api_vocabulary` (`crates/sofarr-core/src/quality.rs`); `decision::tests::wanted_release_without_file` (`crates/sofarr-core/src/decision.rs`) | built | |
| AC-02 | `profile::tests::ranks_and_groups` (`crates/sofarr-core/src/profile.rs`); `decision::tests::quality_score_and_size` (`crates/sofarr-core/src/decision.rs`) | built | |
| AC-03 | `custom_format::tests::same_kind_is_any_of`, `custom_format::tests::required_and_negate`, `custom_format::tests::different_kinds_must_all_match`, `custom_format::tests::empty_format_never_matches`, `custom_format::tests::lookaround_patterns_compile` (`crates/sofarr-core/src/custom_format.rs`); `pattern::tests::variable_lookbehind_with_word_boundaries`, `pattern::tests::alternatives_flags_and_negative_lookbehind`, `pattern::tests::plain_patterns_stay_untouched` (`crates/sofarr-core/src/pattern.rs`); `profile::tests::scores_sum_matched_formats` (`crates/sofarr-core/src/profile.rs`); `decision::tests::wanted_release_without_file`, `decision::tests::quality_score_and_size` (`crates/sofarr-core/src/decision.rs`) | built | |
| AC-04 | `decision::tests::titles_without_year_need_german`, `decision::tests::other_year_skips_all_rules` (`crates/sofarr-core/src/decision.rs`) | built | |
| AC-05 | `decision::tests::quality_score_and_size`, `decision::tests::unknown_runtime_skips_size_limits` (`crates/sofarr-core/src/decision.rs`); `definition::tests::limits_scale_with_runtime` (`crates/sofarr-core/src/definition.rs`) | built | 56dcd9f |
| AC-06 | `decision::tests::reported_languages_replace_parsed_ones`, `decision::tests::language_and_indexer_settings`, `decision::tests::reported_languages_feed_language_formats` (`crates/sofarr-core/src/decision.rs`); `language::tests::matching`, `language::tests::resolves_ids_and_names` (`crates/sofarr-core/src/language.rs`) | built | 0cd1688 |
| AC-07 | `decision::tests::hardcoded_subs_follow_the_setting`, `decision::tests::tagged_hardcoded_subs_name_the_tag` (`crates/sofarr-core/src/decision.rs`) | built | 0cd1688 |
| AC-08 | `decision::tests::language_and_indexer_settings`, `decision::tests::retention_rejects_old_releases_permanently` (`crates/sofarr-core/src/decision.rs`) | built | 56dcd9f |
| AC-09 | `decision::tests::upgrades_rank_then_revision_then_score`, `decision::tests::file_at_cutoff_quality`, `decision::tests::minimum_upgrade_score` (`crates/sofarr-core/src/decision.rs`) | built | 0cd1688 |
| AC-10 | `decision::tests::profile_without_upgrades` (`crates/sofarr-core/src/decision.rs`) | built | |
| AC-11 | `decision::tests::propers_policy`, `decision::tests::repacks_need_the_group_of_the_file` (`crates/sofarr-core/src/decision.rs`) | built | |
| AC-12 | `decision::tests::best_release_order` (`crates/sofarr-core/src/decision.rs`) | built | 56dcd9f |
| AC-13 | `availability::tests::stages`, `availability::tests::released_needs_home_release`, `availability::tests::status_without_dates`, `availability::tests::announced_minimum_takes_unreleased_movies` (`crates/sofarr-core/src/availability.rs`) | built | 0cd1688 |
| AC-14 | `series::tests::missing_episode_is_wanted`, `series::tests::upgrades_and_season_packs`, `series::tests::unwanted_and_unknown`, `series::tests::daily_and_absolute`, `series::tests::sizes_scale_with_covered_episodes` (`crates/sofarr-core/src/series.rs`) | built | 0cd1688 |
| AC-15 | `release_oracle`, `harness_tests::same_set_only_ratchets_up`, `harness_tests::changed_set_may_go_down` (`crates/sofarr-parity/tests/release_oracle.rs`); baseline `fixtures/decision-baseline.toml` | built | |

## Decisions

- 2026-11-19 (extract): Written from the code of phases 2 and 5 with `spec-extract`. Three
  tests were added because no test covered them: an unknown runtime skipping the size limits
  (AC-05), retention (AC-08) and the order of approved releases (AC-12). All three passed on the
  unchanged engine.
- 2026-11-19 (check): AC-09, AC-10 and AC-11 revised to the code: a higher revision counts within the file's rank (a quality group shares one rank), and the repack group check belongs to an upgrade by revision, so it does not apply under "do not prefer" (no oracle for that setting, `parity.md`); the acceptance check found the wording broader.
