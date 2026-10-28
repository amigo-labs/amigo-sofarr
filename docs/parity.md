# Parity-Matrix

> Erzeugt mit `cargo run -p sofarr-parity -- matrix` aus dem Config-Export
> (`scripts/export-arr-config.sh`). Die Tabelle nicht von Hand ändern – beim
> Neuerzeugen bleibt nur die Spalte **Status** erhalten. Enthält bewusst keine
> Namen, URLs, Pfade oder Keys.

Orakel: radarr 6.4.4.10685, sonarr 4.0.20.3014

| Feature | genutzt in | Phase | Status |
| --- | --- | --- | --- |
| Quality Profiles (Cutoff) | radarr: 2 Profile; sonarr: 2 Profile | 2 | umgesetzt, Release-Orakel ✓ |
| Quality Profile: Upgrades erlaubt | radarr: 1 von 2 Profilen; sonarr: 1 von 2 Profilen | 2 | umgesetzt, Release-Orakel ✓ |
| Quality Profile: Quality-Gruppen | radarr: 2 von 2 Profilen; sonarr: 2 von 2 Profilen | 2 | umgesetzt, Release-Orakel ✓ |
| Quality Profile: Custom-Format-Scores | radarr: 1 von 2 Profilen; sonarr: 1 von 2 Profilen | 2 | umgesetzt, Release-Orakel ✓ |
| Quality Profile: Mindest-/Cutoff-Score | radarr: 1 von 2 Profilen; sonarr: 1 von 2 Profilen | 2 | umgesetzt, Release-Orakel ✓ |
| Profil-Sprache: English | radarr: 1 von 2 Profilen | 2 | umgesetzt, Release-Orakel ✓ |
| Profil-Sprache: German | radarr: 1 von 2 Profilen | 2 | umgesetzt, Release-Orakel ✓ |
| Quality: Bluray-1080p | radarr: erlaubt in 1 von 2 Profilen; sonarr: erlaubt in 1 von 2 Profilen | 2 | umgesetzt (Parse-Orakel ✓) |
| Quality: Bluray-2160p | radarr: erlaubt in 1 von 2 Profilen; sonarr: erlaubt in 1 von 2 Profilen | 2 | umgesetzt (Parse-Orakel ✓) |
| Quality: HDTV-1080p | radarr: erlaubt in 1 von 2 Profilen; sonarr: erlaubt in 1 von 2 Profilen | 2 | umgesetzt (Parse-Orakel ✓) |
| Quality: HDTV-2160p | radarr: erlaubt in 1 von 2 Profilen; sonarr: erlaubt in 1 von 2 Profilen | 2 | umgesetzt (Parse-Orakel ✓) |
| Quality: Remux-2160p | radarr: erlaubt in 1 von 2 Profilen | 2 | umgesetzt (Parse-Orakel ✓) |
| Quality: WEBDL-1080p | radarr: erlaubt in 1 von 2 Profilen; sonarr: erlaubt in 1 von 2 Profilen | 2 | umgesetzt (Parse-Orakel ✓) |
| Quality: WEBDL-2160p | radarr: erlaubt in 1 von 2 Profilen; sonarr: erlaubt in 1 von 2 Profilen | 2 | umgesetzt (Parse-Orakel ✓) |
| Quality: WEBRip-1080p | radarr: erlaubt in 1 von 2 Profilen; sonarr: erlaubt in 1 von 2 Profilen | 2 | umgesetzt (Parse-Orakel ✓) |
| Quality: WEBRip-2160p | radarr: erlaubt in 1 von 2 Profilen; sonarr: erlaubt in 1 von 2 Profilen | 2 | umgesetzt (Parse-Orakel ✓) |
| Custom Formats | radarr: 6; sonarr: 5 | 2 | umgesetzt, Release-Orakel ✓ |
| CF-Bedingung: LanguageSpecification | radarr: 5× (negiert 2, Pflicht 5); sonarr: 5× (negiert 2, Pflicht 5) | 2 | umgesetzt, Release-Orakel ✓ |
| CF-Bedingung: ReleaseGroupSpecification | radarr: 1× (negiert 0, Pflicht 1) | 2 | umgesetzt, Release-Orakel ✓ |
| CF-Bedingung: ReleaseTitleSpecification | radarr: 6× (negiert 3, Pflicht 6); sonarr: 6× (negiert 3, Pflicht 6) | 2 | umgesetzt, Release-Orakel ✓ |
| Quality Definitions | radarr: 30; sonarr: 22 | 2 | umgesetzt, Release-Orakel ✓ |
| Indexer-Einstellung: minimumAge | radarr: = 0; sonarr: = 0 | 2 | umgesetzt, im Orakel unbelegt (= 0) |
| Indexer-Einstellung: maximumSize | radarr: = 0; sonarr: = 0 | 2 | umgesetzt, im Orakel unbelegt (= 0) |
| Indexer-Einstellung: retention | radarr: = 0; sonarr: = 0 | 2 | umgesetzt, im Orakel unbelegt (= 0) |
| Indexer-Einstellung: preferIndexerFlags | radarr: = false | 2 | offen |
| Indexer-Einstellung: availabilityDelay | radarr: = 0 | 2 | offen |
| Indexer-Einstellung: allowHardcodedSubs | radarr: = false | 2 | umgesetzt, Release-Orakel ✓ |
| Indexer-Einstellung: rssSyncInterval | radarr: = 30; sonarr: = 15 | 3 | offen |
| Indexer: Newznab | radarr: 2 (1 aktiv, usenet); sonarr: 2 (1 aktiv, usenet) | 3 | offen |
| Delay Profiles | radarr: 1; sonarr: 1 | 3 | offen |
| Delay Profile: Bypass bei höchster Quality | radarr: 1 von 1; sonarr: 1 von 1 | 3 | offen |
| Tags | radarr: 2; sonarr: 1 | 3 | offen |
| Root Folders | radarr: 1; sonarr: 1 | 3 | offen |
| Bibliothek: Filme | radarr: 2420 (53 überwacht) | 3 | offen |
| Import List: TMDbPersonImport | radarr: 5 (5 aktiv) | 4 | offen |
| Download Client: Nzbget | radarr: 1 (1 aktiv, usenet); sonarr: 1 (1 aktiv, usenet) | 4 | offen |
| Download-Client-Einstellung: enableCompletedDownloadHandling | radarr: = true; sonarr: = true | 4 | offen |
| Download-Client-Einstellung: autoRedownloadFailed | radarr: = false; sonarr: = true | 4 | offen |
| Download-Client-Einstellung: autoRedownloadFailedFromInteractiveSearch | radarr: = false; sonarr: = true | 4 | offen |
| Download-Client-Einstellung: checkForFinishedDownloadInterval | radarr: = 2 | 4 | offen |
| Media Management: downloadPropersAndRepacks | radarr: = preferAndUpgrade; sonarr: = preferAndUpgrade | 2 | umgesetzt, Release-Orakel ✓ (nur preferAndUpgrade belegt) |
| Media Management: copyUsingHardlinks | radarr: = true; sonarr: = true | 4 | offen |
| Media Management: importExtraFiles | radarr: = false; sonarr: = false | 4 | offen |
| Media Management: autoUnmonitorPreviouslyDownloadedMovies | radarr: = true | 4 | offen |
| Media Management: createEmptyMovieFolders | radarr: = false | 4 | offen |
| Media Management: deleteEmptyFolders | radarr: = true; sonarr: = true | 4 | offen |
| Media Management: autoRenameFolders | radarr: = false | 4 | offen |
| Media Management: fileDate | radarr: = none; sonarr: = none | 4 | offen |
| Media Management: rescanAfterRefresh | radarr: = always; sonarr: = always | 4 | offen |
| Media Management: setPermissionsLinux | radarr: = false; sonarr: = false | 4 | offen |
| Media Management: skipFreeSpaceCheckWhenImporting | radarr: = false; sonarr: = false | 4 | offen |
| Media Management: minimumFreeSpaceWhenImporting | radarr: = 50000; sonarr: = 10000 | 4 | offen |
| Media Management: enableMediaInfo | radarr: = true; sonarr: = true | 4 | offen |
| Media Management: recycleBinCleanupDays | radarr: = 7; sonarr: = 7 | 4 | offen |
| Benennung: renameMovies | radarr: = true | 4 | offen |
| Benennung: replaceIllegalCharacters | radarr: = true; sonarr: = true | 4 | offen |
| Benennung: colonReplacementFormat | radarr: = spaceDashSpace; sonarr: = 4 | 4 | offen |
| Naming-Token `{-Release Group}` | radarr: standardMovieFormat; sonarr: standardEpisodeFormat | 4 | offen |
| Naming-Token `{.MediaInfo.Simple}` | radarr: standardMovieFormat; sonarr: standardEpisodeFormat | 4 | offen |
| Naming-Token `{Movie CleanTitle:DE}` | radarr: standardMovieFormat | 4 | offen |
| Naming-Token `{Movie Title:DE}` | radarr: movieFolderFormat | 4 | offen |
| Naming-Token `{Movie TitleFirstCharacter:DE}` | radarr: movieFolderFormat | 4 | offen |
| Naming-Token `{Quality.Full}` | radarr: standardMovieFormat; sonarr: standardEpisodeFormat | 4 | offen |
| Naming-Token `{Release Year}` | radarr: movieFolderFormat, standardMovieFormat | 4 | offen |
| Bibliothek: Serien | sonarr: 150 (63 überwacht) | 5+ | offen |
| Media Management: autoUnmonitorPreviouslyDownloadedEpisodes | sonarr: = true | 5+ | offen |
| Media Management: createEmptySeriesFolders | sonarr: = true | 5+ | offen |
| Media Management: episodeTitleRequired | sonarr: = always | 5+ | offen |
| Benennung: renameEpisodes | sonarr: = true | 5+ | offen |
| Benennung: multiEpisodeStyle | sonarr: = 5 | 5+ | offen |
| Naming-Token `{.MediaInfo.VideoDynamicRange}` | sonarr: standardEpisodeFormat | 5+ | offen |
| Naming-Token `{Air-Date}` | sonarr: dailyEpisodeFormat | 5+ | offen |
| Naming-Token `{Episode CleanTitle}` | sonarr: standardEpisodeFormat | 5+ | offen |
| Naming-Token `{Episode Title}` | sonarr: animeEpisodeFormat, dailyEpisodeFormat | 5+ | offen |
| Naming-Token `{ImdbId}` | sonarr: seriesFolderFormat | 5+ | offen |
| Naming-Token `{Quality Full}` | sonarr: animeEpisodeFormat, dailyEpisodeFormat | 5+ | offen |
| Naming-Token `{Series Title}` | sonarr: animeEpisodeFormat, dailyEpisodeFormat, seriesFolderFormat | 5+ | offen |
| Naming-Token `{episode:00}` | sonarr: animeEpisodeFormat, standardEpisodeFormat | 5+ | offen |
| Naming-Token `{season:00}` | sonarr: animeEpisodeFormat, seasonFolderFormat, standardEpisodeFormat | 5+ | offen |

