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
| Indexer-Einstellung: preferIndexerFlags | radarr: = false | 2 | nicht nötig (aus; Newznab-Indexer liefern keine Indexer-Flags) |
| Indexer-Einstellung: availabilityDelay | radarr: = 0 | 2 | umgesetzt (mit `minimumAvailability`, nur RSS und Suche beim Hinzufügen) – Orakel-Abgleich offen |
| Indexer-Einstellung: allowHardcodedSubs | radarr: = false | 2 | umgesetzt, Release-Orakel ✓ |
| Indexer-Einstellung: rssSyncInterval | radarr: = 30; sonarr: = 15 | 3 | umgesetzt (`[rss] interval_minutes`) |
| Indexer: Newznab | radarr: 2 (1 aktiv, usenet); sonarr: 2 (1 aktiv, usenet) | 3 | umgesetzt – Orakel-Abgleich offen |
| Delay Profiles | radarr: 1; sonarr: 1 | 3 | nicht nötig (keine Usenet-Verzögerung konfiguriert) |
| Delay Profile: Bypass bei höchster Quality | radarr: 1 von 1; sonarr: 1 von 1 | 3 | nicht nötig (ohne Verzögerung wirkungslos) |
| Tags | radarr: 2; sonarr: 1 | 3 | umgesetzt: Filme/Serien tragen ihre Tags, Import Lists vergeben sie, `[[indexer]] tags` beschränkt Indexer – Orakel-Abgleich offen |
| Root Folders | radarr: 1; sonarr: 1 | 3 | umgesetzt (Filmpfade aus dem Import) |
| Bibliothek: Filme | radarr: 2420 (53 überwacht) | 3 | umgesetzt – Orakel-Abgleich offen |
| Import List: TMDbPersonImport | radarr: 5 (5 aktiv) | 4 | umgesetzt – Orakel-Abgleich offen |
| Download Client: Nzbget | radarr: 1 (1 aktiv, usenet); sonarr: 1 (1 aktiv, usenet) | 4 | umgesetzt – Orakel-Abgleich offen |
| Download-Client-Einstellung: enableCompletedDownloadHandling | radarr: = true; sonarr: = true | 4 | umgesetzt – Orakel-Abgleich offen |
| Download-Client-Einstellung: autoRedownloadFailed | radarr: = false; sonarr: = true | 4 | radarr umgesetzt (aus: fehlgeschlagene Downloads werden nur markiert); sonarr offen (an) – Phase 6.1 |
| Download-Client-Einstellung: autoRedownloadFailedFromInteractiveSearch | radarr: = false; sonarr: = true | 4 | nicht nötig (keine interaktive Suche) |
| Download-Client-Einstellung: checkForFinishedDownloadInterval | radarr: = 2 | 4 | umgesetzt – Orakel-Abgleich offen |
| Media Management: downloadPropersAndRepacks | radarr: = preferAndUpgrade; sonarr: = preferAndUpgrade | 2 | umgesetzt, Release-Orakel ✓ (nur preferAndUpgrade belegt) |
| Media Management: copyUsingHardlinks | radarr: = true; sonarr: = true | 4 | bewusst anders: Usenet-Downloads werden verschoben |
| Media Management: importExtraFiles | radarr: = false; sonarr: = false | 4 | umgesetzt (aus: nur die Videodatei) |
| Media Management: autoUnmonitorPreviouslyDownloadedMovies | radarr: = true | 4 | umgesetzt (Festplatten-Scan) – Orakel-Abgleich offen |
| Media Management: createEmptyMovieFolders | radarr: = false | 4 | umgesetzt (aus: Ordner entstehen erst beim Import) |
| Media Management: deleteEmptyFolders | radarr: = true; sonarr: = true | 4 | umgesetzt (Festplatten-Scan; Download-Reste räumt `[download] remove_completed` ab) – Orakel-Abgleich offen |
| Media Management: autoRenameFolders | radarr: = false | 4 | umgesetzt (aus: bestehende Ordner bleiben) |
| Media Management: fileDate | radarr: = none; sonarr: = none | 4 | umgesetzt (none: Dateidatum bleibt) |
| Media Management: rescanAfterRefresh | radarr: = always; sonarr: = always | 4 | umgesetzt (`rescan`, nach `refresh-metadata`/`refresh-series` und dem Refresh-Job `[library]`) |
| Media Management: setPermissionsLinux | radarr: = false; sonarr: = false | 4 | umgesetzt (aus) |
| Media Management: skipFreeSpaceCheckWhenImporting | radarr: = false; sonarr: = false | 4 | umgesetzt – Orakel-Abgleich offen |
| Media Management: minimumFreeSpaceWhenImporting | radarr: = 50000; sonarr: = 10000 | 4 | umgesetzt – Orakel-Abgleich offen |
| Media Management: enableMediaInfo | radarr: = true; sonarr: = true | 4 | umgesetzt – eigener Leser für MKV/MP4 (`sofarr-mediainfo`), andere Container per Release-Namen; Orakel-Abgleich (`compare-mediainfo`) am 12.11.2026, siehe „Stand Media-Infos“ |
| Media Management: recycleBinCleanupDays | radarr: = 7; sonarr: = 7 | 4 | umgesetzt – Orakel-Abgleich offen |
| Benennung: renameMovies | radarr: = true | 4 | umgesetzt – Orakel-Abgleich offen |
| Benennung: replaceIllegalCharacters | radarr: = true; sonarr: = true | 4 | umgesetzt – Orakel-Abgleich offen |
| Benennung: colonReplacementFormat | radarr: = spaceDashSpace; sonarr: = 4 | 4 | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{-Release Group}` | radarr: standardMovieFormat; sonarr: standardEpisodeFormat | 4 | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{.MediaInfo.Simple}` | radarr: standardMovieFormat; sonarr: standardEpisodeFormat | 4 | umgesetzt – je Teil aus der Datei, sonst aus dem Release-Namen; Orakel-Abgleich offen |
| Naming-Token `{Movie CleanTitle:DE}` | radarr: standardMovieFormat | 4 | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{Movie Title:DE}` | radarr: movieFolderFormat | 4 | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{Movie TitleFirstCharacter:DE}` | radarr: movieFolderFormat | 4 | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{Quality.Full}` | radarr: standardMovieFormat; sonarr: standardEpisodeFormat | 4 | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{Release Year}` | radarr: movieFolderFormat, standardMovieFormat | 4 | umgesetzt – Orakel-Abgleich offen |
| Bibliothek: Serien | sonarr: 150 (63 überwacht) | 5+ | umgesetzt – Orakel-Abgleich offen |
| Media Management: autoUnmonitorPreviouslyDownloadedEpisodes | sonarr: = true | 5+ | umgesetzt (Festplatten-Scan) – Orakel-Abgleich offen |
| Media Management: createEmptySeriesFolders | sonarr: = true | 5+ | umgesetzt – Orakel-Abgleich offen |
| Media Management: episodeTitleRequired | sonarr: = always | 5+ | umgesetzt (Import wartet bis 48 h nach Ausstrahlung) – Orakel-Abgleich offen |
| Benennung: renameEpisodes | sonarr: = true | 5+ | umgesetzt – Orakel-Abgleich offen |
| Benennung: multiEpisodeStyle | sonarr: = 5 | 5+ | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{.MediaInfo.VideoDynamicRange}` | sonarr: standardEpisodeFormat | 5+ | umgesetzt – aus der Datei, sonst aus dem Release-Namen; Orakel-Abgleich offen |
| Naming-Token `{Air-Date}` | sonarr: dailyEpisodeFormat | 5+ | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{Episode CleanTitle}` | sonarr: standardEpisodeFormat | 5+ | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{Episode Title}` | sonarr: animeEpisodeFormat, dailyEpisodeFormat | 5+ | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{ImdbId}` | sonarr: seriesFolderFormat | 5+ | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{Quality Full}` | sonarr: animeEpisodeFormat, dailyEpisodeFormat | 5+ | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{Series Title}` | sonarr: animeEpisodeFormat, dailyEpisodeFormat, seriesFolderFormat | 5+ | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{episode:00}` | sonarr: animeEpisodeFormat, standardEpisodeFormat | 5+ | umgesetzt – Orakel-Abgleich offen |
| Naming-Token `{season:00}` | sonarr: animeEpisodeFormat, seasonFolderFormat, standardEpisodeFormat | 5+ | umgesetzt – Orakel-Abgleich offen |

<!-- Ab hier manuell gepflegt; bleibt beim Neuerzeugen erhalten. -->

## Bewusste Abweichungen

Verhalten, in dem sofarr absichtlich von Radarr/Sonarr abweicht (Spec §5.4).
Diese Fälle werden aus dem Parity-Diff ausgenommen.

| Feature | Abweichung | Begründung |
|---|---|---|
| Public-Fixtures: `response` | Aus der `/api/v3/parse`-Antwort werden nur `title`, `parsedMovieInfo` und `languages` gespeichert, nicht die komplette Antwort. | Ein Treffer in der Bibliothek liefert ein `movie`-Objekt mit Pfaden und Bibliotheksdaten; die gehören nicht ins öffentliche Repo. Private History-Fixtures bleiben roh. |
| Media-Infos: Atmos in E-AC-3 | sofarr erkennt Joint Object Coding nur in den ersten Frames; Radarr/Sonarr finden es auch weiter hinten im Datenstrom. | sofarr liest nur Header und die ersten Frames jeder Spur (Spec `mediainfo`, „Header data“), damit ein Scan großer Bibliotheken über das Netz schnell bleibt; im Abgleich betrifft das unter 1 % der Filme und höchstens 3,8 % der Episoden (alle Audio-Abweichungen bei Sonarr). |

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
| Rejections | 99,3 % |
| Quality | 99,8 % |

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
- **Quality aus dem Titel** (Release- und Parse-Orakel):
  - `BD25`/`BD50`/`BD66`/`BD100` (auch `BD-50`) ist eine volle Disc, aber nicht neben einer
    UHD-Auflösung (`UHD.BD66` → HDTV-2160p, `UHD.BluRay.x265…BD50` → Bluray-2160p); neben
    `Remux` benennt es nur die Quelle (`Remux.BD25` → Remux-1080p).
  - Volle Disc: Disc-Codec (AVC, VC-1, HEVC bei UHD) mit `DD`, DTS, TrueHD, LPCM, FLAC oder
    Atmos, oder mit `UNTOUCHED`; MPEG-2 und 3D (MVC) immer. `AC3`/`EAC3` zählen nicht
    (→ Remux), `10bit` macht ein Encode.
  - `UHDBD` ist Blu-ray, `UHDBDRip` ohne Auflösung SD; Blu-ray unter 480p zählt als 480p.
  - Region-Codes `R1`–`R6` (auch als Suffix `-r2`) ergeben REGIONAL, außer bei CAM/TS/TC.
  - `WEB` mit folgendem Bindestrich ist keine Quelle (`4K-WEB-5.1`, `WEB - DL`,
    `WEB-DLRip` → HDTV), `WEB-DL`, `WEB-Rip` und `WEB-DL2DvD` schon; `AmazonHD` und
    `HDLight` sind WEB-DL.
  - Ein Encoder ohne Quelle und Auflösung ist nur bei `x264`/`h264` SDTV; `x265` allein
    bleibt Unknown.

Hinweise zur Messung:

- Die Release-Sprachen kommen im Diff aus der Orakel-Antwort, weil Radarr dort
  Indexer-Metadaten einrechnet, die nicht in den Fixtures stehen
  (`(500) Days of Summer (2009) (tt1022603)` → English, German). Die Sprach-Erkennung des
  Parsers misst das Parse-Orakel.
- Custom-Format-Regexes mit Lookbehind am Anfang einer Alternative wertet
  `sofarr_core::pattern` selbst aus; fancy-regex matcht z. B. `(?<=\bGerman\b.*)` nie.

Noch nicht abgedeckt:

- Nicht modelliert: „Raw Bluray/DVD release“ (6×, nicht aus dem Titel ableitbar). „Has
  same release name as a grabbed and imported release“ (1×) braucht die History: RSS-Sync
  und Suche nehmen keinen Release, dessen Name einem importierten Grab gleicht; im
  Release-Orakel (ohne History) bleibt der Fall offen.
- Quality-Abweichungen des Parsers (3×, Einzelfälle ohne Gegenstück im Parse-Orakel):
  `1080p.Bluray.HEVC` mit DTSHD (Radarr: Remux), `[2160p]…WEB.AV1` (Radarr: HDTV, im
  Parse-Orakel ist `WEB.AV1` WEB-DL) und `Cam` vor dem Jahr.
- Unbelegt, weil die Config sie nicht auslöst: „zu klein“ (alle `minSize` = 0),
  Indexer-Limits (alle 0), `downloadPropersAndRepacks` ≠ `preferAndUpgrade`,
  Custom Formats mit nicht-`required` Bedingungen, ein erreichter CF-Cutoff-Score bei
  einem Profil mit Upgrades.

## Stand Media-Infos

`amigo-sofarr compare-mediainfo --app radarr|sonarr --root <Freigabe>` (Spec `mediainfo`
AC-07, AC-08); `scripts/compare-acceptance.ps1` führt alle Abgleiche in einem Lauf aus.
Lauf vom 12.11.2026 gegen die Bibliothek des Owners (MKV und MP4; AVI liegt außerhalb des
Scopes), erst ohne, dann mit den Korrekturen aus dem Abgleich (Sprach-Default, Kanäle aus den
ersten Frames):

| Feld | Radarr (2.341 Dateien) | Sonarr (3.533 Dateien) |
|---|---|---|
| Video | 99,7 % | 97,3 % |
| Audio | 99,1 % | 96,2 % |
| Kanäle | 99,1 % → 99,9 % | 99,7 % → 99,97 % |
| Dynamikumfang | 99,96 % | 99,8 % |
| Audiosprachen | 41,3 % → 99,2 % | 55,4 % → 98,8 % |
| Untertitelsprachen | 68,3 % → 99,3 % | 68,9 % → 99,7 % |

Übersprungen: Radarr 5 Dateien in nicht unterstützten Containern und 2 ohne `mediaInfo` beim
Orakel, Sonarr 92 in nicht unterstützten Containern (91 AVI).

Abweichungen nach Ursache:

| Feld | Orakel → sofarr | Ursache | Stand |
|---|---|---|---|
| Video | `h265` → `x265`, seltener `x265` → `h265` | sofarr nennt den Encoder, wenn der erste Frame eine x265-SEI trägt; das Orakel nennt für einen Teil dieser Dateien den Codec und umgekehrt, ein Muster ist von außen nicht zu erkennen | offen (unter 5 %) |
| Video | `x264` → `h264` (Sonarr), `AVC` → `h264` (Radarr, 3 Dateien) | keine Encoder-SEI im ersten Frame; `AVC` ist ein Rohname des Orakels | offen |
| Audio | `EAC3 Atmos` → `EAC3` | die Datei zeigt Joint Object Coding erst nach den ersten Frames; sofarr liest nur Header und erste Frames (Spec `mediainfo`) | bewusst |
| Audio | `DTS-ES` → `DTS` (Radarr, 3 Dateien) | der erste Kern-Frame zeigt keine Kanalerweiterung | offen |
| Kanäle | `5.1` → `5.0` (DTS) | der Matroska-Header zählt den LFE nicht mit (`Channels` 5); jetzt aus dem DTS-Kern-Frame | behoben |
| Kanäle | `2.0` → `5.1` (AC-3, Fernsehaufnahmen) | der Header meldet 6 Kanäle für einen Stereo-Strom; jetzt aus dem AC-3-Syncframe | behoben |
| Kanäle | `3.0` → `2.1` (1 Datei) | sofarr speichert nur die Anzahl, drei Kanäle heißen `2.1` | offen |
| Kanäle | `8.0` → `7.1`, `6.0` → `5.1` (je 1 Datei) | nicht untersucht | offen |
| Dynamikumfang | leer → `HDR10` (7 Dateien) | die Header tragen PQ-Transfer, BT.2020 und Mastering-Daten (mit ffprobe geprüft); das Orakel meldet nichts, bei einer Staffel mit H.266-Video liest es den Videostrom gar nicht | sofarr korrekt |
| Sprachen | `eng` → `und` | Spuren ohne `Language`-Element sind nach RFC 9559 Englisch, sofarr las sie als unbestimmt | behoben |
| Sprachen | `deu` → `ger`, leer → `und` | das Orakel gibt bei manchen Dateien den Code `deu` (ISO 639-2/T) aus und lässt Spuren unbestimmter Sprache weg; der Abgleich bildet Codes jetzt wie Namen ab und lässt `und` auf beiden Seiten weg | behoben (Abgleich) |
| Kanäle | `7.1` → `5.1` (E-AC-3, 1 Datei) | nicht untersucht | offen |

## Stand Dateinamen

`amigo-sofarr compare-rename --app radarr|sonarr` (Spec `web-ui-library` AC-23). Lauf vom
12.11.2026: Radarr 200 Filme, 43 abweichend; Sonarr 3.625 Episodendateien, 1.684 abweichend.
Nach den Korrekturen am 13.11.2026 über alle Filme (`--limit 5000`): Radarr 2.349 Filme,
rund 32 abweichend (1,4 %); davon betrifft nur ein Drittel den Titel, der Rest Codec-Token
und Revision.

| Abweichung | Ursache | Stand |
|---|---|---|
| Umlaute: Radarr `Fack ju Gohte`, `Der grosse Gatsby`; sofarr `Göhte`, `große` | Radarrs `{Movie CleanTitle}` transliteriert Umlaute und `ß` | behoben (Owner: wie Radarr, Windows-pfadtauglich) |
| Satzzeichen: Radarr behält `-` im Wort (`Super-Pets`), `.` (`Bros.`, Punktfolgen als ein Punkt), `!`, `–`, `…`, `#`, `·`, `‘` und entfernt einen freistehenden `-` sowie Apostrophe | die Annahme zu `{Movie CleanTitle}` (unten) war zu streng | behoben |
| Titel, Rest (9 Filme): Radarr behält in 4 Titeln den Apostroph (`Jetzt taut's`, `Shoot 'Em Up`, `Freddy's`, `'ne`), in über 20 anderen nicht; `11 - 14` und `2 - 22` aus `11:14`/`2:22`; `Thunderbolts-` aus `Thunderbolts*`; `Vorbilder!!`; ein doppeltes Leerzeichen | von außen kein Muster zu erkennen, je 1–4 Filme | offen |
| Codec-Token `x265`/`h265`, `x264`/`h264`, `AVC`/`HEVC` | Radarrs Name nennt hier ein anderes Wort als sein eigenes `mediaInfo`, aus dem sofarr den Namen im Abgleich bildet; nicht untersucht | offen |
| `Proper` → `Repack` (7 Filme) | Radarr schreibt `Proper`, wo sofarr aus der Revision `Repack` ableitet; nicht untersucht | offen |
| Sonarr: `h265` → `x265`, fehlendes `Atmos` | die Abgleich-Datenbank kennt die Dateien nicht (die Pfade des Servers sind auf dem vergleichenden Rechner nicht erreichbar), sofarr nimmt Codec und Atmos deshalb aus dem Release-Namen; im Betrieb liest sofarr die Dateien | Artefakt des Abgleichs, Rest wie bei den Media-Infos |
| Sonarr: `Trompe L'Oeil` → `Trompe LOeil` | Sonarr behält den Apostroph im Episodentitel | offen |

