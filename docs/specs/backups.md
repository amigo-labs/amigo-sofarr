---
status: implemented
created: 2026-11-12
size: M
design: [sofarr.md §3, sofarr.md §7, sofarr.md §9 D4, sofarr.md §9 D14]
media: none
oracle: none
---

# Backups

## Summary

> Completed: 2026-11-12

- Scheduled backups (`[backup] interval_hours`, default 24, 0 = off) via `VACUUM INTO` into `[backup] dir`, pruned to `[backup] keep` (future-dated first, then oldest; manual, `other` and the pending restore stay).
- Manual backups through `POST /api/backups` and `amigo-sofarr backup`; a pre-migration backup before pending schema migrations at start (three newest kept), a failed one stops the start.
- Backup API: list with kind, size, time, validity, pending restore and last error; download, delete; names outside the folder or of non-sofarr files answer 404.
- Restore chosen through the API (marker `sofarr.db.restore`) and applied at the next `serve` start, or directly through `amigo-sofarr restore` while no server holds `sofarr.db.lock`; the previous database and its journals go aside as `sofarr.db.replaced-<time>`, and any failure puts the previous database back.
- Backup operations are serialised per process, names are reserved exclusively across processes, and backups never contain `sofarr.toml`.

## Context

sofarr keeps its whole state – library, files, grabs, imported settings, blocklist – in one
SQLite file (`[database] path`, default `/config/sofarr.db`, D4). Today nothing copies it: a
broken disk, a bad migration or a mistake loses the library state, and a schema migration of a
new version cannot be undone. D14 decides backups as consistent copies taken while sofarr runs,
a snapshot before every migration, and no secrets in a backup (the config file stays out).

Terms used below:

- **Backup** – a sofarr database file in the backup folder. Its **kind** comes from its name:
  `scheduled` (taken by the timer), `manual` (started by the user), `pre-migration` (taken at
  start before pending schema migrations run), or `other` for a sofarr database whose name
  matches none of these (e.g. copied into the folder by hand).
- **sofarr database** – a SQLite file whose migration table starts with sofarr's first migration;
  decided by opening the file, not by its name.
- **Backup time** – the UTC time in the backup's name; for kind `other` the file's modification
  time, because such a name carries no time.
- **Backup folder** – `[backup] dir`, default `/config/backups`.
- **Valid backup** – a sofarr database that passes an integrity check and has no migration
  applied that this sofarr version does not know (a database of a newer version is a sofarr
  database but not a valid backup).
- **Pending restore** – a valid backup the user chose to put in place at the next start.
- **Running server** – an `amigo-sofarr serve` process using the same database file.

## Scope

### In scope

- Scheduled, manual and pre-migration backups of the database
- Pruning of old scheduled and pre-migration backups
- Listing, downloading and deleting backups through the API
- Restoring a backup: chosen through the API and applied at the next start, or directly through the CLI while the server is stopped

### Out of scope

- Backing up `sofarr.toml` — because it holds API keys and passwords (D14); the owner backs up `/config` with the host
- Backing up media, downloads or logs — because they are not sofarr's state and are large
- Uploading a backup file through the API — because a file copied into the backup folder is listed like any other backup
- Off-site upload — because the backup folder can be any mounted path, e.g. a NAS
- Encryption — because a backup holds no secrets (D14) and stays on the owner's own storage
- Restoring while the server keeps running — because the open database would have to be swapped under running jobs
- A configurable number of pre-migration backups — because migrations are rare and three steps back are enough; one key less

## Acceptance criteria