<!-- Ab hier manuell gepflegt; bleibt beim Neuerzeugen erhalten. -->

## Bewusste Abweichungen

Verhalten, in dem sofarr absichtlich von Radarr/Sonarr abweicht (Spec §5.4).
Diese Fälle werden aus dem Parity-Diff ausgenommen.

| Feature | Abweichung | Begründung |
|---|---|---|
| Public-Fixtures: `response` | Aus der `/api/v3/parse`-Antwort werden nur `title`, `parsedMovieInfo` und `languages` gespeichert, nicht die komplette Antwort. | Ein Treffer in der Bibliothek liefert ein `movie`-Objekt mit Pfaden und Bibliotheksdaten; die gehören nicht ins öffentliche Repo. Private History-Fixtures bleiben roh. |

## Stand Release-Orakel

`cargo test -p sofarr-parity --test release_oracle` (CI-Ratchet, Baseline
`fixtures/decision-baseline.toml`) bzw. `cargo run -p sofarr-parity -- diff releases`
(inkl. privater Fixtures, nur Report).

1.703 öffentliche Releases aus 20 Filmen: beide Profile, Filme mit und ohne Datei, Dateien
in allen vorkommenden Qualities und Revisionen.

| Feld | Pass-Rate |
|---|---|
| approved | 100 % |
| Custom Formats | 99,8 % |
| Score | 99,8 % |
| Rejections | 98,4 % |
| Quality | 98,0 % |

