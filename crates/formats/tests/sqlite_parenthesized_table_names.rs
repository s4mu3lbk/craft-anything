use craft_anything_formats::sqlite::{CellValue, SqliteDb, parse_create_columns};
use craft_anything_testkit::sqlite::{Val, table_db, without_rowid_db};

fn text(value: &str) -> CellValue {
    CellValue::Text { text: value.into(), truncated: false }
}

/// A quoted table name may hold parentheses, as imported sheets often do ("export (1)"). The
/// column list starts at the first parenthesis outside quotes (and outside comments).
#[test]
fn quoted_table_names_with_parentheses_keep_their_columns() {
    for name in ["\"export (1)\"", "[Sheet1 (2)]", "`a(b`", "'q (1)'", "\"(\"", "t /* (v2) */"] {
        let sql = format!("CREATE TABLE {name} (id INTEGER PRIMARY KEY, name TEXT, size INT)");
        let columns = parse_create_columns(&sql);
        let names: Vec<_> = columns.iter().map(|c| c.name.as_str()).collect();
        let types: Vec<_> = columns.iter().map(|c| c.type_name.as_str()).collect();
        assert_eq!(names, ["id", "name", "size"], "{sql}");
        assert_eq!(types, ["INTEGER", "TEXT", "INT"], "{sql}");
    }
}

/// A line comment glued to the table name is a comment, so a quote inside it doesn't hide the
/// column list.
#[test]
fn a_line_comment_glued_to_the_table_name_is_skipped() {
    let names: Vec<_> = parse_create_columns("CREATE TABLE t-- here's the table\n(id INTEGER PRIMARY KEY, name TEXT)").into_iter().map(|c| c.name).collect();
    assert_eq!(names, ["id", "name"]);
    let sql = "CREATE TABLE t-- here's the key order\n(v INT, k TEXT, PRIMARY KEY (k)) WITHOUT ROWID";
    let data = without_rowid_db(sql, &[vec![Val::Text("x".into()), Val::Int(1)]]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![CellValue::Int(1), text("x")]);
}

/// The rowid alias of such a table still shows the rowid.
#[test]
fn a_parenthesized_table_name_keeps_the_rowid_alias() {
    let data = table_db("CREATE TABLE \"export (1)\" (id INTEGER PRIMARY KEY, name TEXT)", &[(7, vec![Val::Null, Val::Text("a".into())])]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![CellValue::Int(7), text("a")]);
}

/// A WITHOUT ROWID table named this way is still put back in declaration order.
#[test]
fn a_parenthesized_without_rowid_table_keeps_every_column() {
    let sql = "CREATE TABLE [Sheet1 (2)] (v INT, k TEXT, PRIMARY KEY (k)) WITHOUT ROWID";
    let data = without_rowid_db(sql, &[vec![Val::Text("x".into()), Val::Int(1)]]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![CellValue::Int(1), text("x")]);
    assert!(page.notes.is_empty(), "{:?}", page.notes);
}