- [x] AC-01 (event): When the backup interval has passed since the backup time of the newest scheduled backup that does not lie in the future (or there is none), then sofarr writes a scheduled backup within 5 minutes of that moment or of its start, whichever is later — at most one per interval — while it keeps answering API requests.
- [x] AC-02 (always): Every backup sofarr writes is a valid backup that contains every change committed before the backup was requested; a change committed while the backup runs is either completely in it or not at all.
- [x] AC-03 (event): When a scheduled backup was written, then sofarr deletes scheduled backups beyond the configured number to keep — first those whose backup time lies in the future, then the oldest by backup time; manual and `other` backups and the pending restore are never deleted automatically.
- [x] AC-04 (event): When sofarr starts normally (no pending restore) with an existing database that has schema migrations pending, then sofarr writes a pre-migration backup before running them and keeps the three newest pre-migration backups (by backup time).
- [x] AC-05 (state): While no migration is pending, or no database exists yet, a start writes no pre-migration backup.
- [x] AC-06 (event): When the user starts a backup through the API or the CLI, then sofarr writes a manual backup and answers with its name, size and backup time; a CLI backup that fails prints a German message and exits with a code ≠ 0.
- [x] AC-07 (error): If a backup of any kind cannot be written (folder missing and not creatable, not writable, disk full), then no partial backup file remains, no older backup is deleted, the error is logged, and the backup list in the API shows the last error with its time until a later backup succeeds or sofarr restarts.
- [x] AC-22 (error): If pruning or deleting a backup fails (e.g. permission denied), then the backup stays, the error is logged and shown as in AC-07, and an API delete answers status 500 with a German message; the new backup that triggered the pruning still counts as written.
- [x] AC-08 (error): If a pre-migration backup cannot be written, then sofarr does not run the migrations and stops with a German error message (exit code ≠ 0).
- [x] AC-09 (event): When the user lists the backups through the API, then sofarr answers with every backup in the backup folder – name, kind, size, backup time, and whether it is a valid backup – newest first, and the pending restore if there is one; files that are not sofarr databases are not listed; if the backup folder cannot be read, sofarr answers status 503 with a German message.
- [x] AC-10 (event): When the user downloads a backup through the API, then sofarr sends the file unchanged; when the user deletes one, it is removed from the folder, and if it was the pending restore, the pending restore is cleared.
- [x] AC-11 (error): If an API call names a backup that does not exist or a name outside the backup folder (e.g. with path separators or `..`), then sofarr answers status 404 and touches no file.
- [x] AC-12 (event): When the user chooses a valid backup for restore through the API, then sofarr records it as the pending restore, replacing an earlier one, and answers that it will be applied at the next start; the user can cancel the pending restore; if the pending restore cannot be recorded or cancelled, sofarr answers status 500 with a German message and the previous state stays.
- [x] AC-13 (event): When sofarr starts with a pending restore that is still a valid backup, then it keeps the current database (with its journal files) under a name that marks it as replaced, puts the backup in place as the database without any journal files of the old one, clears the pending restore, runs pending migrations on it (without a pre-migration backup: the chosen backup file is that snapshot) and starts normally.
- [x] AC-14 (error): If a backup chosen for restore is not valid, then sofarr does not record it (API: status 422 with a German message); if the pending restore is no longer valid or no longer exists at start, or the record of it cannot be read, then sofarr keeps the current database, clears the pending restore, logs the reason and starts normally.
- [x] AC-15 (error): If putting a backup in place fails at any point (e.g. disk full while copying), or the migrations fail on the restored database, then the database sofarr had before is in place again and unchanged, the pending restore is cleared, the reason is logged, and sofarr starts with the previous database.
- [x] AC-16 (event): When the user runs the CLI restore with a valid backup while no server is running, then sofarr keeps the current database under a replaced name and puts the backup in place without running migrations (the next start runs them per AC-04); if putting it in place fails, the previous database is in place again and unchanged and the CLI exits with a German message (exit code ≠ 0).
- [x] AC-17 (error): If the CLI restore runs while a server is running, names the current database itself, or names a file that is not a valid backup, then it changes nothing and exits with a German message saying why (exit code ≠ 0).
- [x] AC-18 (option): Where `[backup] interval_hours = 0` is set, sofarr writes no scheduled backups; manual and pre-migration backups still work.
- [x] AC-19 (error): If `[backup] keep` is below 1 or `interval_hours` is negative or not a whole number, then sofarr does not start and names the key in a German error message.
- [x] AC-20 (always): Within one sofarr process, backup operations (writing, pruning, deleting, choosing a restore) run one at a time; a second request waits for the first and both succeed or fail on their own. A CLI backup next to a running server produces a separate backup with its own name; neither disturbs the other.
- [x] AC-21 (always): No backup contains the content of `sofarr.toml`.

## Technical notes

- Config: `[backup] dir` (default `/config/backups`), `interval_hours` (default 24, 0 = off),
  `keep` (default 7, at least 1, scheduled backups only).
- File names carry kind and UTC time, e.g. `sofarr-2026-11-12T030000Z.db`,
  `sofarr-manual-2026-11-12T081500Z.db`, `sofarr-pre-migrate-0004-2026-11-12T080000Z.db`;
  two backups in the same second get a suffix `-2`, `-3`, …; a backup file is created exclusively
  (failing if the name exists, then trying the next suffix), which keeps names unique across processes.
