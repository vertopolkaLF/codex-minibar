use rusqlite::params;

use super::*;

fn reader() -> Reader {
    Reader::new(ProviderId::primary(ProviderKind::Antigravity), None)
}

fn varint(mut value: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    while value >= 128 {
        bytes.push(value as u8 | 128);
        value >>= 7;
    }
    bytes.push(value as u8);
    bytes
}

fn number(field: u64, value: u64) -> Vec<u8> {
    [varint(field << 3), varint(value)].concat()
}

fn message(field: u64, bytes: &[u8]) -> Vec<u8> {
    [
        varint((field << 3) | 2),
        varint(bytes.len() as u64),
        bytes.to_vec(),
    ]
    .concat()
}

fn timestamp(field: u64, at: DateTime<Local>) -> Vec<u8> {
    message(field, &number(1, at.timestamp() as u64))
}

fn usage(input: u64) -> Vec<u8> {
    [
        number(1, 1026),
        number(2, input),
        number(3, 5),
        number(4, 10),
        number(5, 20),
        number(9, 7),
        number(10, 5),
    ]
    .concat()
}

fn create_database(path: &Path) -> Connection {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let db = Connection::open(path).unwrap();
    db.execute_batch(
        "CREATE TABLE steps(idx INTEGER PRIMARY KEY, metadata BLOB);
         CREATE TABLE gen_metadata(idx INTEGER PRIMARY KEY, data BLOB);
         CREATE TABLE trajectory_metadata_blob(data BLOB);",
    )
    .unwrap();
    db
}

fn add_step(db: &Connection, idx: i64, fields: Vec<u8>) {
    db.execute("INSERT INTO steps VALUES (?1, ?2)", params![idx, fields])
        .unwrap();
}

fn add_generation(db: &Connection, idx: i64, fields: &[u8]) {
    db.execute(
        "INSERT INTO gen_metadata VALUES (?1, ?2)",
        params![idx, message(1, fields)],
    )
    .unwrap();
}

#[test]
fn pairs_mirrors_without_response_ids_and_retains_distinct_retries() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_owned();
    let db = create_database(&root.join("conversations/nested/conversation.db"));
    let now = Local::now();
    let primary = usage(100);
    let retry = [number(2, 50), number(1, 1035)].concat();
    for idx in 1..=2 {
        add_step(&db, idx, [timestamp(8, now), message(9, &primary)].concat());
        let mut generation = [
            message(4, &primary),
            message(17, &message(2, &primary)),
            message(19, b"Claude 4.6 Opus (Thinking)"),
            message(9, &timestamp(4, now)),
        ]
        .concat();
        if idx == 1 {
            generation.extend(message(17, &message(2, &retry)));
        }
        // The two tables can use different indexes for corresponding calls.
        add_generation(&db, idx + 10, &generation);
    }
    db.execute("INSERT INTO steps VALUES (3, NULL)", [])
        .unwrap();

    let scan = reader()
        .scan(vec![root.clone(), root.join("conversations")], &[], now)
        .unwrap();
    assert_eq!(scan.files.len(), 1);
    assert_eq!(scan.files[0].1.usage.requests, 3);
    let rollup = scan.rollup;
    let day = &rollup.daily[0].usage;
    assert_eq!(
        (
            day.requests,
            day.input_tokens,
            day.cached_input_tokens,
            day.output_tokens
        ),
        (3, 310, 40, 24)
    );
    assert_eq!(rollup.hourly[0].1, *day);
    assert_eq!(rollup.models.len(), 2);
    assert_eq!(rollup.models[0].0, "claude-opus-4-6-thinking");
    assert_eq!(rollup.models[0].2.requests, 2);
    assert_eq!(rollup.models[1].0, "claude-sonnet-4-6");
}

