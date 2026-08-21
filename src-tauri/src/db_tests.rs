// ============================================
// TaskFlow — tests for backup rotation & throttling
// ============================================
// The backup ring is the only thing standing between a corrupted
// trello_clone.db and total data loss, so its two rules — "keep the newest
// BACKUP_KEEP" and "no more than one per BACKUP_MIN_INTERVAL_SECS" — are
// verified rather than assumed.

use super::*;
use std::time::{Duration, SystemTime};

/// Unique temp directory for one test, removed if it already exists.
fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("taskflow-test-{}", name));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Creates a backup file and back-dates it by `age`, since modification time
/// is what `backup_is_recent` reads. Uses std's `File::set_modified` rather
/// than pulling in the `filetime` crate for one line.
fn make_backup(dir: &Path, name: &str, age: Duration) {
    let path = dir.join(name);
    fs::write(&path, b"x").unwrap();

    let file = fs::File::options().write(true).open(&path).unwrap();
    file.set_modified(SystemTime::now() - age).unwrap();
}

#[test]
fn no_backups_yet_is_not_recent() {
    let dir = temp_dir("empty");
    assert!(!backup_is_recent(&dir), "в пустой папке нечего считать свежим");
}

#[test]
fn missing_directory_is_not_recent() {
    let dir = std::env::temp_dir().join("taskflow-test-does-not-exist");
    let _ = fs::remove_dir_all(&dir);
    assert!(!backup_is_recent(&dir));
}

#[test]
fn a_backup_from_minutes_ago_blocks_another_one() {
    let dir = temp_dir("recent");
    make_backup(&dir, "backup-20260817-120000.db", Duration::from_secs(26 * 60));
    assert!(
        backup_is_recent(&dir),
        "копия 26-минутной давности должна блокировать новую при интервале в час"
    );
}

#[test]
fn a_backup_older_than_the_interval_allows_another_one() {
    let dir = temp_dir("old");
    make_backup(&dir, "backup-20260817-000000.db", Duration::from_secs(BACKUP_MIN_INTERVAL_SECS + 60));
    assert!(!backup_is_recent(&dir), "копия старше интервала не должна блокировать новую");
}

#[test]
fn the_newest_backup_decides_not_the_oldest() {
    let dir = temp_dir("mixed");
    // An old copy plus a fresh one: the fresh one has to win.
    make_backup(&dir, "backup-20260801-000000.db", Duration::from_secs(10 * 24 * 60 * 60));
    make_backup(&dir, "backup-20260817-120000.db", Duration::from_secs(5 * 60));
    assert!(backup_is_recent(&dir), "решать должна самая свежая копия, а не самая старая");
}

#[test]
fn unrelated_files_are_ignored() {
    let dir = temp_dir("unrelated");
    // A fresh file that is not one of our backups must not block anything.
    fs::write(dir.join("trello_clone.db"), b"x").unwrap();
    fs::write(dir.join("readme.txt"), b"x").unwrap();
    assert!(!backup_is_recent(&dir), "посторонние файлы не должны считаться копиями");
}

#[test]
fn pruning_keeps_the_newest_and_only_our_files() {
    let dir = temp_dir("prune");

    // 13 backups, named so that lexicographic order matches chronological.
    for i in 0..13 {
        make_backup(&dir, &format!("backup-20260817-{:06}.db", i), Duration::from_secs(0));
    }
    // Something that is not a backup must survive the prune untouched.
    fs::write(dir.join("trello_clone.db"), b"x").unwrap();

    prune_backups(&dir);

    let mut remaining: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("backup-"))
        .collect();
    remaining.sort();

    assert_eq!(remaining.len(), BACKUP_KEEP, "должно остаться ровно BACKUP_KEEP копий");
    // 13 created, 10 kept → the three oldest (000000..000002) are gone.
    assert_eq!(remaining[0], "backup-20260817-000003.db");
    assert_eq!(remaining[BACKUP_KEEP - 1], "backup-20260817-000012.db");
    assert!(dir.join("trello_clone.db").exists(), "ротация не должна трогать саму базу");
}