- SQLite `VACUUM INTO` gives a consistent copy of a live database. It writes to `<name>.tmp` in
  the backup folder and renames when complete; `.tmp` files are never listed, and a start removes
  those older than one day (a younger one may belong to a CLI backup running next to the server).
- A running server holds an exclusive lock on a lock file next to the database
  (`sofarr.db.lock`); the CLI restore fails when it cannot take that lock.
- The pending restore is a small marker file next to the database (`sofarr.db.restore` with the
  backup name), so it survives a restart. A marker naming a path outside the backup folder is
  treated as unreadable (AC-14).
- Restore order: copy the backup to a temporary file next to the database, check it, rename the
  current database to `sofarr.db.replaced-<time>`, rename the copy into place; on any failure
  rename back. The replaced database is never deleted automatically.
- `POST /api/backups` answers once the backup is written; there is no timeout on sofarr's side.
- SQLite journal files (`-wal`, `-shm`) belong to the database they sit next to: moved aside with
  the replaced database, never carried over to the restored one.
- API: `GET /api/backups`, `POST /api/backups`, `GET /api/backups/{name}`,
  `DELETE /api/backups/{name}`, `POST /api/backups/{name}/restore`, `DELETE /api/backups/restore`.
- CLI: `amigo-sofarr backup`, `amigo-sofarr restore <name-or-path>`.

## Edge cases

| Scenario | Expected behavior |
| -------- | ----------------- |
| sofarr was down longer than the interval | One scheduled backup within 5 minutes after start, not one per missed interval (AC-01) |
| Two manual backups started within the same second | Both are kept under distinct names |
| A scheduled backup is due while an RSS sync writes grabs | The backup is consistent (AC-02); API requests are still answered |
| The pending backup was deleted through the API | The pending restore is cleared (AC-10) |
| The pending backup was deleted by hand before the restart | Nothing is restored, the reason is logged (AC-14) |
| A backup from an older sofarr version is restored | Its pending migrations run after the restore (AC-13) |
| Migrations fail on a restored older backup | The previous database is back in place, sofarr starts with it (AC-15) |
| The backup folder is on a different file system than the database | Works; the restore copies the file |
| A hand-copied file named like a scheduled backup | Counted as scheduled by its name and pruned like one |
| A crash leaves a `.tmp` file in the backup folder | Not listed, removed at a start once older than one day |
| A backup is downloaded while it gets pruned | The download may end early with an error; the pruning is not delayed |
| A database of a newer sofarr version lies in the folder | Listed as not valid; choosing it for restore answers 422 (AC-14) |
| The newest scheduled backup carries a time in the future (clock change) | A scheduled backup is due (AC-01) |
| Several hundred files in the backup folder | Listing stays complete; only scheduled and pre-migration backups are pruned |

## Assumptions

| Assumption | Reason | Risk |
| ---------- | ------ | ---- |
| Manual backups are never pruned | The user made them on purpose | The folder grows if the user never deletes them |
| Three pre-migration backups are kept | One per recent update is enough to step back; migrations are rare | A chain of four migrating updates loses the oldest snapshot |
| A replaced database is kept and never deleted automatically | A restore by mistake must be undoable | Disk use until the user deletes it |
| The database holds no secrets | Indexer and download-client credentials live only in `sofarr.toml`; the imported documents are profiles, formats, definitions, media-management, naming and download settings, tags and import lists; the owner's import lists are TMDb person lists, which carry no token | An import list type with a token (e.g. Trakt) imported later would put that token into backups |

## Open questions

<!-- One bullet per open question; empty means: everything settled, spec-build may start -->

## Evidence

