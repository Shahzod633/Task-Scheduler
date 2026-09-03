// ============================================
// TaskFlow — тесты шифрования базы
// ============================================
// Здесь нельзя обойтись базой в памяти: всё, что проверяется, — про файл на
// диске. Признак незашифрованного файла читается по байтам заголовка, а
// миграция переименовывает файлы.
//
// Диспетчер учётных данных тесты не трогают: `load_or_create_key` завязан на
// учётную запись Windows, и тест, который туда пишет, менял бы настоящий ключ
// пользователя. Проверяется всё остальное — то, что решает судьбу данных.

use super::*;
use rusqlite::params;

/// Отдельная папка на каждый тест, чтобы они не спорили за одни и те же файлы.
fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("taskflow-crypto-test-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn test_key() -> DbKey {
    generate_key().unwrap()
}

/// Незашифрованная база с одной таблицей и одной строкой.
fn make_plaintext_db(path: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "CREATE TABLE cards (id INTEGER PRIMARY KEY, title TEXT);
         INSERT INTO cards (title) VALUES ('Не потеряй меня');",
    )
    .unwrap();
}

#[test]
fn a_generated_key_is_256_bits_of_hex_and_never_repeats() {
    let a = generate_key().unwrap();
    let b = generate_key().unwrap();

    assert_eq!(a.0.len(), 64, "32 байта в шестнадцатеричной записи");
    assert!(is_valid_hex_key(&a.0));
    assert_ne!(a.0, b.0, "два ключа подряд не должны совпасть");
}

#[test]
fn only_a_well_formed_key_is_accepted() {
    assert!(is_valid_hex_key(&"a1".repeat(32)));

    // Всё это подставлялось бы в текст SQL, поэтому проверка строгая.
    assert!(!is_valid_hex_key(""), "пустая строка");
    assert!(!is_valid_hex_key(&"a".repeat(63)), "короче 64");
    assert!(!is_valid_hex_key(&"a".repeat(65)), "длиннее 64");
    assert!(!is_valid_hex_key(&"A".repeat(64)), "верхний регистр мы не пишем");
    assert!(!is_valid_hex_key(&"g".repeat(64)), "не шестнадцатеричный символ");
    assert!(!is_valid_hex_key(&format!("{}'; DROP TABLE cards; --", "a".repeat(40))));
}

#[test]
fn a_plain_sqlite_file_is_recognised_and_an_encrypted_one_is_not() {
    let dir = temp_dir("detect");
    let plain = dir.join("plain.db");
    make_plaintext_db(&plain);
    assert!(is_plaintext_database(&plain), "обычная база SQLite");

    // Отсутствующий файл — это первый запуск, а не незашифрованная база.
    assert!(!is_plaintext_database(&dir.join("нет-такого.db")));

    let key = test_key();
    encrypt_existing_database(&plain, &key).unwrap();
    assert!(
        !is_plaintext_database(&plain),
        "после миграции заголовок SQLite должен исчезнуть"
    );
}

#[test]
fn encrypting_keeps_the_data_and_leaves_the_original_alone() {
    let dir = temp_dir("migrate");
    let db = dir.join("trello_clone.db");
    make_plaintext_db(&db);

    let key = test_key();
    encrypt_existing_database(&db, &key).unwrap();

    // Данные на месте и читаются ключом.
    let conn = open_encrypted(&db, &key).unwrap();
    let title: String = conn
        .query_row("SELECT title FROM cards", [], |r| r.get(0))
        .unwrap();
    assert_eq!(title, "Не потеряй меня");

    // Исходник не удалён — именно он спасает, если ключ потеряется.
    let backup = find_plaintext_backup(&dir).expect("старая база обязана остаться на диске");
    assert!(is_plaintext_database(&backup), "и остаться читаемой");

    // Промежуточный файл убран за собой.
    assert!(!dir.join("trello_clone.db.encrypting").exists());
}