Aus den Fixtures abgeleitete Regeln:

- **Film-Zuordnung:** Bei der Suche für einen Film entscheidet nur das Jahr im Release-Namen
  (`year` oder `secondaryYear` des Films), nicht der Titel. Ohne Jahr wird jeder Titel
  genommen, Fortsetzungen mit eigenem Jahr (`Blade.2.2002`) also abgelehnt, ohne Jahr
  (`Iron.Man.1-3.German…`) nicht.
- **Unverständliche Titel:** Ohne Jahr versteht Radarr nur Titel, die als German markiert
  sind; sonst „Unable to parse release“, ohne weitere Prüfungen.
- **Upgrades:** Quality-Rang, dann Revision, dann CF-Score. Erreicht die Datei die
  Cutoff-Quality, zählt eine höhere Quality nicht mehr, eine höhere Revision schon. Ein
  Profil ohne Upgrades hat die niedrigste erlaubte Quality als Cutoff. Ein Repack ersetzt
  nur eine Datei derselben Release-Gruppe.
- **Größe:** MB = 1024².

Hinweise zur Messung:

- Die Release-Sprachen kommen im Diff aus der Orakel-Antwort, weil Radarr dort
  Indexer-Metadaten einrechnet, die nicht in den Fixtures stehen
  (`(500) Days of Summer (2009) (tt1022603)` → English, German). Die Sprach-Erkennung des
  Parsers misst das Parse-Orakel.
- Custom-Format-Regexes mit Lookbehind am Anfang einer Alternative wertet
  `sofarr_core::pattern` selbst aus; fancy-regex matcht z. B. `(?<=\bGerman\b.*)` nie.

Noch nicht abgedeckt:

- Nicht modelliert: „Raw Bluray/DVD release“ (6×, nicht aus dem Titel ableitbar) und „Has
  same release name as a grabbed and imported release“ (1×, braucht die History, Phase 3).
- Quality-Abweichungen des Parsers (34×, v. a. BR-DISK/Remux/Bluray und HDTV/WEB-DL).
- Unbelegt, weil die Config sie nicht auslöst: „zu klein“ (alle `minSize` = 0),
  Indexer-Limits (alle 0), `downloadPropersAndRepacks` ≠ `preferAndUpgrade`,
  Custom Formats mit nicht-`required` Bedingungen, ein erreichter CF-Cutoff-Score bei
  einem Profil mit Upgrades.