## Annahmen ab Phase 3

Verhalten, das ohne Orakel-Daten festgelegt wurde. Die Abgleiche (`compare-library`,
`compare-history`, `compare-rename`, `compare-import-lists`, `compare-series-library`) zeigen,
ob es stimmt; Abweichungen werden hier korrigiert oder als bewusst markiert.
`compare-manual-import --folder …` prüft zusätzlich die Zuordnung von Downloads gegen
Radarrs manuellen Import (Spec §5).

| Thema | Annahme in sofarr |
| --- | --- |
| RSS: Film-Zuordnung | normalisierter Titel (Titel, Originaltitel, deutscher Titel, Alternativtitel), Jahr ±1, oder IMDb-ID des Indexers; danach entscheidet die Engine aus Phase 2 (Jahr wie bei der Suche). |
| RSS: bestes Release | Quality-Rang, dann CF-Score, dann Revision (außer `doNotPrefer`), dann Größe. |
| Pending Grab | Ein offener Grab (Shadow oder Download läuft) gilt wie eine vorhandene Datei: nur bessere Releases werden genommen. |
| Release ohne Veröffentlichungsdatum | gilt als gerade veröffentlicht (Alter 0). |
| Fehlende Filme | überwacht und ohne Datei, unabhängig von der Verfügbarkeit (wie `wanted/missing` mit `monitored=true`). |
| Verfügbarkeit | Nur RSS-Sync und die Suche beim Hinzufügen warten darauf (die interaktive Suche im Release-Orakel lehnt nicht verfügbare Filme nicht ab). `announced`: immer; `inCinemas`: Kinostart oder Heimkino-Release + `availabilityDelay` erreicht; `released`: früheres von Digital- und Disc-Release + Verzögerung erreicht. Ohne Datum zählt der gemeldete Status (TMDBs `Released` heißt nur Kinostart; Radarrs Status hat Vorrang); ganz ohne Angaben gilt ein Film als verfügbar. TMDB-Daten: weltweit frühestes Datum je Typ (3/2 Kino, 4 digital, 5 Disc). |
| Tags | Ein Indexer mit `tags` bedient nur Filme/Serien mit mindestens einem gleichen Tag (Label, ohne Groß-/Kleinschreibung); ohne Tags alle. Delay Profiles, Download-Clients und Benachrichtigungen werten Tags nicht aus (nicht konfiguriert). |
| Festplatten-Scan | Fehlt eine Datei, wird sie vergessen (mit `autoUnmonitorPreviouslyDownloaded…` auch die Überwachung beendet); ein Video ohne Eintrag wird mit Quality und Gruppe aus dem Dateinamen übernommen; Filme/Serien, deren Stammordner fehlt oder leer ist (nicht gemountet), werden übersprungen, ebenso unlesbare Dateien und Ordner; wird eine fehlende Datei durch eine gefundene ersetzt (umbenannt), bleibt die Überwachung. Symbolische Links werden nie verfolgt, Ordner nur einzeln entfernt, wenn sie leer sind. `deleteEmptyFolders` entfernt leere Filmordner und leere Unterordner von Serien (Serienordner bleiben mit `createEmptySeriesFolders`); im Shadow-Mode wird nichts gelöscht. |
| Download-Reste | Nach dem Import verschwindet der Download aus der nzbget-History; sein Ordner wird gelöscht, wenn kein Video mehr darin liegt (`[download] remove_completed`, Standard an). |
| `episodeTitleRequired` | `always`: Import wartet, solange eine Episode ohne Titel (leer/`TBA`) vor weniger als 48 h lief (ohne bekanntes Datum wird nicht gewartet) und das Format den Episodentitel nutzt (bei `:DE` der deutsche Titel); `bulkSeasonReleases`: nur Staffelpakete. |
| Manueller Import (`compare-manual-import`) | Film aus dem Ordnernamen, sonst dem Dateinamen (bei mehreren Treffern das gleiche Jahr); Quality aus dem Dateinamen, sonst dem Ordnernamen. |
| Cutoff unerfüllt | gleiche Cutoff-Regel wie bei Upgrades (Profil ohne Upgrades: niedrigste erlaubte Quality). |
| CF-Score vorhandener Dateien | aus `customFormatScore` der Radarr-Antwort, sonst aus Szene- bzw. Dateiname berechnet. |
| Import | Die größte Videodatei ohne `sample` im Namen wird verschoben (gleiches Dateisystem: `rename`, sonst kopieren und löschen); Extras bleiben liegen. |
| Alte Datei bei Upgrade | in den Papierkorb (`recycleBin`), falls gesetzt, sonst gelöscht. |
| `{Movie CleanTitle}` (auch Serie und Episode) | Buchstaben ohne Diakritika (`ö` → `o`, `ß` → `ss`, `æ` → `ae`), Ziffern, Leerzeichen, `-` im Wort, `.` (Punktfolgen als ein Punkt), `!`, `–`, `…`, `#`, `·` und `‘` bleiben; ein freistehender `-` und Apostrophe entfallen; `&` wird `and`, übrige Satzzeichen entfallen. Abgeleitet aus dem Abgleich vom 12.11.2026 (siehe „Stand Dateinamen“); Ligaturen und andere Diakritika ohne Beleg. |
| `{Movie TitleFirstCharacter}` | erster Buchstabe/Ziffer des Titels ohne führenden Artikel (The/A/An/Der/Die/Das), groß. |
| Unerlaubte Zeichen | `/` und `\\` werden `+`, `? * " < > \|` entfallen; keine Ordnerebene endet auf Punkt, Leerzeichen, `-` oder `_` (Windows lehnt Punkt und Leerzeichen am Ende ab). |
| `{Quality Full}` | Quality-Name, dann `Proper`/`Repack` (ab Version 2, ab 3 mit Zahl) und `REAL`. |
| Import Lists (TMDb-Person) | Cast und die gewählten Crew-Abteilungen (Directing, Production, Sound, Writing); Ausschlüsse aus `exclusions`. |
| Serien: Staffelpakete | decken alle Episoden der Staffel(n) ab; maßgeblich ist die schwächste vorhandene Datei der überwachten, ausgestrahlten Episoden; ein Paket schlägt einzelne Episoden, die es enthält. |
| Serien: Regeln | dieselben wie für Filme (Quality, Sprache, Größe pro Episode, CF, Upgrades), aber ohne die filmspezifische Jahr-Prüfung. |
| Serien: ausgestrahlt | Ausstrahlungsdatum ≤ heute (UTC). |
| Serien: `multiEpisodeStyle` | 0 Extend, 1 Duplicate, 2 Repeat, 3 Scene, 4 Range, 5 Prefixed Range; Duplicate schreibt die Staffel mit der Auffüllung der Episode. |
| Serien: `colonReplacementFormat` (numerisch) | 0 Delete, 1 Dash, 2 Space Dash, 3 Space Dash Space, 4 Smart. |
| Serien: Episoden-Import | jede Videodatei wird über ihren Namen (sonst den Grab-Titel) Episoden zugeordnet. |
| Fehlgeschlagene Downloads | Release-Fehler (`FAILURE/HEALTH`, `/PAR`, `/UNPACK`, `DELETED/HEALTH`, `/MANUAL`) und kaputte NZBs (HTTP 404/410, Newznab-Fehler 300, von nzbget abgelehnt) kommen auf die Blocklist des Films bzw. der Serie (GUID und normalisierter Titel, ohne Ablauf); lokale Fehler, Ausfälle und verschwundene Downloads nicht. Re-Suche je Medienart laut `[download] redownload_failed_*`, sonst `autoRedownloadFailed` aus dem Import, sonst aus (Spec `fail-handling`). |
| Verschwundene Downloads | Ein aktiver Grab, dessen Download bei jedem erfolgreichen Download-Check seit 24 Stunden weder in Queue noch History steht, gilt als fehlgeschlagen; nicht erreichbare Checks zählen nicht. |
| Serien-Refresh: neue Episoden | Eine neue Episode einer bekannten Staffel wird nur überwacht, wenn in dieser Staffel schon eine Episode überwacht ist; eine neue Staffel folgt dem Schalter „neue Staffeln überwachen“ der Serie (Standard an, Sonarrs eigene Einstellung wird nicht übernommen). Aus Sonarr importierte Serien mit einer ganz unüberwachten Staffel behalten diese unüberwacht (D28). |
