# Parity-Matrix

> Erzeugt mit `cargo run -p sofarr-parity -- matrix` aus dem Config-Export
> (`scripts/export-arr-config.sh`). Die Tabelle nicht von Hand ändern – beim
> Neuerzeugen bleibt nur die Spalte **Status** erhalten. Enthält bewusst keine
> Namen, URLs, Pfade oder Keys.

> **Noch nicht erzeugt.** Ablauf (lokal, mit Zugriff auf Radarr/Sonarr):
>
> ```sh
> RADARR_URL=http://<host>:7878 RADARR_API_KEY=… ./scripts/export-arr-config.sh
> cargo run -p sofarr-parity -- matrix
> ```

| Feature | genutzt in | Phase | Status |
|---|---|---|---|

<!-- Ab hier manuell gepflegt; bleibt beim Neuerzeugen erhalten. -->

## Bewusste Abweichungen

Verhalten, in dem sofarr absichtlich von Radarr/Sonarr abweicht (Spec §5.4).
Diese Fälle werden aus dem Parity-Diff ausgenommen.

| Feature | Abweichung | Begründung |
|---|---|---|
| Public-Fixtures: `response` | Aus der `/api/v3/parse`-Antwort werden nur `title`, `parsedMovieInfo` und `languages` gespeichert, nicht die komplette Antwort. | Ein Treffer in der Bibliothek liefert ein `movie`-Objekt mit Pfaden und Bibliotheksdaten; die gehören nicht ins öffentliche Repo. Private History-Fixtures bleiben roh. |