| AC    | Test | Status | Commit |
| ----- | ---- | ------ | ------ |
| AC-01 | `backup_tests::a_scheduled_backup_is_due_once_per_interval`, `backup_tests::a_backup_runs_while_the_database_answers` (`crates/sofarr-server/src/backup_tests.rs`); `backup_tests::the_scheduler_writes_a_backup_soon_after_start`, `backup_tests::api_requests_are_answered_while_a_backup_runs` (`crates/sofarr-server/src/backup_tests.rs`); `backup_tests::an_empty_placeholder_is_no_scheduled_backup` (`crates/sofarr-server/src/backup_tests.rs`) | verified | ee9d25f |
| AC-02 | `backup_tests::a_backup_is_valid_and_consistent` (`crates/sofarr-server/src/backup_tests.rs`); `files::tests::copies_are_valid_sofarr_databases` (`crates/sofarr-db/src/files.rs`) | verified | 92fd11f |
| AC-03 | `backup_tests::scheduled_backups_beyond_keep_are_pruned` (`crates/sofarr-server/src/backup_tests.rs`) | verified | ee9d25f |
| AC-04 | `backup_tests::a_start_with_pending_migrations_writes_a_pre_migration_backup` (`crates/sofarr-server/src/backup_tests.rs`); `backup_tests::pre_migration_backups_keep_the_three_newest_by_time` (`crates/sofarr-server/src/backup_tests.rs`) | verified | ee9d25f |
| AC-05 | `backup_tests::a_start_without_pending_migrations_writes_none` (`crates/sofarr-server/src/backup_tests.rs`) | verified | ee9d25f |
| AC-06 | `backup_api_tests::a_manual_backup_through_the_api` (`crates/sofarr-server/src/backup_api_tests.rs`); `backup_and_restore_through_the_cli`, `a_failing_cli_backup_exits_with_a_german_message` (`crates/sofarr-server/tests/cli_backup.rs`) | verified | ee9d25f |
| AC-07 | `backup_tests::a_failed_backup_leaves_nothing_and_is_shown` (`crates/sofarr-server/src/backup_tests.rs`) | verified | ee9d25f |
| AC-08 | `backup_tests::a_failed_pre_migration_backup_stops_before_the_migrations` (`crates/sofarr-server/src/backup_tests.rs`); `a_failed_pre_migration_backup_stops_the_start` (`crates/sofarr-server/tests/cli_backup.rs`) | verified | ee9d25f |
| AC-09 | `backup_api_tests::the_backup_list` (`crates/sofarr-server/src/backup_api_tests.rs`); `files::tests::tells_foreign_newer_and_broken_files_apart` (`crates/sofarr-db/src/files.rs`) | verified | 92fd11f |
| AC-10 | `backup_api_tests::download_and_delete_a_backup` (`crates/sofarr-server/src/backup_api_tests.rs`) | verified | ee9d25f |
| AC-11 | `backup_api_tests::names_outside_the_folder_or_unknown_are_not_found` (`crates/sofarr-server/src/backup_api_tests.rs`) | verified | ee9d25f |
| AC-12 | `backup_api_tests::choosing_and_cancelling_a_restore` (`crates/sofarr-server/src/backup_api_tests.rs`); `backup_api_tests::a_failed_cancel_keeps_the_pending_restore` (`crates/sofarr-server/src/backup_api_tests.rs`) | verified | ee9d25f |
| AC-13 | `backup_api_tests::a_pending_restore_is_applied_at_start`, `backup_api_tests::an_older_backup_is_migrated_after_the_restore` (`crates/sofarr-server/src/backup_api_tests.rs`); `backup_tests::journal_files_go_aside_with_the_replaced_database` (`crates/sofarr-server/src/backup_tests.rs`); `backup_api_tests::a_restore_is_applied_only_once` (`crates/sofarr-server/src/backup_api_tests.rs`, Windows) | verified | ee9d25f |
| AC-14 | `backup_api_tests::an_unusable_pending_restore_is_dropped` (`crates/sofarr-server/src/backup_api_tests.rs`) | verified | ee9d25f |
| AC-15 | `backup_api_tests::a_failed_restore_leaves_the_previous_database` (`crates/sofarr-server/src/backup_api_tests.rs`) | verified | ee9d25f |
| AC-16 | `backup_and_restore_through_the_cli` (`crates/sofarr-server/tests/cli_backup.rs`); `a_cli_restore_moves_the_journal_files_aside` (`crates/sofarr-server/tests/cli_backup.rs`) | verified | ee9d25f |
| AC-17 | `backup_and_restore_through_the_cli` (`crates/sofarr-server/tests/cli_backup.rs`) | verified | ee9d25f |
| AC-18 | `backup_tests::interval_zero_writes_no_scheduled_backups` (`crates/sofarr-server/src/backup_tests.rs`) | verified | ee9d25f |
| AC-19 | `config::tests::backup_settings_are_checked` (`crates/sofarr-server/src/config.rs`) | verified | ee9d25f |
| AC-20 | `backup_api_tests::concurrent_backups_get_their_own_names` (`crates/sofarr-server/src/backup_api_tests.rs`); `backup_tests::backup_operations_wait_for_each_other` (`crates/sofarr-server/src/backup_tests.rs`); `cli_backups_next_to_a_running_server` (`crates/sofarr-server/tests/cli_backup.rs`) | verified | ee9d25f |
| AC-21 | `backup_tests::no_backup_contains_the_config` (`crates/sofarr-server/src/backup_tests.rs`) | verified | ee9d25f |
| AC-22 | `backup_api_tests::a_failed_delete_keeps_the_backup_and_is_shown` (`crates/sofarr-server/src/backup_api_tests.rs`); `backup_tests::a_failed_pruning_keeps_the_backup_and_is_shown` (`crates/sofarr-server/src/backup_tests.rs`, Windows); `backup_tests::a_failed_pre_migration_pruning_is_shown` (`crates/sofarr-server/src/backup_tests.rs`, Windows) | verified | ee9d25f |

