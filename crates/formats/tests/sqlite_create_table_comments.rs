use craft_anything_formats::sqlite::{CellValue, SqliteDb, parse_create_columns};
use craft_anything_testkit::sqlite::{Val, table_db, without_rowid_db};

fn text(value: &str) -> CellValue {
    CellValue::Text { text: value.into(), truncated: false }
}

fn names_and_types(sql: &str) -> (Vec<String>, Vec<String>) {
    parse_create_columns(sql).into_iter().map(|c| (c.name, c.type_name)).unzip()
}

/// SQLite keeps a CREATE TABLE exactly as written, comments included. A comma or a quote inside
/// a comment doesn't end a column, and a comment in front of a column isn't its name.
#[test]
fn comments_between_columns_are_not_columns() {
    let sql =
        "CREATE TABLE t (\n  id INTEGER PRIMARY KEY, -- the id, unique\n  name TEXT,              -- don't forget\n  /* size, in bytes */ size INTEGER\n)";
    let (names, types) = names_and_types(sql);
    assert_eq!(names, ["id", "name", "size"]);
    assert_eq!(types, ["INTEGER", "TEXT", "INTEGER"]);
}

/// A comment inside a column's definition is not part of its type, so INTEGER PRIMARY KEY
/// still makes the column the rowid alias.
#[test]
fn a_comment_inside_a_definition_keeps_the_rowid_alias() {
    for sql in [
        "CREATE TABLE t (id INTEGER -- key\n PRIMARY KEY, v TEXT)",
        "CREATE TABLE t (id INTEGER/* key */PRIMARY KEY, v TEXT)",
        "CREATE TABLE t (id INTEGER--key\n PRIMARY KEY, v TEXT)",
        "CREATE TABLE t (id/* key */INTEGER PRIMARY KEY, v TEXT)",
        "CREATE TABLE t (id--key\n INTEGER PRIMARY KEY, v TEXT)",
    ] {
        assert_eq!(names_and_types(sql), (vec!["id".into(), "v".into()], vec!["INTEGER".into(), "TEXT".into()]), "{sql}");
        let data = table_db(sql, &[(3, vec![Val::Null, Val::Text("x".into())])]);
        let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
        assert_eq!(page.rows[0].values, vec![CellValue::Int(3), text("x")], "{sql}");
    }
}

/// ALTER TABLE ADD COLUMN copies the new definition, comments and all, into the CREATE TABLE.
/// Older rows then take that column's default.
#[test]
fn an_added_column_with_a_comment_takes_its_default() {
    let sql = "CREATE TABLE t (a INTEGER PRIMARY KEY, b TEXT, c TEXT /* added, later */ DEFAULT 'q')";
    assert_eq!(names_and_types(sql), (vec!["a".into(), "b".into(), "c".into()], vec!["INTEGER".into(), "TEXT".into(), "TEXT".into()]));
    let data = table_db(sql, &[(1, vec![Val::Null, Val::Text("x".into())])]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![CellValue::Int(1), text("x"), text("q")]);
}

/// Comment markers inside quotes are text.
#[test]
fn comment_markers_inside_quotes_are_not_comments() {
    let (names, _) = names_and_types("CREATE TABLE t (a TEXT DEFAULT '--', \"b/*\" TEXT, c TEXT DEFAULT '*/,', d)");
    assert_eq!(names, ["a", "b/*", "c", "d"]);
}

/// A table constraint stays a constraint when a comment follows its keyword with no space.
#[test]
fn a_constraint_keyword_followed_by_a_comment_is_not_a_column() {
    let sql = "CREATE TABLE t (k TEXT, v INT, CONSTRAINT--the key\n pk PRIMARY KEY (k)) WITHOUT ROWID";
    assert_eq!(names_and_types(sql).0, ["k", "v"]);
    let data = without_rowid_db(sql, &[vec![Val::Text("x".into()), Val::Int(1)]]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![text("x"), CellValue::Int(1)]);
}

/// A WITHOUT ROWID table's key is still found past comments, so rows come back in declaration order.
#[test]
fn without_rowid_keys_are_found_past_comments() {
    let sql = "CREATE TABLE t (\n  v INT, -- value, not the key\n  k TEXT, -- the key's text\n  PRIMARY KEY (k)\n) WITHOUT ROWID";
    let data = without_rowid_db(sql, &[vec![Val::Text("x".into()), Val::Int(1)]]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![CellValue::Int(1), text("x")]);
    assert!(page.notes.is_empty(), "{:?}", page.notes);
}
