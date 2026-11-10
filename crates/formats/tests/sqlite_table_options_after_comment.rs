use craft_anything_formats::sqlite::{CellValue, SqliteDb};
use craft_anything_testkit::sqlite::{Val, without_rowid_db};

fn text(value: &str) -> CellValue {
    CellValue::Text { text: value.into(), truncated: false }
}

fn rows(sql: &str, stored: &[Vec<Val>]) -> Vec<(Option<i64>, Vec<CellValue>)> {
    let data = without_rowid_db(sql, stored);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    page.rows.into_iter().map(|row| (row.rowid, row.values)).collect()
}

/// SQLite keeps a comment after the column list in the stored statement, so a `)` inside it
/// must not be taken for the end of the list. Each statement is what SQLite stored for it.
#[test]
fn a_parenthesis_in_a_comment_after_the_column_list_keeps_without_rowid() {
    let stored = [vec![Val::Text("a".into()), Val::Int(1)], vec![Val::Text("b".into()), Val::Int(2)]];
    let expected = vec![(None, vec![text("a"), CellValue::Int(1)]), (None, vec![text("b"), CellValue::Int(2)])];
    for sql in [
        "CREATE TABLE t(k TEXT PRIMARY KEY, v INT) WITHOUT ROWID /* ) */",
        "CREATE TABLE t(k TEXT PRIMARY KEY, v INT) WITHOUT /* ) */ ROWID",
        "CREATE TABLE t(k TEXT PRIMARY KEY, v INT) WITHOUT ROWID -- note (x)\n",
        "CREATE TABLE t(k TEXT PRIMARY KEY, v INT) STRICT, WITHOUT ROWID /* (a) */",
    ] {
        assert_eq!(rows(sql, &stored), expected, "{sql:?}");
    }
}

/// The key order comes from the column list itself, not from text in a later comment.
#[test]
fn a_key_declared_last_is_found_before_a_comment_with_parentheses() {
    let sql = "CREATE TABLE t(v INT, k TEXT, PRIMARY KEY (k)) WITHOUT ROWID /* stored as (k, v) */";
    assert_eq!(rows(sql, &[vec![Val::Text("a".into()), Val::Int(1)]]), vec![(None, vec![CellValue::Int(1), text("a")])]);
}

/// STRICT after a comment still makes an ANY column keep a filled-in default as written.
#[test]
fn strict_after_a_comment_with_a_parenthesis_still_counts() {
    let sql = "CREATE TABLE t(k TEXT PRIMARY KEY, v ANY DEFAULT '5') WITHOUT ROWID, /* ) */ STRICT";
    assert_eq!(rows(sql, &[vec![Val::Text("a".into())]]), vec![(None, vec![text("a"), text("5")])]);
}
