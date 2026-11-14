# Backups (3-2-1)

BonsCompte's SQLite database is backed up on a 3-2-1 scheme
(3 copies, 2 machines, 1 offsite):

| Copy | Where | What |
|---|---|---|
| 1 | Production host, `LOCAL_SNAP_DIR` | Last 14 daily `VACUUM INTO` snapshots (fast restores, no borg needed) |
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
     `$LOCAL_SNAP_DIR/backup.log`
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

## Production setup

Production runs the backend in Docker, so the database lives in the
`bonscompte_sqlite_data` volume under `/var/lib/docker`, readable only by root.
The backup therefore runs as root, from a copy of the scripts outside any
checkout:

- Scripts in `/usr/local/lib/bonscompte-backup/` (root-owned), with a root-only
  (`600`) `backup.env` there setting `BACKUP_INSTANCE="prod"`, `BORG_REPOS`
  (with explicit `user@host`, since root's default SSH user would be wrong),
  `DB_PATH` into the volume, `LOCAL_SNAP_DIR="/var/backups/bonscompte"`, and
  `export BORG_RSH="ssh -i /root/.ssh/id_bonscompte_borg -o IdentitiesOnly=yes -o BatchMode=yes"`.
- A dedicated, passphrase-less key `/root/.ssh/id_bonscompte_borg`, used for
  nothing else. On each backup host it is pinned in `authorized_keys` to
  append-only `borg serve` on this one repo:

  ```
  command="borg serve --append-only --restrict-to-path /home/<user>/bonscompte-borg",restrict ssh-ed25519 AAAA... bonscompte-backup@<prod-host>
  ```

  (On NixOS, point `command=` at a small wrapper script that `exec`s the system
  borg with the same arguments.) The key cannot open a shell or reach any other
  repo, and a compromised production host cannot destroy existing archives.
- The backup hosts' SSH host keys are pinned in root's `known_hosts`
  (`BatchMode=yes` never prompts).
- Root's fcrontab: `30 3 * * * /usr/local/lib/bonscompte-backup/bonscompte-backup.sh >/dev/null 2>&1`.

The first time root opens a repo, borg asks to confirm the unknown repository;
answer it once with `BORG_UNKNOWN_UNENCRYPTED_REPO_ACCESS_IS_OK=yes borg info <repo>`
after checking the repository ID.

Append-only means the nightly `borg prune` only hides old archives: their space
is reclaimed when `borg compact` runs over a full-access login on the backup
host. With a database this small that can wait months.

## Restore

Quick (local snapshot, no borg):

```bash
# stop backend, then (DB = DB_PATH, e.g. the Docker volume in production):
cp "$LOCAL_SNAP_DIR"/bonscompte-<timestamp>.db "$DB"
rm -f "$DB-wal" "$DB-shm"
# start backend
```

From borg (works even if this machine is gone — run on any host with the SSH key
and a `backup.env` naming the same `BACKUP_INSTANCE`):

```bash
scripts/backup/bonscompte-restore.sh list
scripts/backup/bonscompte-restore.sh restore                      # latest, LAN repo
scripts/backup/bonscompte-restore.sh restore "" user@offsite-host:bonscompte-borg
```

The restore path was tested end-to-end on production on 2026-11-13: the latest
`prod` archive pulled from the offsite repo passed `integrity_check` and matched
the live database row for row in all 13 tables. (An earlier 2026-07-05 test ran
from a dev checkout and only covered dummy data.)

## Encryption & keys

Repos use borg **repokey** with an **empty passphrase** (same convention as the
immich backups: nothing to type, nothing to lose). The repo key lives inside each
repo; exported copies are kept off the backup hosts, on the admin workstation at
`~/backups/bonscompte/keys/`. To add a real
passphrase later: `borg key change-passphrase <repo>`, then set `BORG_PASSPHRASE`
in `backup.env`.

Consequence of the empty passphrase: anyone with SSH access to a backup host can
read the archives. Acceptable for now (both hosts are David's); revisit alongside
issue #225 (encrypt DB).

## Monitoring

Cron discards stdout; failures are visible in `$LOCAL_SNAP_DIR/backup.log`
(lines containing `ERROR`/`FATAL`). Check occasionally, or wire the exit code to a
notifier later. If the production host is ever replaced, redo the steps in
[Production setup](#production-setup).
