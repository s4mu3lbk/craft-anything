use craft_anything_formats::sqlite::{CellValue, SqliteDb};
use craft_anything_testkit::sqlite::{Val, without_rowid_db};

fn text(value: &str) -> CellValue {
    CellValue::Text { text: value.into(), truncated: false }
}

/// Rows stored before `ALTER TABLE ADD COLUMN` end early. In a WITHOUT ROWID table the missing
/// fields take the added columns' defaults, as they do in a rowid table.
#[test]
fn older_without_rowid_rows_take_the_added_columns_defaults() {
    let sql = "CREATE TABLE t (k TEXT PRIMARY KEY, v INT, w INT DEFAULT 5, s TEXT DEFAULT 'z') WITHOUT ROWID";
    let rows = [
        vec![Val::Text("a".into()), Val::Int(1)],
        vec![Val::Text("b".into()), Val::Int(2)],
        vec![Val::Text("c".into()), Val::Int(3), Val::Int(6), Val::Text("y".into())],
    ];
    let data = without_rowid_db(sql, &rows);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    let values: Vec<_> = page.rows.iter().map(|row| row.values.clone()).collect();
    assert_eq!(
        values,
        vec![
            vec![text("a"), CellValue::Int(1), CellValue::Int(5), text("z")],
            vec![text("b"), CellValue::Int(2), CellValue::Int(5), text("z")],
            vec![text("c"), CellValue::Int(3), CellValue::Int(6), text("y")],
        ]
    );
    assert!(page.notes.is_empty(), "{:?}", page.notes);
}

/// The key may be declared after other columns: the record still stores it first, and only the
/// added columns are missing from older rows.
#[test]
fn a_key_declared_last_still_leaves_only_added_columns_to_defaults() {
    let sql = "CREATE TABLE t (v INT, k TEXT, w TEXT DEFAULT 'new', PRIMARY KEY (k)) WITHOUT ROWID";
    let data = without_rowid_db(sql, &[vec![Val::Text("a".into()), Val::Int(1)]]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![CellValue::Int(1), text("a"), text("new")]);
}

/// A record that ends before a column declared ahead of the key still keeps the key it stores.
#[test]
fn a_short_record_keeps_every_stored_field() {
    let sql = "CREATE TABLE t (v INT, k TEXT, PRIMARY KEY (k)) WITHOUT ROWID";
    let data = without_rowid_db(sql, &[vec![Val::Text("a".into())]]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![CellValue::Null, text("a")]);
}

/// A VIRTUAL column added later stores no field and is shown as not read; a column added after it
/// still takes its default.
#[test]
fn an_added_virtual_column_is_not_read_and_later_defaults_apply() {
    let sql = "CREATE TABLE t (k TEXT PRIMARY KEY, v INT, g INT AS (v * 2), w INT DEFAULT 5) WITHOUT ROWID";
    let data = without_rowid_db(sql, &[vec![Val::Text("a".into()), Val::Int(1)]]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![text("a"), CellValue::Int(1), CellValue::NotRead, CellValue::Int(5)]);
}

/// A default that isn't a plain literal still becomes NULL, and the note says so.
#[test]
fn an_expression_default_is_disclosed_for_without_rowid_rows_too() {
    let sql = "CREATE TABLE t (k TEXT PRIMARY KEY, w INT DEFAULT (random())) WITHOUT ROWID";
    let data = without_rowid_db(sql, &[vec![Val::Text("a".into())]]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![text("a"), CellValue::Null]);
    assert!(page.notes.iter().any(|note| note.contains("Column 2") && note.contains("DEFAULT")), "{:?}", page.notes);
}
