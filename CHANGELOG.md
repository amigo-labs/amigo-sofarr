# Changelog

## [1.3.0](https://github.com/amigo-labs/amigo-sofarr/compare/v1.2.0...v1.3.0) (2026-11-19)


### Features

* **core:** reject a release already grabbed and imported under its name (AC-01, AC-02) ([6bef5bb](https://github.com/amigo-labs/amigo-sofarr/commit/6bef5bb5157a74d9fdd5a682c03a84167379f4c7))
* **download:** a release marked bad (FAILURE/BAD) is a release failure ([028d336](https://github.com/amigo-labs/amigo-sofarr/commit/028d3366ea2a93f3383fa5ffca08355a3b30abfc))
* **parity:** record imported release names and compare already_imported (AC-03, AC-04, AC-05, AC-06) ([f6984d2](https://github.com/amigo-labs/amigo-sofarr/commit/f6984d287a312f58f7f2faaf5e1c15af1e5f3e97))
* **parser:** season word before ' - 06' and season in a bracketed title (AC-01, AC-02, AC-03, AC-04) ([5eec838](https://github.com/amigo-labs/amigo-sofarr/commit/5eec8382b66c5353841c87acdc196d11db6c50a0))


### Bug Fixes

* **db:** a movie update without German title keeps the stored one ([861678c](https://github.com/amigo-labs/amigo-sofarr/commit/861678ccb603703e56ad9321c72ded35a159f3fb))
* **metadata:** keep a person's credits once per movie and role (AC-11) ([4f2b784](https://github.com/amigo-labs/amigo-sofarr/commit/4f2b7844887109c7e651406d51a43f00113ca7ca))
* **server:** a scan judges each root folder once, before deleting there (AC-10) ([1e6bea8](https://github.com/amigo-labs/amigo-sofarr/commit/1e6bea8797144a8903e5cd87993f61e9e346499e))
* **server:** an empty export folder names Sonarr for import-sonarr ([6f40a79](https://github.com/amigo-labs/amigo-sofarr/commit/6f40a797b48e41107d9c63164cbfbd351f9a5b81))
* **server:** an import list selects each movie once, through cast or crew (AC-11) ([fd59917](https://github.com/amigo-labs/amigo-sofarr/commit/fd5991735ca858d49e923d7ccc2e89c739cd93f0))

## [1.2.0](https://github.com/amigo-labs/amigo-sofarr/compare/v1.1.0...v1.2.0) (2026-11-18)


### Features

* **core:** rule for library files too large for their resolution (AC-03, AC-04, AC-05) ([0f7d0c7](https://github.com/amigo-labs/amigo-sofarr/commit/0f7d0c7cdb561f6aa5970f876d2bc46669b81329))
* **db:** all re-encode attempts of library files, newest first (AC-02, AC-10) ([a8aa276](https://github.com/amigo-labs/amigo-sofarr/commit/a8aa2769f95358a09ecfc0098759b1fc3759674b))
* **db:** re-encode attempts and the update of a replaced video (AC-06, AC-15) ([5201085](https://github.com/amigo-labs/amigo-sofarr/commit/520108564d35d1ed87c12517784f5a9adacd188b))
* **db:** re-encode exclusions and the latest attempts (AC-05, AC-06) ([2bcbfae](https://github.com/amigo-labs/amigo-sofarr/commit/2bcbfaefd2db3562ac281a7f35b5bc5601ba3a71))
* **db:** read the stored media info of every file in one query (AC-06) ([495f950](https://github.com/amigo-labs/amigo-sofarr/commit/495f950751396d2ffeff4c215e3d9e83f3fa2a1f))
* **db:** space saved by re-encoding (AC-02) ([a93361c](https://github.com/amigo-labs/amigo-sofarr/commit/a93361c7157244fa286cd3640893d7611a9652b3))
* re-encode oversized library files (rule, one file, job, page, audio) ([1c1e3ed](https://github.com/amigo-labs/amigo-sofarr/commit/1c1e3ed34be2c0d4b21b7d452069685b7fcebbaa))
* **server:** [reencode] convert_lossless_audio and audio_bitrate_kbps (AC-03, AC-04) ([424862d](https://github.com/amigo-labs/amigo-sofarr/commit/424862d7f447b67615587846616b1bf421d1ac8f))
* **server:** [reencode] encoder settings crf, preset, threads, gain and time limit (AC-16) ([240249c](https://github.com/amigo-labs/amigo-sofarr/commit/240249c8ce6b41d7b4276afbb17d7ea6cd2cd4c3))
* **server:** API of the re-encode page and exclusions from the job (AC-02, AC-03, AC-05, AC-06, AC-08, AC-10) ([5b0e5af](https://github.com/amigo-labs/amigo-sofarr/commit/5b0e5afffd57ea37f24938ac92cfe7eb64efefe4))
* **server:** background re-encode job, largest due file first (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10) ([872720d](https://github.com/amigo-labs/amigo-sofarr/commit/872720dea04269a8c61a476acb22627724f0f3af))
* **server:** German shelf names, „Sperrliste“, file actions note without config syntax (AC-07, AC-10) ([54b79f4](https://github.com/amigo-labs/amigo-sofarr/commit/54b79f4410d1306866e63ed30b0f78ab21f5b947))
* **server:** list library files too large for their resolution by API and CLI (AC-02, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12) ([b9a9be5](https://github.com/amigo-labs/amigo-sofarr/commit/b9a9be56edcc6f507c35f117e82fee02933537a4))
* **server:** lossless audio without Atmos to E-AC-3, HEVC video copied (AC-01, AC-02, AC-05, AC-06, AC-07) ([75ec357](https://github.com/amigo-labs/amigo-sofarr/commit/75ec3570b739e707ca80ee804325788513f11d76))
* **server:** media info carries the duration from Matroska and MP4 headers (AC-01) ([39f7000](https://github.com/amigo-labs/amigo-sofarr/commit/39f70004b1aae51b876aee6b4445365b3d30b9d8))
* **server:** queue answer says whether a download client is configured (AC-04) ([bf5b0dd](https://github.com/amigo-labs/amigo-sofarr/commit/bf5b0dd36e9d554b530ad9bcb0aa07e3205a2b7a))
* **server:** re-encode one library file by CLI and API (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10, AC-11, AC-12, AC-13, AC-14, AC-15) ([6241251](https://github.com/amigo-labs/amigo-sofarr/commit/6241251a6bf4d36aec9092ceacd4bfdb7d6bd613))
* **ui:** „Sperrliste“, „Heute beliebt“, German series types, date-times to the minute (AC-07, AC-08, AC-09, AC-10) ([eb1474c](https://github.com/amigo-labs/amigo-sofarr/commit/eb1474cc51306fe307946bb39dbb3276c892a1ee))
* **ui:** accent only for states; neutral info notices; one line of episode toggles (AC-01, AC-02, AC-11) ([e399146](https://github.com/amigo-labs/amigo-sofarr/commit/e3991462bce9473e7aa2b6c3535dceaefd3f5929))
* **ui:** discover tiles and search like the library (AC-03, AC-04, AC-05, AC-06) ([7840a2a](https://github.com/amigo-labs/amigo-sofarr/commit/7840a2a0d17a7b17b1ae8c95d841816811c27247))
* **ui:** overview figures with icons, a muted line without client, „Heute“ and „Morgen“ (AC-01, AC-02, AC-03, AC-05, AC-06, AC-07) ([f558a6f](https://github.com/amigo-labs/amigo-sofarr/commit/f558a6f560bb382409a0aa1f655d68f3aab41fc4))
* **ui:** page "Neu encodieren" with job, candidates, history and exclusions (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09) ([6cc44e9](https://github.com/amigo-labs/amigo-sofarr/commit/6cc44e940cd2c82f2bce3fb9216691e7cbbecec8))
* **ui:** paths break at separators; sofarr's monogram in the sidebar and tab (AC-12, AC-13, AC-14) ([2b11fc5](https://github.com/amigo-labs/amigo-sofarr/commit/2b11fc580d800492c91d79431a8d00f947345205))
* **ui:** system page says whether files are re-encoded (AC-08) ([36ab14b](https://github.com/amigo-labs/amigo-sofarr/commit/36ab14b511edbf37376ca2dfae206262aa7e282e))
* **ui:** Übersicht and Entdecken first in the sidebar without a group heading (ui-shell-redesign AC-01, web-ui-shell AC-08) ([0de78c9](https://github.com/amigo-labs/amigo-sofarr/commit/0de78c93e334ad0f577d8c251258bebea5e78933))


### Bug Fixes

* **server:** job states follow the spec's order; episode exclusions tested (AC-03, AC-05) ([9426111](https://github.com/amigo-labs/amigo-sofarr/commit/94261110a0412c6c5917f28c921c0f9b709f6d8c))
* **server:** re-encode job shows its next look and stops through the real shutdown path (AC-03, AC-06, AC-08) ([41991c6](https://github.com/amigo-labs/amigo-sofarr/commit/41991c6883b0145bfe64a9882a511531eab52904))
* **ui:** overview figure labels no longer take the status chip look (AC-01) ([3d2442e](https://github.com/amigo-labs/amigo-sofarr/commit/3d2442e8b99aab38a4e8a9523f381429f395d01c))
* **ui:** say "1 fehlt" instead of "1 fehlen" for one missing episode ([03cd9f6](https://github.com/amigo-labs/amigo-sofarr/commit/03cd9f6d8c7375fe3e90cc41c44a70d1b100bd19))

## [1.1.0](https://github.com/amigo-labs/amigo-sofarr/compare/v1.0.1...v1.1.0) (2026-11-17)


### Features

* **parser:** episode forms without season and anime numbers (AC-01, AC-02, AC-03, AC-04, AC-05) ([150b012](https://github.com/amigo-labs/amigo-sofarr/commit/150b012106ef38abfbc4967193e5f291721660e9))
* **parser:** episode gaps from the episode oracle (spec parser-episode-gaps) ([9716e1b](https://github.com/amigo-labs/amigo-sofarr/commit/9716e1b329833c2bcb28423b2e397b4cfd97f41e))
* **ui:** add the shared amigo theme files and check them in CI (AC-01) ([964c3e6](https://github.com/amigo-labs/amigo-sofarr/commit/964c3e65d37b150af26c78a70dc4fde96bd19ddd))
* **ui:** calendar month grid, week columns and agenda (AC-01 .. AC-07) ([550c976](https://github.com/amigo-labs/amigo-sofarr/commit/550c9768a7fea11ceac7bc35ebe0dbcc0bdcdbb8))
* **ui:** dark by default; the switch offers Dunkel, Hell and System (AC-04, AC-05, AC-06, AC-07, AC-08) ([54fef01](https://github.com/amigo-labs/amigo-sofarr/commit/54fef01e117a699b9ea09c76911ce0cd7fdeb4a8))
* **ui:** detail page head with poster backdrop, compact facts and cast strip (AC-01 .. AC-07) ([aa26a2c](https://github.com/amigo-labs/amigo-sofarr/commit/aa26a2cadc1994c44ffab8fd7c783f904fbc46cd))
* **ui:** discover filter panel with chips, shelf arrows, preview backdrop (AC-01 .. AC-06) ([d8b58e6](https://github.com/amigo-labs/amigo-sofarr/commit/d8b58e626585f9012e5132732f24041094b374e4))
* **ui:** draw the UI in the shared theme's tokens and in Inter (AC-02, AC-03, AC-09, AC-10, AC-11, AC-12) ([d0e8fd2](https://github.com/amigo-labs/amigo-sofarr/commit/d0e8fd25258cbdd5bcda0fd340a426de681ed68e))
* **ui:** list toolbar, filter panel and filter chips (AC-01 .. AC-12) ([30680b4](https://github.com/amigo-labs/amigo-sofarr/commit/30680b45be0523e9d70dd927078f27552dfb2ee2))
* **ui:** page header, overview key figures and columns, calm rows (AC-01 .. AC-09) ([d2736ef](https://github.com/amigo-labs/amigo-sofarr/commit/d2736ef836a492de4a514a7f441c6f0af83aada0))
* **ui:** quieter list cards and rows with status chips and compact rating (AC-01 .. AC-10) ([f0fc20d](https://github.com/amigo-labs/amigo-sofarr/commit/f0fc20da6cab6ac623d204b4ebccdc4bb0680285))
* **ui:** shared look for buttons, fields, checkboxes, labels and links (AC-10, AC-11, AC-12, AC-13, AC-14) ([b9abc06](https://github.com/amigo-labs/amigo-sofarr/commit/b9abc06aa80de2a13eecb434b3c9fafa687b26e0))
* **ui:** sidebar navigation, drawer and segmented theme switch (AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08, AC-09) ([c7ca8fb](https://github.com/amigo-labs/amigo-sofarr/commit/c7ca8fbcb2c5dfd5d1c413d2e99f5c7a29d2e3e9))
* **ui:** the shared amigo theme, dark by default ([bbb09bb](https://github.com/amigo-labs/amigo-sofarr/commit/bbb09bbfeec7751fdc7e1d65ebd6023807e54d29))
* **ui:** two-column detail boxes, quiet headers, compact seasons (AC-01 .. AC-08) ([a7bca22](https://github.com/amigo-labs/amigo-sofarr/commit/a7bca22435f67813d50267c6ab5809c0511db412))


### Bug Fixes

* **ui:** at most two filter columns on narrow windows (ui-list-toolbar AC-10) ([4dbc607](https://github.com/amigo-labs/amigo-sofarr/commit/4dbc607f35c6eb9d453af2aa549e6ea5b971a5f3))
* **ui:** dim outside calendar days by colour so they keep AA contrast (ui-calendar AC-01, AC-07) ([bde35d5](https://github.com/amigo-labs/amigo-sofarr/commit/bde35d553534d75d7d2f98d58d676d577d1eff47))
* **ui:** keep the page header above a failed first load on System and Untertitel (ui-overview-pages AC-01) ([126a11b](https://github.com/amigo-labs/amigo-sofarr/commit/126a11bc3b120972a7d268861ed469c2465086c6))
* **ui:** keep the theme switch's labels on one line in the sidebar (AC-09) ([edc3b6b](https://github.com/amigo-labs/amigo-sofarr/commit/edc3b6b88ca3a52c5c9ee6e3b5bf5db802a0af0e))
* **ui:** queue rows keep their action at the end of the row (ui-overview-pages AC-02, AC-06) ([b7b74cd](https://github.com/amigo-labs/amigo-sofarr/commit/b7b74cdf389feca5bed69ccd9dd8ceb75cd60972))

## [1.0.1](https://github.com/amigo-labs/amigo-sofarr/compare/v1.0.0...v1.0.1) (2026-11-13)


### Bug Fixes

* **ui:** keep the view switch labels on one line ([a29ad55](https://github.com/amigo-labs/amigo-sofarr/commit/a29ad556aa2dc62c2d76e44f75bec22bcdb1b677))

## 1.0.0 (2026-11-13)


### Features

* close the open parity gaps (availability, disk scan, tags, manual-import oracle) ([56f5885](https://github.com/amigo-labs/amigo-sofarr/commit/56f5885c4d62156e3be8f7678ff93a1e1e36e87c))
* **core:** media info naming parts from the file with name fallback (AC-06) ([970f167](https://github.com/amigo-labs/amigo-sofarr/commit/970f1673eef4462a6f27191e85d69829772c0d7e))
* **core:** normalize release titles for the blocklist ([42e1c8f](https://github.com/amigo-labs/amigo-sofarr/commit/42e1c8f39823ce183abbaeb6a0032b6c2b6a3039))
* **db:** blocklist and download absence tracking (AC-20) ([9e7bc65](https://github.com/amigo-labs/amigo-sofarr/commit/9e7bc65fd7dcb68daf821732ef39cb9bd5b56d52))
* **db:** consistent copies, inspection and pending migrations of database files (AC-02, AC-09) ([92fd11f](https://github.com/amigo-labs/amigo-sofarr/commit/92fd11f02d7c41a30925588e3911bc26b22b319d))
* **db:** IMDb ratings swapped in as a whole, series TMDB ids (AC-01, AC-03, AC-12) ([2d6bc23](https://github.com/amigo-labs/amigo-sofarr/commit/2d6bc2383f39f2b6bfeb5ae3b945c5526ed8852c))
* **db:** library keys for discover and the new-season switch per series (AC-01, AC-08) ([82818aa](https://github.com/amigo-labs/amigo-sofarr/commit/82818aa71bc411f08bea673ec08a751fb504a30b))
* **db:** library version counter that moves with every library change (AC-04) ([1c6addb](https://github.com/amigo-labs/amigo-sofarr/commit/1c6addb56285017127f9972e2192c7fbbe97ae8b))
* **db:** login sessions by token hash (AC-07) ([e4e0a49](https://github.com/amigo-labs/amigo-sofarr/commit/e4e0a49f78564f7cdd8b9ad892c072c7d339e358))
* **db:** own subtitles and needed subtitles with their schedule (AC-03, AC-05) ([004cf07](https://github.com/amigo-labs/amigo-sofarr/commit/004cf07409f61f371d835ab2ce6578ab8b65325c))
* **db:** poster path of one movie or series (AC-20) ([9040a2a](https://github.com/amigo-labs/amigo-sofarr/commit/9040a2a47314c134642a02fbbae31f124ab8988c))
* **db:** posters, overview, bulk monitoring, item settings, removal and activity for the library pages (AC-01, AC-05, AC-10, AC-11, AC-12, AC-20) ([7dd9d8e](https://github.com/amigo-labs/amigo-sofarr/commit/7dd9d8e043db69c800436684f016765d06191d53))
* **db:** ratings of every library title in one query (AC-04, AC-11) ([cfd1a23](https://github.com/amigo-labs/amigo-sofarr/commit/cfd1a2316bb225228355d25295234a0d2e512def))
* **db:** remove sessions unused since a cutoff (AC-06) ([a66238b](https://github.com/amigo-labs/amigo-sofarr/commit/a66238b8366eec660867c472e235156dec990c61))
* **db:** store media info per movie and episode file (AC-01) ([fd8ebac](https://github.com/amigo-labs/amigo-sofarr/commit/fd8ebac3a6e5e632507c1577c78f80ce8a425111))
* **db:** trailer state per movie and series (AC-01, AC-02, AC-05, AC-08) ([174389d](https://github.com/amigo-labs/amigo-sofarr/commit/174389da350c37b80021268d382d3a5533dba6be))
* **docker:** Alpine runtime with yt-dlp and ffmpeg for trailers (AC-14) ([f00ba74](https://github.com/amigo-labs/amigo-sofarr/commit/f00ba743fcf71fa4e15f9545531f4e7da41ab163))
* **docker:** bun stage builds the UI into the image (AC-18) ([6359e92](https://github.com/amigo-labs/amigo-sofarr/commit/6359e920b77db9427b485f0a1098b5395cb2f07d))
* **download:** classify failed downloads, reject invalid NZBs (AC-01, AC-02) ([21fa06e](https://github.com/amigo-labs/amigo-sofarr/commit/21fa06e5ae80fc6191655f3d9778807dc650fb7a))
* **download:** nzbget download rate for the remaining time (AC-01) ([61d07bb](https://github.com/amigo-labs/amigo-sofarr/commit/61d07bbe35f5d5584a04c893af71c53f94b13f77))
* **indexer:** TV search by caps, broken NZB vs outage (AC-03, AC-04, AC-14) ([130254f](https://github.com/amigo-labs/amigo-sofarr/commit/130254f6263bb13657c8d54694c7702a75862aa8))
* **mediainfo:** dynamic range in the oracle's words HDR10, HDR10Plus, DV HDR10Plus (AC-07) ([9073c6c](https://github.com/amigo-labs/amigo-sofarr/commit/9073c6cdd7080dca6b06e78d1d8bf3f37f5732ae))
* **mediainfo:** read media info from Matroska and MP4 headers (AC-03, AC-04, AC-05, AC-11) ([9f8cddf](https://github.com/amigo-labs/amigo-sofarr/commit/9f8cddf488dc3ba3def907b1b6899bb7ea044e92))
* **metadata:** poster path and German overview of movies and series (AC-05, AC-20) ([094aeb6](https://github.com/amigo-labs/amigo-sofarr/commit/094aeb6e1e6e0af2c2245bb052515630e377b7a6))
* **metadata:** TMDB lists, discover filters, search, title and person details in German (AC-01..AC-05, AC-16) ([62751ea](https://github.com/amigo-labs/amigo-sofarr/commit/62751ea3f8abddb055a2b7a5b9c177ed225928af))
* **metadata:** TMDB lookups of a series by TVDB id and its IMDb id (AC-05, AC-12) ([317bbba](https://github.com/amigo-labs/amigo-sofarr/commit/317bbbae10fe1f66766844fb6f72df14b4f2d1f3))
* **metadata:** TMDB videos of movies and series, ranked trailer candidates (AC-03) ([0bc23da](https://github.com/amigo-labs/amigo-sofarr/commit/0bc23da144bea8db298a449f40502d0ac2fd82dd))
* phase 6 operations and comfort (ten feature specs) ([56e6de3](https://github.com/amigo-labs/amigo-sofarr/commit/56e6de3f7e47f33badacd4119a9af49e8f4c19e5))
* **server:** blocklist, re-search, series and manual search (AC-01..AC-19, AC-21..AC-24) ([4e12aa0](https://github.com/amigo-labs/amigo-sofarr/commit/4e12aa0266280cdccb65c1b6c89563a63dda2d94))
* **server:** compare-mediainfo reads the library from another machine with --root (mediainfo AC-07) ([3dcf0d8](https://github.com/amigo-labs/amigo-sofarr/commit/3dcf0d8a01319c544220c02c6ad6b30905c67125))
* **server:** compare-rename for series against Sonarr's rename preview (AC-23) ([0922d0a](https://github.com/amigo-labs/amigo-sofarr/commit/0922d0a988d2959c0a149a455a2f3fed7c8b6de5))
* **server:** daily IMDb ratings load with hourly retry, series ids at refresh (AC-01, AC-02, AC-03, AC-05, AC-09, AC-10, AC-12) ([7f334d1](https://github.com/amigo-labs/amigo-sofarr/commit/7f334d11331474056baa42684b94ed39cd5c8cf2))
* **server:** daily trailer downloads with yt-dlp, manual download and trailer state API (AC-01..AC-13, AC-15) ([6699567](https://github.com/amigo-labs/amigo-sofarr/commit/6699567f1c841aed5683000efdb3e07446a1e7f5))
* **server:** discover shelves, filtered list, search, previews, people, cast and adding titles (AC-01..AC-17) ([3fcf641](https://github.com/amigo-labs/amigo-sofarr/commit/3fcf641aa91c1a68167620696fdc3c530bdab90d))
* **server:** IMDb rating in library lists and pages, sort and minimum filters (AC-04, AC-06, AC-07, AC-09, AC-11) ([761d397](https://github.com/amigo-labs/amigo-sofarr/commit/761d3970114eee279a946215eb2306def971af6e))
* **server:** library lists, item pages, monitoring, profile changes and the missing page (AC-02, AC-03, AC-04, AC-05, AC-06, AC-08, AC-09, AC-10, AC-11) ([b29b773](https://github.com/amigo-labs/amigo-sofarr/commit/b29b773f014eaaddb8d4dd81d258233f31776594))
* **server:** media info at import and scan, file endpoints, compare-mediainfo (AC-01, AC-02, AC-06, AC-08, AC-09, AC-10, AC-12) ([f223376](https://github.com/amigo-labs/amigo-sofarr/commit/f2233768da558d68c81b779e14ce3f9ce2fe5e8a))
* **server:** optional login, API key, trusted networks and proxies (AC-01..AC-16) ([3f07d9b](https://github.com/amigo-labs/amigo-sofarr/commit/3f07d9bc8ea1a4ea83e7d9d0127c583a37499c25))
* **server:** overview with queue, activity, problems and upcoming releases; calendar (AC-01, AC-07) ([ac6baa3](https://github.com/amigo-labs/amigo-sofarr/commit/ac6baa373b4b747cbb9d3a46b62f651d77a75b5c))
* **server:** own subtitles follow renames and leave with removed items (AC-09, AC-10) ([a4e0103](https://github.com/amigo-labs/amigo-sofarr/commit/a4e0103ab73f5854df1e11779a75df45536f579c))
* **server:** posters through sofarr with a cache next to the database (AC-20, AC-21) ([7ad1743](https://github.com/amigo-labs/amigo-sofarr/commit/7ad17434fe9e011b6d87716b314fc8bcb2b7e00c))
* **server:** rename, folder move and remove with files, never overwriting and moving back on failure (AC-12, AC-13, AC-14, AC-15, AC-16, AC-17, AC-24) ([7a98cd7](https://github.com/amigo-labs/amigo-sofarr/commit/7a98cd765fefe572f53d00a93d98f6a0049e5ec6))
* **server:** scheduled, manual and pre-migration backups with restore (AC-01..AC-22) ([ee9d25f](https://github.com/amigo-labs/amigo-sofarr/commit/ee9d25fb00828e034d7e9dd2ffc6550448739368))
* **server:** serve the embedded UI, system endpoint, generated UI types (AC-01, AC-02, AC-03, AC-12, AC-15, AC-17) ([efd4234](https://github.com/amigo-labs/amigo-sofarr/commit/efd4234db3a866aa8cbe9651f80b4ffc082daf80))
* **server:** subtitle needs, scheduled and import search within the quota, subtitle API (AC-01..AC-08, AC-10..AC-15) ([69f801e](https://github.com/amigo-labs/amigo-sofarr/commit/69f801e35f1c95c7b7c006a181d8310c3eb5b6cf))
* **server:** TMDB and TVDB API roots configurable for test setups (discover AC-01) ([078708c](https://github.com/amigo-labs/amigo-sofarr/commit/078708cd5e4fb7346148f386b2e1059a11e4575b))
* **server:** UI types of the library pages, blocklist and backups (AC-01 to AC-21) ([2cbfbdc](https://github.com/amigo-labs/amigo-sofarr/commit/2cbfbdc375558e04d6c86ff6302d59a11ddd0e49))
* **subtitles:** OpenSubtitles.com client, movie hash, ranking, UTF-8 and Jellyfin names (AC-04, AC-07, AC-08, AC-14) ([9447653](https://github.com/amigo-labs/amigo-sofarr/commit/944765358c546f5fc967eaef4ff8c8e9448ce9d4))
* **subtitles:** own subtitles follow a renamed or moved video (AC-09) ([c4a9c69](https://github.com/amigo-labs/amigo-sofarr/commit/c4a9c6918c35f436e6f77f7f38bf700b42358b2d))
* **ui:** an episode shows its file with quality, size, release group and media info (web-ui-library AC-06) ([b1201dc](https://github.com/amigo-labs/amigo-sofarr/commit/b1201dc90541f7dbd62674f7462261aa80dd8ed0))
* **ui:** discover, search, title previews, person pages and adding titles (discover AC-01..AC-17) ([4fecc0b](https://github.com/amigo-labs/amigo-sofarr/commit/4fecc0b1b504c85b67abd86aea06a4b1ebcbe95e))
* **ui:** IMDb rating texts and attribution with the date of the data (imdb-ratings AC-04, AC-08) ([f272a77](https://github.com/amigo-labs/amigo-sofarr/commit/f272a771dab3aa1b6c90b8a716a47e46f5e7a3b6))
* **ui:** IMDb ratings in the library lists and pages, sort and minimum filters (imdb-ratings AC-04, AC-06, AC-07, AC-08, AC-09) ([3c31034](https://github.com/amigo-labs/amigo-sofarr/commit/3c31034377e6ed563e965a9198ca1a34168e398a))
* **ui:** library pages – overview, movie and series lists and pages, calendar, missing, blocklist, backups (AC-01..AC-13, AC-15..AC-22, AC-24) ([0dc658c](https://github.com/amigo-labs/amigo-sofarr/commit/0dc658cddb4009af62306566385a0ddb0b17f87b))
* **ui:** subtitle overview page and the subtitles of movie and episode files (subtitles AC-11, AC-12) ([658585d](https://github.com/amigo-labs/amigo-sofarr/commit/658585d55d8e2d0afa62dc7e35f0e1c613c1b1ee))
* **ui:** SvelteKit shell with login, navigation, theme, system page and e2e tests (AC-01..AC-14, AC-16, AC-19) ([78db961](https://github.com/amigo-labs/amigo-sofarr/commit/78db961a39e52343819a8b56abb2575e142e1bf1))
* **ui:** trailer state and manual download on movie and series pages, trailers on the system page (trailers AC-09, AC-10, AC-12) ([8cc1be1](https://github.com/amigo-labs/amigo-sofarr/commit/8cc1be1f4a4eb62c240add1d1884cfeb12a91e9a))


### Bug Fixes

* **core:** CleanTitle as Radarr renames it, every folder level valid on Windows ([bf81a0a](https://github.com/amigo-labs/amigo-sofarr/commit/bf81a0aa65cd65329eaf353fcf5b9f0a3d33278f))
* **core:** CleanTitle keeps !, … # · ‘, drops lone hyphens and apostrophes like Radarr ([8d14161](https://github.com/amigo-labs/amigo-sofarr/commit/8d1416153406169f79d9aceb95d08bc6969d0157))
* **mediainfo:** keep complete tracks of a truncated header (AC-04) ([888982c](https://github.com/amigo-labs/amigo-sofarr/commit/888982cadb477d7a7912d9e15db2426319c7f180))
* **mediainfo:** RFC 9559 language default and channels from DTS core and AC-3 frames (AC-03, AC-07) ([d5c677c](https://github.com/amigo-labs/amigo-sofarr/commit/d5c677c62cb00a9d3fc3a70b3c6165e21008d5f7))
* **metadata:** a title without German texts falls back to its original language (discover AC-16) ([a6df894](https://github.com/amigo-labs/amigo-sofarr/commit/a6df894f173aaf4ba520706e5b54b71fdd5e5bf3))
* **server:** any invalid session_days gets the German config message (AC-12) ([cd0eb71](https://github.com/amigo-labs/amigo-sofarr/commit/cd0eb7123170b836e2f29aecfb6393ae3c268e14))
* **server:** backup scheduler, journals, serialisation and pruning errors proven (AC-01, AC-13, AC-16, AC-20, AC-22) ([0468d55](https://github.com/amigo-labs/amigo-sofarr/commit/0468d5509348b443e52020fbb26fcb6673b8e06c))
* **server:** bounded unbuffered reads, size and time from before the read, oracle named in errors (AC-01, AC-05, AC-06, AC-08, AC-09, AC-10) ([bf506a5](https://github.com/amigo-labs/amigo-sofarr/commit/bf506a51ce394af28e2a5aa950bb314465bdb1dc))
* **server:** clear the pending restore before applying it, keep the newest pre-migration backups (AC-04, AC-12, AC-13) ([1d1e80c](https://github.com/amigo-labs/amigo-sofarr/commit/1d1e80c07f2dc7fe9e5403efd406845d6a5cb9a1))
* **server:** compare-mediainfo --root replaces the item's root folder, keeping letter folders (mediainfo AC-07) ([6ed06d8](https://github.com/amigo-labs/amigo-sofarr/commit/6ed06d8a0eeadef04324ee0da42f1cdc5f24199b))
* **server:** compare-mediainfo maps oracle language codes and leaves undetermined languages out (mediainfo AC-08) ([2b7e11b](https://github.com/amigo-labs/amigo-sofarr/commit/2b7e11b6a4df8dae0db14304ac02dc8e6f14832c))
* **server:** count every release of a series search, exact download folders (AC-05, AC-12, AC-15) ([655d496](https://github.com/amigo-labs/amigo-sofarr/commit/655d496d5130a91ad06c642c45c8a78abb2a165a))
* **server:** empty placeholders are no backups, CLI output and restore marker (AC-01, AC-06, AC-16) ([6f62b17](https://github.com/amigo-labs/amigo-sofarr/commit/6f62b17f9ce29a132b713d973eba67fff6c615e5))
* **server:** generic 503 when marking without download client, bounded session_days (AC-12, AC-14) ([d5fa445](https://github.com/amigo-labs/amigo-sofarr/commit/d5fa4458ee937b5af916d2ab53fea7df5ee22eb4))
* **server:** German CLI errors for search targets, re-search tests per trigger (AC-06, AC-07, AC-13) ([2e0bbe7](https://github.com/amigo-labs/amigo-sofarr/commit/2e0bbe79fdc42074068897fdc20923b3180a455c))
* **server:** German message when no backup name is free ([5469db1](https://github.com/amigo-labs/amigo-sofarr/commit/5469db193ff2c30a90b62edeba7daf04124002be))
* **server:** keep renaming when own subtitles cannot follow (subtitles AC-09) ([547e811](https://github.com/amigo-labs/amigo-sofarr/commit/547e81167103851bf7294db263490985d0eb5909))
* **server:** list only the needs of monitored items in the subtitle overview (subtitles AC-11) ([af331a0](https://github.com/amigo-labs/amigo-sofarr/commit/af331a00fb6e3eb3a55f33ae1fd3cdc97eb6ad95))
* **server:** logout without auth clears no cookie; tests pin the German messages (AC-01, AC-08, AC-12) ([9e2a987](https://github.com/amigo-labs/amigo-sofarr/commit/9e2a987c621009c84e323e15a665cbff0d70eb9b))
* **server:** sliding cookie, unknown client on invalid forwarding, hash-password without config (AC-06, AC-09, AC-10, AC-13) ([e499a97](https://github.com/amigo-labs/amigo-sofarr/commit/e499a9794c48b257c0ec20ee706c433938f57712))
* **server:** TMDB image route loads only images sofarr handed out (discover AC-17) ([c4f0e80](https://github.com/amigo-labs/amigo-sofarr/commit/c4f0e80cf2ce8a519ada8e641197e9c66e5e523d))
* **ui:** discover add tests wait for the library page, not the preview (discover AC-07, AC-08) ([bf2b333](https://github.com/amigo-labs/amigo-sofarr/commit/bf2b333f8916930dc89ee2257f9d765fe65b857d))
* **ui:** list row samples carry the IMDb rating fields ([264cda5](https://github.com/amigo-labs/amigo-sofarr/commit/264cda5814725ddecfc50cc05f3c6e6c78a28504))
* **ui:** media info of a file in German rows with the file names' labels, not as raw answer (web-ui-library AC-05) ([e4a93a0](https://github.com/amigo-labs/amigo-sofarr/commit/e4a93a0e4153686e5ab0b69b7f36bc5fd771d0a7))
* **ui:** system sample answer carries the trailer support field ([1de1578](https://github.com/amigo-labs/amigo-sofarr/commit/1de157871e380a1fa77d3b8541b8bea3a5187b80))