#[test]
fn a_wrong_key_opens_nothing_rather_than_silently_making_an_empty_database() {
    let dir = temp_dir("wrongkey");
    let db = dir.join("db.db");
    make_plaintext_db(&db);

    let right = test_key();
    encrypt_existing_database(&db, &right).unwrap();

    let wrong = test_key();
    assert!(
        open_encrypted(&db, &wrong).is_err(),
        "неверный ключ обязан дать ошибку здесь, а не «file is not a database» посреди работы"
    );

    // И база при этом не испорчена — правильный ключ по-прежнему открывает её.
    assert!(open_encrypted(&db, &right).is_ok());
}

#[test]
fn restoring_a_plaintext_copy_over_the_database_migrates_it_again() {
    let dir = temp_dir("restore");
    let db = dir.join("db.db");
    make_plaintext_db(&db);

    let key = test_key();
    encrypt_existing_database(&db, &key).unwrap();

    // Человек восстанавливается из расшифрованного экспорта: кладёт его на
    // место базы. Копирование файла поверх существующего заменяет его целиком,
    // поэтому зашифрованный сначала исчезает. Приложение обязано это пережить —
    // отказ мигрировать означал бы, что оно не запускается ровно тогда, когда
    // человек спасает данные.
    std::fs::remove_file(&db).unwrap();
    make_plaintext_db(&db);
    encrypt_existing_database(&db, &key)
        .expect("вторая миграция должна пройти, а не упереться в старый .bak");

    assert!(!is_plaintext_database(&db), "база снова зашифрована");
    assert!(open_encrypted(&db, &key).is_ok());
}

/// Единственный `.plaintext-*.bak` в папке.
fn find_plaintext_backup(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.to_string_lossy().contains(BACKUP_PREFIX) && p.extension().is_some_and(|e| e == "bak")
        })
}

#[test]
fn an_encrypted_database_survives_vacuum_into_which_is_how_backups_are_written() {
    let dir = temp_dir("backup");
    let db = dir.join("db.db");
    make_plaintext_db(&db);

    let key = test_key();
    encrypt_existing_database(&db, &key).unwrap();
    let conn = open_encrypted(&db, &key).unwrap();

    // Ровно то, что делает `write_backup` после перехода с онлайн-API бэкапа.
    let copy = dir.join("backup-copy.db");
    conn.execute("VACUUM INTO ?1", params![copy.to_string_lossy()])
        .unwrap();

    assert!(!is_plaintext_database(&copy), "копия обязана быть зашифрована");

    let restored = open_encrypted(&copy, &key).unwrap();
    let title: String = restored
        .query_row("SELECT title FROM cards", [], |r| r.get(0))
        .unwrap();
    assert_eq!(title, "Не потеряй меня", "копия должна открываться тем же ключом");
}

#[test]
fn a_plaintext_copy_can_be_taken_out_of_an_encrypted_database() {
    let dir = temp_dir("export");
    let db = dir.join("db.db");
    make_plaintext_db(&db);

    let key = test_key();
    encrypt_existing_database(&db, &key).unwrap();
    let conn = open_encrypted(&db, &key).unwrap();

    // Ровно то, что делает `export_database_to`. Проверяется именно с
    // зашифрованного источника: тест в `commands_tests` берёт базу в памяти,
    // то есть незашифрованную, и настоящий путь экспорта не трогает.
    let out = dir.join("export.db");
    conn.execute("ATTACH DATABASE ?1 AS plaintext KEY ''", params![out.to_string_lossy()])
        .unwrap();
    conn.query_row("SELECT sqlcipher_export('plaintext')", [], |_| Ok(())).unwrap();
    conn.execute_batch("DETACH DATABASE plaintext").unwrap();

    assert!(is_plaintext_database(&out), "экспорт обязан быть обычной базой SQLite");

    // И открываться без ключа — в этом весь смысл.
    let plain = Connection::open(&out).unwrap();
    let title: String = plain.query_row("SELECT title FROM cards", [], |r| r.get(0)).unwrap();
    assert_eq!(title, "Не потеряй меня");
}
