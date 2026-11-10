use craft_anything_formats::sqlite::{SqliteDb, parse_create_columns};
use craft_anything_testkit::sqlite::table_db;

/// A declared type keeps the comma inside its parentheses, both in the parsed columns and in the
/// schema overview the database tools show.
#[test]
fn a_type_comma_survives_the_schema_overview() {
    let sql = "CREATE TABLE t (amount DECIMAL(10, 2) NOT NULL, ratio NUMERIC(5,1), id INTEGER PRIMARY KEY)";
    let types: Vec<String> = parse_create_columns(sql).into_iter().map(|c| c.type_name).collect();
    assert_eq!(types, ["DECIMAL(10, 2)", "NUMERIC(5,1)", "INTEGER"]);

    let data = table_db(sql, &[]);
    let parsed = SqliteDb::parse(&data).expect("the synthetic database parses");
    let overview = parsed.overview();
    let columns = &overview.schema[0].columns;
    assert_eq!(columns[0].type_name, "DECIMAL(10, 2)");
    assert_eq!(columns[1].type_name, "NUMERIC(5,1)");
    assert_eq!(columns[2].type_name, "INTEGER");
}