#[test]
fn merges_response_ids_across_files_and_prefers_recorded_timestamp_and_model() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_owned();
    let now = Local::now();
    let yesterday = now - chrono::Duration::days(1);
    let a = create_database(&root.join("a.db"));
    let b = create_database(&root.join("b.db"));
    let one = [usage(100), message(11, b"response")].concat();
    let link = [usage(120), message(11, b"response"), message(12, b"call")].concat();
    let two = [usage(80), message(12, b"call")].concat();
    let unrelated = [usage(120), message(11, b"unrelated")].concat();
    add_step(
        &a,
        1,
        [message(9, &one), message(9_000, b"ignored")].concat(),
    );
    add_step(&a, 2, [message(9, &link), timestamp(8, yesterday)].concat());
    add_generation(
        &b,
        1,
        &[message(4, &two), message(19, b"Gemini 3.6 Flash (High)")].concat(),
    );
    add_generation(
        &b,
        2,
        &[message(4, &unrelated), message(9, &timestamp(4, now))].concat(),
    );

    let scan = reader().scan(vec![root], &[], now).unwrap();
    assert_eq!(scan.files.len(), 2);
    assert!(scan.files[0].0.ends_with("a.db"));
    assert_eq!(scan.files[0].1.date, yesterday.date_naive());
    let rollup = scan.rollup;
    assert_eq!(rollup.daily[0].date, yesterday.date_naive());
    assert_eq!(rollup.daily[0].usage.input_tokens, 150);
    assert_eq!(rollup.daily[0].usage.requests, 1);
    assert_eq!(rollup.daily[1].usage.requests, 1);
    assert_eq!(rollup.models[0].0, "gemini-3.6-flash-high");
    assert_eq!(rate_model(&rollup.models[0].0), "gemini-3.6-flash");
}

#[test]
fn discovers_managed_profiles_and_reloads_when_the_wal_changes() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let profile = home.join(".t3/userdata/providers/antigravity/profile/antigravity-acp");
    let path = profile.join("conversations/conversation.db");
    let db = create_database(&path);
    db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA wal_autocheckpoint = 0;")
        .unwrap();
    let now = Local::now();
    let roots = data_roots(Some(home), Some("~/external, ~/other"), None).unwrap();
    assert_eq!(
        roots,
        vec![home.join("external"), home.join("other"), profile]
    );
    let mut reader = reader();
    assert!(
        reader
            .scan(roots.clone(), &[], now)
            .unwrap()
            .rollup
            .daily
            .is_empty()
    );
    let before = fs::metadata(&path).unwrap().modified().unwrap();
    db.execute(
        "INSERT INTO trajectory_metadata_blob VALUES (?1)",
        [timestamp(2, now)],
    )
    .unwrap();
    add_step(&db, 1, message(9, &usage(100)));
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    let rollup = reader.scan(roots.clone(), &[], now).unwrap().rollup;
    assert_eq!(rollup.daily[0].usage.requests, 1);
    assert_eq!(rollup.daily[0].date, now.date_naive());
    assert_eq!(rollup.models[0].0, "claude-opus-4-6");
    let excluded = roots.last().unwrap().canonicalize().unwrap();
    assert!(
        reader
            .scan(roots, &[excluded], now)
            .unwrap()
            .rollup
            .daily
            .is_empty()
    );
}

#[test]
fn keeps_the_last_successful_database_when_a_rescan_fails() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().to_owned();
    let path = root.join("conversation.db");
    let now = Local::now();
    let db = create_database(&path);
    add_step(&db, 1, message(9, &usage(100)));
    let mut cached = reader();
    assert_eq!(
        cached
            .scan(vec![root.clone()], &[], now)
            .unwrap()
            .rollup
            .daily[0]
            .usage
            .requests,
        1
    );
    add_step(&db, 2, vec![0x80]);
    assert_eq!(
        cached
            .scan(vec![root.clone()], &[], now)
            .unwrap()
            .rollup
            .daily[0]
            .usage
            .requests,
        1
    );
    assert!(reader().scan(vec![root], &[], now).is_err());
}
