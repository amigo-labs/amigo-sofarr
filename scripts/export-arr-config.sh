#!/usr/bin/env bash
# Exportiert die Konfiguration deiner Radarr-/Sonarr-Instanzen als JSON.
# Grundlage für docs/parity.md (Parity-Scope) und spätere Fixtures.
#
# ACHTUNG: Die Exporte enthalten Secrets (Indexer-API-Keys, Passwörter der
# Download-Clients) und deine komplette Bibliothek. Sie landen deshalb in
# fixtures/private/ – das ist gitignored. Niemals committen.
#
# Nutzung:
#   RADARR_URL=http://localhost:7878 RADARR_API_KEY=... \
#   SONARR_URL=http://localhost:8989 SONARR_API_KEY=... \
#   ./scripts/export-arr-config.sh
#
# Fehlt URL oder Key einer App, wird sie übersprungen.

set -euo pipefail

OUT_DIR="$(cd "$(dirname "$0")/.." && pwd)/fixtures/private/config"

COMMON=(
  system/status
  qualityprofile
  qualitydefinition
  customformat
  indexer
  downloadclient
  notification
  importlist
  tag
  delayprofile
  releaseprofile
  rootfolder
  config/naming
  config/mediamanagement
  config/indexer
  config/downloadclient
)
RADARR_ONLY=(movie)
SONARR_ONLY=(series)

export_app () {
  local name="$1" url="$2" key="$3"; shift 3
  local dir="$OUT_DIR/$name"
  mkdir -p "$dir"
  echo "→ $name ($url)"
  for ep in "$@"; do
    local file="$dir/${ep//\//_}.json"
    if curl -fsS --max-time 30 -H "X-Api-Key: $key" "${url%/}/api/v3/$ep" -o "$file"; then
      echo "  ✓ $ep"
    else
      echo "  ✗ $ep (nicht verfügbar – übersprungen)"
      rm -f "$file"
    fi
  done
}

ran=0
if [[ -n "${RADARR_URL:-}" && -n "${RADARR_API_KEY:-}" ]]; then
  export_app radarr "$RADARR_URL" "$RADARR_API_KEY" "${COMMON[@]}" "${RADARR_ONLY[@]}"
  ran=1
fi
if [[ -n "${SONARR_URL:-}" && -n "${SONARR_API_KEY:-}" ]]; then
  export_app sonarr "$SONARR_URL" "$SONARR_API_KEY" "${COMMON[@]}" "${SONARR_ONLY[@]}"
  ran=1
fi

if [[ $ran -eq 0 ]]; then
  echo "Weder RADARR_URL/RADARR_API_KEY noch SONARR_URL/SONARR_API_KEY gesetzt." >&2
  exit 1
fi
echo "Fertig: $OUT_DIR"
