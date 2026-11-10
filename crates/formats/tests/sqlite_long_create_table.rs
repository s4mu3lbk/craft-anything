use craft_anything_formats::sqlite::{CellValue, SqliteDb};
use craft_anything_testkit::sqlite::{Val, table_db, without_rowid_db};

fn text(value: &str) -> CellValue {
    CellValue::Text { text: value.into(), truncated: false }
}

/// `count` filler columns named `c000`, `c001`, …, each `INT`.
fn filler(count: usize) -> String {
    (0..count).map(|i| format!("c{i:03} INT")).collect::<Vec<_>>().join(", ")
}

/// A CREATE TABLE longer than the 4 KiB text cap is read whole: every column is listed, the
/// rowid alias shows the rowid, and the last column holds its own value.
#[test]
fn a_long_create_table_keeps_its_later_columns_and_rowid_alias() {
    let sql = format!("CREATE TABLE t(id INTEGER PRIMARY KEY, {}, last TEXT)", filler(470));
    assert!(sql.len() > 4096, "the statement must pass the text cap");
    let mut record = vec![Val::Null; 471];
    record.push(Val::Text("end".into()));
    let data = table_db(&sql, &[(7, record)]);
    let db = SqliteDb::parse(&data).unwrap();
    let (schema, _) = db.schema();
    let names: Vec<_> = schema[0].columns.iter().map(|column| column.name.as_str()).collect();
    assert_eq!(names.len(), 472);
    assert_eq!((names[0], names[471]), ("id", "last"));
    assert_eq!(schema[0].columns[471].type_name, "TEXT");
    // The statement shown in the schema is still cut at the cap.
    assert!(schema[0].sql.len() <= 4096 && sql.starts_with(&schema[0].sql));

    let page = db.read_rows(2, 0, 10);
    let values = &page.rows[0].values;
    assert_eq!((values[0].clone(), values[471].clone()), (CellValue::Int(7), text("end")));
    assert!(page.notes.is_empty(), "{:?}", page.notes);
}

/// `WITHOUT ROWID` comes at the very end of the statement, so a cut statement read the table as a
/// rowid table and showed none of its rows.
#[test]
fn a_long_without_rowid_table_still_shows_its_rows() {
    let sql = format!("CREATE TABLE t(k TEXT PRIMARY KEY, {}, last TEXT) WITHOUT ROWID", filler(470));
    let row = |key: &str, last: &str| {
        let mut values = vec![Val::Text(key.into())];
        values.extend(std::iter::repeat_n(Val::Null, 470));
        values.push(Val::Text(last.into()));
        values
    };
    let data = without_rowid_db(&sql, &[row("a", "end"), row("b", "fin")]);
    let page = SqliteDb::parse(&data).unwrap().read_rows(2, 0, 10);
    let ends: Vec<_> = page.rows.iter().map(|row| (row.values[0].clone(), row.values[471].clone())).collect();
    assert_eq!(ends, vec![(text("a"), text("end")), (text("b"), text("fin"))]);
}

/// A column added with `ALTER TABLE ADD COLUMN` after a long statement still fills its default
/// into older rows, and a long constraint (here a CHECK over two overflow pages) doesn't hide the
/// columns after it.
#[test]
fn columns_after_a_long_constraint_keep_their_defaults_and_alias() {
    let values = (0..1000).map(|i| format!("'v{i:04}'")).collect::<Vec<_>>().join(", ");
    let sql = format!("CREATE TABLE t(x TEXT CHECK (x IN ({values})), id INTEGER PRIMARY KEY, extra TEXT DEFAULT 'filled')");
    assert!(sql.len() > 8192, "the statement must spill over more than one overflow page");
    let data = table_db(&sql, &[(5, vec![Val::Text("v0001".into()), Val::Null])]);
    let db = SqliteDb::parse(&data).unwrap();
    let names: Vec<_> = db.schema().0[0].columns.iter().map(|column| column.name.clone()).collect();
    assert_eq!(names, ["x", "id", "extra"]);
    let page = db.read_rows(2, 0, 10);
    assert_eq!(page.rows[0].values, vec![text("v0001"), CellValue::Int(5), text("filled")]);
}
