# Backups (3-2-1)

BonsCompte's SQLite database is backed up on a 3-2-1 scheme
(3 copies, 2 machines, 1 offsite):

| Copy | Where | What |
|---|---|---|
| 1 | This host, `~/backups/bonscompte/` | Last 14 daily `VACUUM INTO` snapshots (fast restores, no borg needed) |
| 2 | LAN host (`BORG_REPOS[0]`) | Borg archives, LAN |
| 3 | Offsite host (`BORG_REPOS[1]`) | Borg archives, offsite |

Everything lives in `scripts/backup/`:

- `bonscompte-backup.sh` — the whole pipeline, run daily by cron (fcron, 03:30):
  1. `VACUUM INTO` snapshot of the live DB (WAL-safe; the server keeps running)
  2. `PRAGMA integrity_check` on the snapshot — a corrupt snapshot is never shipped
  3. rotate local snapshots (keep 14)
  4. `borg create` (zstd) to each repo, then `borg prune` (14 daily / 8 weekly /
     24 monthly) + `borg compact`
  5. non-zero exit if the snapshot or any remote fails; log at
     `~/backups/bonscompte/backup.log`
- `bonscompte-restore.sh` — `list` archives / `restore` one to a temp dir with an
  integrity check and swap-in instructions
- `backup.env.example` — copy to `backup.env` (git-ignored) and set
  `BACKUP_INSTANCE` + `BORG_REPOS` (both required; repo hosts are kept out of
  git); optionally override paths and retention

## One instance per repo

Only the production host should have a `backup.env` pointing at the production
repos. A dev checkout must either have no `backup.env` (the script then refuses
to run) or its own `BACKUP_INSTANCE` and its own repos. Safeguards:

- Archives are named `bonscompte-<BACKUP_INSTANCE>-<timestamp>`, and `borg prune`
  only matches the instance's own prefix, so instances never prune each other.
- Before writing, the backup script lists each repo and **refuses to write into a
  repo holding any archive from another instance** (logged as `ERROR: refusing to
  back up to ...`). A dev machine pointed at the prod repos fails loudly instead
  of mixing its database into prod's history.
- `bonscompte-restore.sh restore` with no archive name only picks the latest
  archive of the configured `BACKUP_INSTANCE`; without one it requires an
  explicit archive name.

## Restore

Quick (local snapshot, no borg):

```bash
# stop backend, then:
cp ~/backups/bonscompte/bonscompte-<timestamp>.db backend/data/bonscompte.db
rm -f backend/data/bonscompte.db-wal backend/data/bonscompte.db-shm
# start backend
```

From borg (works even if this machine is gone — run on any host with the SSH key
and a `backup.env` naming the same `BACKUP_INSTANCE`):

```bash
scripts/backup/bonscompte-restore.sh list
scripts/backup/bonscompte-restore.sh restore                      # latest, LAN repo
scripts/backup/bonscompte-restore.sh restore "" user@offsite-host:bonscompte-borg
```

The restore path was tested end-to-end on 2026-07-05 (archive pulled from the
offsite repo, `integrity_check: ok`), but from a dev checkout, so the database
in that test was not production data.

## Encryption & keys

Repos use borg **repokey** with an **empty passphrase** (same convention as the
immich backups: nothing to type, nothing to lose). The repo key lives inside each
repo; exported copies are kept at `~/backups/bonscompte/keys/`. To add a real
passphrase later: `borg key change-passphrase <repo>`, then set `BORG_PASSPHRASE`
in `backup.env`.

Consequence of the empty passphrase: anyone with SSH access to a backup host can
read the archives. Acceptable for now (both hosts are David's); revisit alongside
issue #225 (encrypt DB).

## Monitoring

Cron discards stdout; failures are visible in `~/backups/bonscompte/backup.log`
(lines containing `ERROR`/`FATAL`). Check occasionally, or wire the exit code to a
notifier later. If this host is ever replaced, re-add the fcrontab line:

```
30 3 * * * /path/to/BonsCompte/scripts/backup/bonscompte-backup.sh >/dev/null 2>&1
```