#[test]
fn pruning_below_the_limit_deletes_nothing() {
    let dir = temp_dir("prune-few");
    for i in 0..3 {
        make_backup(&dir, &format!("backup-20260817-{:06}.db", i), Duration::from_secs(0));
    }
    prune_backups(&dir);
    let count = fs::read_dir(&dir).unwrap().count();
    assert_eq!(count, 3);
}

// ─── Migration rehearsal against a copy of a real database ───
//
// Ignored by default: it needs a database that only exists on a machine the app
// has actually been used on. Run it before letting a schema change touch live
// data:
//
//   TASKFLOW_REAL_DB=/path/to/a/COPY/of/trello_clone.db \
//       cargo test --lib rehearse_migration -- --ignored --nocapture
//
// Always point it at a copy. It opens the file for writing.

#[test]
#[ignore]
fn rehearse_migration_on_a_real_database() {
    let Ok(path) = std::env::var("TASKFLOW_REAL_DB") else {
        panic!("укажите TASKFLOW_REAL_DB — путь к КОПИИ реальной базы");
    };

    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();

    let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };

    let cards_before = count("SELECT COUNT(*) FROM cards");
    let boards_before = count("SELECT COUNT(*) FROM boards");
    let columns_before = count("SELECT COUNT(*) FROM columns");
    let labels_before = count("SELECT COUNT(*) FROM labels");
    let (name_before, initials_before): (String, String) = conn
        .query_row(
            "SELECT display_name, avatar_initials FROM user_profile WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    println!(
        "ДО:    доски {}, колонки {}, карточки {}, метки {}, профиль «{}» ({})",
        boards_before, columns_before, cards_before, labels_before, name_before, initials_before
    );

    super::create_schema(&conn).expect("миграция должна пройти на реальной базе");
    // Twice, because the app runs it on every start.
    super::create_schema(&conn).expect("повторный запуск миграции");

    let cards_after = count("SELECT COUNT(*) FROM cards");
    let members = count("SELECT COUNT(*) FROM members");
    let no_author = count("SELECT COUNT(*) FROM cards WHERE author_id IS NULL");
    let bad_priority = count("SELECT COUNT(*) FROM cards WHERE priority NOT IN ('Low','Medium','High')");
    let violations = count("SELECT COUNT(*) FROM pragma_foreign_key_check");
    let (self_name, self_initials): (String, String) = conn
        .query_row("SELECT name, initials FROM members WHERE is_self = 1", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();

    println!(
        "ПОСЛЕ: доски {}, колонки {}, карточки {}, метки {}, участников {}, «я» — «{}» ({})",
        count("SELECT COUNT(*) FROM boards"),
        count("SELECT COUNT(*) FROM columns"),
        cards_after,
        count("SELECT COUNT(*) FROM labels"),
        members,
        self_name,
        self_initials
    );
    println!(
        "без автора {}, недопустимый приоритет {}, нарушений внешних ключей {}",
        no_author, bad_priority, violations
    );

    assert_eq!(cards_before, cards_after, "миграция не должна терять карточки");
    assert_eq!(boards_before, count("SELECT COUNT(*) FROM boards"), "миграция не должна терять доски");
    assert_eq!(columns_before, count("SELECT COUNT(*) FROM columns"), "миграция не должна терять колонки");
    assert_eq!(labels_before, count("SELECT COUNT(*) FROM labels"), "миграция не должна терять метки");
    assert_eq!(members, 1, "должен появиться ровно один участник — сам пользователь");
    assert_eq!(self_name, name_before, "имя из профиля должно перенестись дословно");
    assert_eq!(self_initials, initials_before);
    assert_eq!(no_author, 0, "у всех существующих карточек должен появиться автор");
    assert_eq!(bad_priority, 0);
    assert_eq!(violations, 0, "внешние ключи должны остаться согласованными");
}