## Decisions

- 2026-11-12: A restore chosen through the API is applied at the next start; the CLI restores directly while the server is stopped (owner).
- 2026-11-12: The backup folder is configurable, default `/config/backups` (owner).
- 2026-11-12: A failed restore always leaves the previous database in place, so a restore can never end without a database.
- 2026-11-12: Tests and code were written together (the tests need the new APIs); the red run was the failing build plus a first run with eight failures (wrong time parsing of backup names, an open pool blocking the swap under Windows). As a counter-check the core behavior was switched off one piece at a time (no pruning, no pre-migration backup, no restore at start, no cleanup after a failed write, no validity check, error never cleared): eight tests went red.
- 2026-11-12: A backup name is reserved by creating the file exclusively (`create_new`), `VACUUM INTO` writes `<name>.tmp`, a rename replaces the placeholder; a failure removes both. While a backup is written its placeholder is an empty file, which is no sofarr database and is not listed.
- 2026-11-12: A missing backup folder lists as empty (it appears with the first backup); only a folder that exists but cannot be read answers 503.
- 2026-11-12: Only files that are sofarr databases count as backups for download, delete and restore; any other name answers 404, so the API never touches other files in the folder.
- 2026-11-12: A download reads the whole backup into memory: streaming would need `tokio-util` as a new dependency, and a backup is a few megabytes.
- 2026-11-12: Pruning keeps `keep` backups of the kind including a protected pending restore; the next one in line is deleted instead of the pending restore.
- 2026-11-12: The server lock uses `std::fs::File::try_lock` (Rust 1.89+), no new dependency. A second `serve` on the same database stops with a German message.
- 2026-11-12: `sofarr_db::create_at_version` builds a migrator from the first migrations (fields of sqlx's `Migrator` that are public but not covered by semver; `--locked` pins sqlx 0.8.6) to create databases of an older schema for tests; a checked-in fixture would break on the CRLF/LF checksums of the migrations between Windows and CI.
- 2026-11-12: `sofarr-server` gets `sqlx` (already a workspace dependency) as a dev-dependency, to craft broken and newer-version databases in tests.
- 2026-11-12: The pruning failure test runs under Windows only (an open file cannot be deleted there); under unix a read-only folder would also stop the new backup. The delete failure test runs on both.
- 2026-11-12: After the first verification round: the scheduler is started by `backups::spawn_schedule` (tested), the due check reads only the names of the scheduled backups (no file is opened every minute), a pruning failure at start is handed to the app and shown in the list, the list skips entries that vanish while it is read, and only sofarr's own `sofarr-*.db.tmp` files are cleaned up at start. All new tests went red with the behavior switched off.
- 2026-11-12: After the second verification round: the pending restore marker is removed before the backup is put in place (if it cannot be removed, nothing is restored), so a restart never restores the same backup again over newer changes; pre-migration backups keep the three newest by time without the future-first rule of scheduled ones (AC-04 says "by backup time").
- 2026-11-12: After the third verification round: the due check skips empty files (the placeholder of a backup being written or one that crashed) — still without opening a file — and a start removes such placeholders older than a day like the `.tmp` files; the backup time sofarr reports is the whole second of the name; a CLI restore clears a restore chosen through the API before, which would otherwise undo it at the next start.
- 2026-11-12: Timestamps in JSON answers are RFC 3339 text (`serialize_with`), without enabling jiff's `serde` feature.
