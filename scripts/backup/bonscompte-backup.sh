#!/usr/bin/env bash
# BonsCompte 3-2-1 backup: safe SQLite snapshot -> local rotation + borg to N remotes.
# Designed to run unattended from cron on the host serving the backend.
# Exit code is non-zero if the snapshot fails or ANY remote repo fails.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Defaults; override any of these in backup.env next to this script
# (or point BONSCOMPTE_BACKUP_CONF at another env file).
DB_PATH="/home/david/src/BonsCompte/backend/data/bonscompte.db"
LOCAL_SNAP_DIR="/home/david/backups/bonscompte"
LOCAL_KEEP=14
BORG_REPOS=(
  "gentoo-tv.local:bonscompte-borg"
  "dracine@tau.binarybone.com:bonscompte-borg"
)
PRUNE_OPTS=(--keep-daily 14 --keep-weekly 8 --keep-monthly 24)
export BORG_PASSPHRASE="${BORG_PASSPHRASE:-}"

CONF="${BONSCOMPTE_BACKUP_CONF:-$SCRIPT_DIR/backup.env}"
# shellcheck source=/dev/null
[ -f "$CONF" ] && source "$CONF"

LOG_FILE="$LOCAL_SNAP_DIR/backup.log"
mkdir -p "$LOCAL_SNAP_DIR"

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG_FILE"; }

timestamp="$(date +%Y%m%d-%H%M%S)"
snap="$LOCAL_SNAP_DIR/bonscompte-$timestamp.db"

# 1. Consistent snapshot of the live DB (WAL-safe, works while the server runs).
if ! sqlite3 "file:$DB_PATH?mode=ro" "VACUUM INTO '$snap'"; then
  log "FATAL: VACUUM INTO snapshot failed for $DB_PATH"
  exit 1
fi

# 2. Verify the snapshot before trusting it as a backup.
if [ "$(sqlite3 "$snap" 'PRAGMA integrity_check;')" != "ok" ]; then
  log "FATAL: integrity_check failed on snapshot $snap"
  rm -f "$snap"
  exit 1
fi
log "snapshot OK: $snap ($(du -h "$snap" | cut -f1))"

# 3. Rotate local snapshots (copy #1, fast restores).
ls -1t "$LOCAL_SNAP_DIR"/bonscompte-*.db 2>/dev/null \
  | tail -n +$((LOCAL_KEEP + 1)) | xargs -r rm --

# 4. Ship to each borg repo (copies #2 and #3). Stage under a stable file name
#    so every archive contains exactly one file: bonscompte.db
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp "$snap" "$stage/bonscompte.db"

fail=0
for repo in "${BORG_REPOS[@]}"; do
  if (cd "$stage" && borg create --compression zstd \
        "$repo::bonscompte-$timestamp" bonscompte.db) >>"$LOG_FILE" 2>&1; then
    log "borg create OK -> $repo"
    if borg prune -a 'bonscompte-*' "${PRUNE_OPTS[@]}" "$repo" >>"$LOG_FILE" 2>&1 \
       && borg compact "$repo" >>"$LOG_FILE" 2>&1; then
      log "borg prune+compact OK -> $repo"
    else
      log "WARN: prune/compact failed on $repo (backup itself succeeded)"
    fi
  else
    log "ERROR: borg create FAILED -> $repo"
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  log "backup finished WITH ERRORS"
else
  log "backup finished OK (local + ${#BORG_REPOS[@]} remotes)"
fi
exit "$fail"
