#!/usr/bin/env bash
# BonsCompte restore helper.
#
#   bonscompte-restore.sh list [repo]            List available archives
#   bonscompte-restore.sh restore [archive] [repo]
#                                                Extract archive (default: latest)
#                                                to a temp dir, verify integrity,
#                                                and print swap-in instructions.
#
# Local snapshots (no borg needed) also live in $LOCAL_SNAP_DIR — for a quick
# restore just stop the server and copy one over the live DB.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

DB_PATH="/home/david/src/BonsCompte/backend/data/bonscompte.db"
LOCAL_SNAP_DIR="/home/david/backups/bonscompte"
BORG_REPOS=(
  "gentoo-tv.local:bonscompte-borg"
  "dracine@tau.binarybone.com:bonscompte-borg"
)
export BORG_PASSPHRASE="${BORG_PASSPHRASE:-}"

CONF="${BONSCOMPTE_BACKUP_CONF:-$SCRIPT_DIR/backup.env}"
# shellcheck source=/dev/null
[ -f "$CONF" ] && source "$CONF"

cmd="${1:-list}"

case "$cmd" in
  list)
    repo="${2:-${BORG_REPOS[0]}}"
    echo "== Archives in $repo =="
    borg list -a 'bonscompte-*' "$repo"
    echo
    echo "== Local snapshots in $LOCAL_SNAP_DIR =="
    ls -1t "$LOCAL_SNAP_DIR"/bonscompte-*.db 2>/dev/null || echo "(none)"
    ;;
  restore)
    archive="${2:-}"
    repo="${3:-${BORG_REPOS[0]}}"
    if [ -z "$archive" ]; then
      archive="$(borg list -a 'bonscompte-*' --short --last 1 "$repo")"
      [ -n "$archive" ] || { echo "No archives found in $repo" >&2; exit 1; }
      echo "No archive given; using latest: $archive"
    fi
    dest="$(mktemp -d /tmp/bonscompte-restore-XXXXXX)"
    (cd "$dest" && borg extract "$repo::$archive")
    restored="$dest/bonscompte.db"
    [ -f "$restored" ] || { echo "Archive did not contain bonscompte.db" >&2; exit 1; }
    check="$(sqlite3 "$restored" 'PRAGMA integrity_check;')"
    echo "integrity_check: $check"
    [ "$check" = "ok" ] || { echo "Restored DB FAILED integrity check" >&2; exit 1; }
    echo
    echo "Restored to: $restored"
    echo "To swap it in:"
    echo "  1. Stop the backend server"
    echo "  2. cp '$DB_PATH' '$DB_PATH.pre-restore'   # keep the current DB just in case"
    echo "  3. rm -f '$DB_PATH-wal' '$DB_PATH-shm'"
    echo "  4. cp '$restored' '$DB_PATH'"
    echo "  5. Start the backend server"
    ;;
  *)
    echo "Usage: $0 list [repo] | restore [archive] [repo]" >&2
    exit 1
    ;;
esac
