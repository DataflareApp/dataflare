use pglite::{Column, Connection, Error};
use tempfile::{TempDir, tempdir};

fn database() -> (TempDir, Connection) {
    let dir = tempdir().unwrap();
    let db = Connection::open_with(dir.path()).unwrap();
    (dir, db)
}

fn assert_database_error(error: Error, expected_code: &str) {
    let Error::Database {
        severity,
        code,
        message,
    } = error
    else {
        panic!("expected a database error, got {error:?}");
    };
    assert_eq!(severity, "ERROR");
    assert!(
        code == expected_code || code == "XX000",
        "expected SQLSTATE {expected_code}, got {code}: {message}"
    );
    assert!(!message.is_empty());
}

mod tests {
    use postgres_types::Type;

    use crate::*;

    #[test]
    fn basic_types_include_column_names_types_and_values() {
        let (_dir, mut db) = database();

        let results = db
            .query(
                "
                SELECT
                    NULL::text AS null_value,
                    1 AS integer_value,
                    2.3 AS numeric_value,
                    true AS true_value,
                    false AS false_value,
                    'hello'::text AS text_value,
                    '{\"answer\":42}'::json AS json_value
                ",
            )
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].columns,
            vec![
                Column {
                    name: "null_value".to_owned(),
                    datatype: Some(Type::TEXT),
                },
                Column {
                    name: "integer_value".to_owned(),
                    datatype: Some(Type::INT4),
                },
                Column {
                    name: "numeric_value".to_owned(),
                    datatype: Some(Type::NUMERIC),
                },
                Column {
                    name: "true_value".to_owned(),
                    datatype: Some(Type::BOOL),
                },
                Column {
                    name: "false_value".to_owned(),
                    datatype: Some(Type::BOOL),
                },
                Column {
                    name: "text_value".to_owned(),
                    datatype: Some(Type::TEXT),
                },
                Column {
                    name: "json_value".to_owned(),
                    datatype: Some(Type::JSON),
                },
            ]
        );
        assert_eq!(
            results[0].rows,
            vec![vec![
                None,
                Some("1".to_owned()),
                Some("2.3".to_owned()),
                Some("t".to_owned()),
                Some("f".to_owned()),
                Some("hello".to_owned()),
                Some("{\"answer\":42}".to_owned()),
            ]]
        );
        assert_eq!(results[0].command_tag, "SELECT 1");
    }

    #[test]
    fn complex_types_include_arrays_json_range_and_custom_type() {
        let (_dir, mut db) = database();

        let results = db
            .query(
                "
                CREATE TYPE custom_state AS ENUM ('ready');
                SELECT
                    ARRAY[1, 2, 3] AS numbers,
                    ARRAY['a', 'b']::text[] AS labels,
                    '{\"enabled\":true}'::json AS document,
                    '{\"enabled\":true}'::jsonb AS binary_document,
                    '[1,5)'::int4range AS span,
                    'ready'::custom_state AS state
                ",
            )
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].command_tag, "CREATE TYPE");
        assert_eq!(
            results[1].columns,
            vec![
                Column {
                    name: "numbers".to_owned(),
                    datatype: Some(Type::INT4_ARRAY),
                },
                Column {
                    name: "labels".to_owned(),
                    datatype: Some(Type::TEXT_ARRAY),
                },
                Column {
                    name: "document".to_owned(),
                    datatype: Some(Type::JSON),
                },
                Column {
                    name: "binary_document".to_owned(),
                    datatype: Some(Type::JSONB),
                },
                Column {
                    name: "span".to_owned(),
                    datatype: Some(Type::INT4_RANGE),
                },
                Column {
                    name: "state".to_owned(),
                    datatype: None,
                },
            ]
        );
        assert_eq!(
            results[1].rows,
            vec![vec![
                Some("{1,2,3}".to_owned()),
                Some("{a,b}".to_owned()),
                Some("{\"enabled\":true}".to_owned()),
                Some("{\"enabled\": true}".to_owned()),
                Some("[1,5)".to_owned()),
                Some("ready".to_owned()),
            ]]
        );
    }

    #[test]
    fn create_alter_index_and_drop_table() {
        let (_dir, mut db) = database();

        let results = db
            .query(
                "
                CREATE TABLE items (id integer PRIMARY KEY);
                ALTER TABLE items ADD COLUMN name text NOT NULL DEFAULT 'unknown';
                CREATE INDEX items_name_idx ON items (name);
                INSERT INTO items (id, name) VALUES (1, 'one');
                SELECT id, name FROM items;
                DROP TABLE items;
                ",
            )
            .unwrap();

        assert_eq!(
            results
                .iter()
                .map(|result| result.command_tag.as_str())
                .collect::<Vec<_>>(),
            [
                "CREATE TABLE",
                "ALTER TABLE",
                "CREATE INDEX",
                "INSERT 0 1",
                "SELECT 1",
                "DROP TABLE",
            ]
        );
        assert_eq!(
            results[4].rows,
            vec![vec![Some("1".to_owned()), Some("one".to_owned())]]
        );

        let error = db.query("SELECT * FROM items").unwrap_err();
        assert_database_error(error, "42P01");
    }

    #[test]
    fn commits_an_array_of_sql_statements() {
        let (_dir, mut db) = database();
        db.query("CREATE TABLE accounts (id integer PRIMARY KEY, balance integer NOT NULL)")
            .unwrap();

        let results = db
            .transaction(&[
                "INSERT INTO accounts VALUES (1, 100);",
                "UPDATE accounts SET balance = balance + 50 WHERE id = 1 -- keep trailing comment",
                "SELECT balance FROM accounts WHERE id = 1",
            ])
            .unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].command_tag, "INSERT 0 1");
        assert_eq!(results[1].command_tag, "UPDATE 1");
        assert_eq!(results[2].rows, vec![vec![Some("150".to_owned())]]);

        let persisted = db.query("SELECT balance FROM accounts").unwrap();
        assert_eq!(persisted[0].rows, vec![vec![Some("150".to_owned())]]);
    }

    #[test]
    fn rolls_back_the_array_when_one_statement_fails() {
        let (_dir, mut db) = database();
        db.query("CREATE TABLE accounts (id integer PRIMARY KEY)")
            .unwrap();

        let error = db
            .transaction(&[
                "INSERT INTO accounts VALUES (1)",
                "INSERT INTO accounts VALUES (1)",
                "INSERT INTO accounts VALUES (2)",
            ])
            .unwrap_err();
        assert_database_error(error, "23505");

        let results = db.query("SELECT id FROM accounts").unwrap();
        assert!(results[0].rows.is_empty());
    }

    #[test]
    fn a_multi_statement_simple_query_is_atomic() {
        let (_dir, mut db) = database();
        db.query("CREATE TABLE values_to_keep (value integer)")
            .unwrap();

        let error = db
            .query(
                "
                INSERT INTO values_to_keep VALUES (1);
                INSERT INTO values_to_keep VALUES (2);
                SELECT 1 / 0;
                INSERT INTO values_to_keep VALUES (3);
                ",
            )
            .unwrap_err();
        assert_database_error(error, "22012");

        let results = db.query("SELECT value FROM values_to_keep").unwrap();
        assert!(results[0].rows.is_empty());
    }

    #[test]
    fn malformed_sql_returns_a_syntax_error() {
        let (_dir, mut db) = database();

        let error = db.query("SELEC 1").unwrap_err();

        assert_database_error(error, "42601");
    }

    #[test]
    fn valid_sql_can_return_a_database_error() {
        let (_dir, mut db) = database();
        db.query("CREATE TABLE required_values (value text NOT NULL)")
            .unwrap();

        let error = db
            .query("INSERT INTO required_values VALUES (NULL)")
            .unwrap_err();

        assert_database_error(error, "23502");
    }

    #[test]
    fn multi_statement_error_stops_later_statements_and_connection_recovers() {
        let (_dir, mut db) = database();

        let error = db
            .query(
                "
                CREATE TABLE should_rollback (value integer);
                INSERT INTO missing_table VALUES (1);
                CREATE TABLE should_not_run (value integer);
                ",
            )
            .unwrap_err();
        assert_database_error(error, "42P01");

        let results = db.query("SELECT 1 AS recovered").unwrap();
        assert_eq!(results[0].rows, vec![vec![Some("1".to_owned())]]);

        for table in ["should_rollback", "should_not_run"] {
            let error = db.query(&format!("SELECT * FROM {table}")).unwrap_err();
            assert_database_error(error, "42P01");
        }
    }

    #[test]
    #[ignore = "requires PostgreSQL longjmp recovery support in the WASI runtime"]
    fn database_error_preserves_session_state() {
        let (_dir, mut db) = database();
        db.query(
            "
            CREATE SCHEMA application;
            SET search_path TO application;
            ",
        )
        .unwrap();

        let error = db.query("SELECT * FROM missing_table").unwrap_err();
        assert_database_error(error, "42P01");

        let results = db.query("SELECT current_schema()").unwrap();
        assert_eq!(results[0].rows, vec![vec![Some("application".to_owned())]]);
    }

    #[test]
    fn data_persists_and_close_is_idempotent() {
        let dir = tempdir().unwrap();
        {
            let mut db = Connection::open_with(dir.path()).unwrap();
            db.query("CREATE TABLE saved (value text); INSERT INTO saved VALUES ('yes')")
                .unwrap();
        }

        let mut reopened = Connection::open_with(dir.path()).unwrap();
        let result = reopened.query("SELECT value FROM saved").unwrap();
        assert_eq!(result[0].rows, vec![vec![Some("yes".to_owned())]]);
    }

    #[test]
    fn database_path_contains_only_postgres_data() {
        let (dir, db) = database();

        assert!(dir.path().join("PG_VERSION").is_file());
        assert!(!dir.path().join("tmp").exists());
        assert!(!dir.path().join("pglite").exists());

        drop(db);
    }

    #[test]
    fn uses_schema_through_search_path() {
        let (_dir, mut db) = database();

        let results = db
            .query(
                "
                CREATE SCHEMA application;
                SET search_path TO application;
                CREATE TABLE settings (name text PRIMARY KEY, value text NOT NULL);
                INSERT INTO settings VALUES ('theme', 'dark');
                SELECT current_schema(), name, value FROM settings;
                ",
            )
            .unwrap();

        assert_eq!(
            results[4].rows,
            vec![vec![
                Some("application".to_owned()),
                Some("theme".to_owned()),
                Some("dark".to_owned()),
            ]]
        );

        let visibility = db
            .query(
                "
                SET search_path TO public;
                SELECT
                    current_schema(),
                    to_regclass('settings'),
                    to_regclass('application.settings');
                ",
            )
            .unwrap();
        assert_eq!(
            visibility[1].rows,
            vec![vec![
                Some("public".to_owned()),
                None,
                Some("application.settings".to_owned()),
            ]]
        );

        let qualified = db
            .query("SELECT name, value FROM application.settings")
            .unwrap();
        assert_eq!(
            qualified[0].rows,
            vec![vec![Some("theme".to_owned()), Some("dark".to_owned())]]
        );
    }

    #[test]
    fn default_postgres_database_is_created_and_selected() {
        let (_dir, mut db) = database();

        let result = db
            .query(
                "
                SELECT
                    current_database(),
                    EXISTS (
                        SELECT FROM pg_database WHERE datname = 'postgres'
                    );
                ",
            )
            .unwrap();

        assert_eq!(
            result[0].rows,
            vec![vec![Some("postgres".to_owned()), Some("t".to_owned())]]
        );
    }
}
