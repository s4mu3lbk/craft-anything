use craft_anything_formats::sqlite::{CellValue, SqliteDb, parse_create_columns};
use craft_anything_testkit::sqlite::{Val, table_db, without_rowid_db};

fn text(value: &str) -> CellValue {
    CellValue::Text { text: value.into(), truncated: false }
}

/// Inside a quoted name a doubled quote stands for one quote, and SQLite also takes a
/// single-quoted name for a column. The name ends at the real closing quote.
#[test]
fn quoted_column_names_are_read_whole() {
    let sql = "CREATE TABLE t (\"we\"\"ird\" TEXT, `back``tick` INT, 'single quoted' REAL, 'it''s' BLOB, [plain] NUMERIC)";
    let columns = parse_create_columns(sql);
    let names: Vec<_> = columns.iter().map(|c| c.name.as_str()).collect();
    let types: Vec<_> = columns.iter().map(|c| c.type_name.as_str()).collect();
    assert_eq!(names, ["we\"ird", "back`tick", "single quoted", "it's", "plain"]);
    assert_eq!(types, ["TEXT", "INT", "REAL", "BLOB", "NUMERIC"]);
}

/// An empty quoted name is still a column (SQLite names it ""), so later columns keep their place.
#[test]
fn empty_quoted_names_are_columns() {
    for quoted in ["\"\"", "``", "''", "[]"] {
        let sql = format!("CREATE TABLE t ({quoted} INTEGER PRIMARY KEY, b TEXT)");
        let names: Vec<_> = parse_create_columns(&sql).into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["", "b"], "{sql}");
        let data = table_db(&sql, &[(7, vec![Val::Null, Val::Text("x".into())])]);
        let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
        assert_eq!(page.rows[0].values, vec![CellValue::Int(7), text("x")], "{sql}");
    }
}

/// A table-level key that names such a column still finds it, so its rowid alias is filled.
#[test]
fn a_key_on_a_quoted_name_with_a_doubled_quote_keeps_the_rowid_alias() {
    let data = table_db("CREATE TABLE t (v TEXT, \"i\"\"d\" INTEGER, PRIMARY KEY (\"i\"\"d\"))", &[(9, vec![Val::Text("a".into()), Val::Null])]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![text("a"), CellValue::Int(9)]);
}

/// A WITHOUT ROWID key on single-quoted names is found too, so rows come back in declaration order.
#[test]
fn a_without_rowid_key_on_single_quoted_names_is_found() {
    let sql = "CREATE TABLE t ('the value' INT, 'the key' TEXT, PRIMARY KEY ('the key')) WITHOUT ROWID";
    let data = without_rowid_db(sql, &[vec![Val::Text("x".into()), Val::Int(1)]]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![CellValue::Int(1), text("x")]);
    assert!(page.notes.is_empty(), "{:?}", page.notes);
}
