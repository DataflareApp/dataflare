use crate::{Query, QueryColumn, QueryValue, Result, StringError};
use pglite::{PGlite, PGliteOptions, SimpleQuery};
use postgres_types::Type;

pub(crate) struct Connection {
    db: PGlite,
}

impl Connection {
    pub(crate) async fn connect(path: &str) -> Result<Self> {
        let options = PGliteOptions::default();
        let db = PGlite::open_with(path, options).await.string_err()?;
        Ok(Self { db })
    }

    pub(crate) async fn close(self) -> Result<()> {
        self.db.close().await.string_err()
    }

    pub(crate) async fn execute(&self, sql: &str) -> Result<()> {
        self.db.exec(sql).await.string_err()
    }

    pub(crate) async fn execute_batch(&self, sql: &str) -> Result<()> {
        self.execute(sql).await
    }

    pub(crate) async fn transaction(&self, sqls: &[&str]) -> Result<()> {
        let tx = self.db.transaction().await.string_err()?;
        for sql in sqls {
            if let Err(err) = tx.exec(sql).await {
                let _ = tx.rollback().await;
                return Err(err.to_string());
            }
        }
        tx.commit().await.string_err()
    }

    pub(crate) async fn query(&self, sql: &str) -> Result<Query> {
        let query = self.db.simple_query(sql).await.string_err()?;
        to_query(query)
    }
}

fn to_query(queries: Vec<SimpleQuery>) -> Result<Query> {
    let query = queries
        .first()
        .ok_or_else(|| "simple query returned no results".to_string())?;
    let rows_affected = rows_affected_from_tag(&query.command_tag).unwrap_or_default();

    let columns = query
        .columns
        .iter()
        .map(|(name, datatype)| QueryColumn {
            name: name.to_string(),
            datatype: datatype.name().to_string(),
        })
        .collect::<Vec<_>>();

    let mut rows = Vec::with_capacity(query.rows.len());
    for row in &query.rows {
        let mut values = Vec::with_capacity(row.len());
        for (index, value) in row.iter().enumerate() {
            values.push(query_value(value.as_deref(), &query.columns[index].1)?);
        }
        rows.push(values);
    }

    Ok(Query {
        columns,
        rows,
        rows_affected,
    })
}

fn query_value(value: Option<&str>, datatype: &Type) -> Result<QueryValue> {
    let Some(value) = value else {
        return Ok(QueryValue::Null);
    };
    match *datatype {
        Type::BOOL => decode_bool(value).map(QueryValue::Bool),
        Type::INT2 | Type::INT4 | Type::INT8 => value
            .parse::<i64>()
            .map(QueryValue::I64)
            .map_err(|err| err.to_string()),
        Type::FLOAT4 | Type::FLOAT8 => value
            .parse::<f64>()
            .map(QueryValue::F64)
            .map_err(|err| err.to_string()),
        Type::BYTEA => decode_bytea(value).map(QueryValue::Bytes),
        Type::OID => value
            .parse::<u32>()
            .map(QueryValue::U32)
            .map_err(|err| err.to_string()),
        _ => Ok(QueryValue::String(value.to_string())),
    }
}

fn decode_bool(value: &str) -> Result<bool> {
    match value {
        "t" => Ok(true),
        "f" => Ok(false),
        _ => Err(format!("invalid bool value '{value}'")),
    }
}

fn decode_bytea(value: &str) -> Result<Vec<u8>> {
    let value = value
        .strip_prefix("\\x")
        .ok_or_else(|| "bytea value must start with '\\x'".to_string())?;
    hex::decode(value).map_err(|err| err.to_string())
}

fn rows_affected_from_tag(tag: &str) -> Option<u64> {
    let mut parts = tag.split_ascii_whitespace();
    let command = parts.next()?;
    match command {
        "COPY" | "DELETE" | "INSERT" | "MERGE" | "UPDATE" => {
            parts.next_back().and_then(|s| s.parse::<u64>().ok())
        }
        _ => None,
    }
}

// pglite-rs can only boot once in a process, so run this test module separately:
// cargo test connection_queries_transactions_and_ddl
#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> String {
        let path = std::env::temp_dir().join(format!(
            "dataflare-libpglite-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        path.to_string_lossy().to_string()
    }

    fn cleanup(path: &str) {
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn connection_queries_transactions_and_ddl() {
        let path = test_path("connection");
        let result = crate::RUNTIME.block_on(async {
            let conn = Connection::connect(&path).await?;
            let query = conn
                .query(
                    "CREATE TABLE test (
                    id serial PRIMARY KEY,
                    name text,
                    score float8,
                    payload bytea,
                    active bool
                )",
                )
                .await?;
            assert_eq!(query.columns.len(), 0);
            assert_eq!(query.rows.len(), 0);
            assert_eq!(query.rows_affected, 0);

            let query = conn
                .query(
                    "INSERT INTO test (name, score, payload, active)
                     VALUES ('alice', 1.5, decode('6869', 'hex'), true)",
                )
                .await?;
            assert_eq!(query.columns.len(), 0);
            assert_eq!(query.rows.len(), 0);
            assert_eq!(query.rows_affected, 1);

            let query = conn
                .query(
                    "SELECT
                        NULL::text AS null_value,
                        true AS bool_value,
                        1::int2 AS int2_value,
                        2::int4 AS int4_value,
                        3::int8 AS int8_value,
                        3.14::float8 AS float_value,
                        decode('6869', 'hex') AS bytes_value,
                        'hello'::text AS text_value,
                        ARRAY[1, 2, 3]::int4[] AS array_value,
                        '{\"a\":1}'::json AS json_value,
                        42::oid AS oid_value",
                )
                .await?;
            assert_eq!(query.columns.len(), 11);
            assert_eq!(query.columns[0].datatype, "text");
            assert_eq!(query.columns[1].datatype, "bool");
            assert_eq!(query.columns[2].datatype, "int2");
            assert_eq!(query.columns[3].datatype, "int4");
            assert_eq!(query.columns[4].datatype, "int8");
            assert_eq!(query.columns[5].datatype, "float8");
            assert_eq!(query.columns[6].datatype, "bytea");
            assert_eq!(query.columns[7].datatype, "text");
            assert_eq!(query.columns[8].datatype, "_int4");
            assert_eq!(query.columns[9].datatype, "json");
            assert_eq!(query.columns[10].datatype, "oid");
            assert_eq!(query.rows.len(), 1);
            assert_value_null(&query.rows[0][0]);
            assert_value_bool(&query.rows[0][1], true);
            assert_value_i64(&query.rows[0][2], 1);
            assert_value_i64(&query.rows[0][3], 2);
            assert_value_i64(&query.rows[0][4], 3);
            assert_value_f64(&query.rows[0][5], 3.14);
            assert_value_bytes(&query.rows[0][6], b"hi");
            assert_value_string(&query.rows[0][7], "hello");
            assert_value_string(&query.rows[0][8], "{1,2,3}");
            assert_value_string(&query.rows[0][9], "{\"a\":1}");
            assert_value_u32(&query.rows[0][10], 42);

            let query = conn
                .query("SELECT id, name, ARRAY[1, 2, 3]::int4[] AS nums FROM test WHERE false")
                .await?;
            assert_eq!(query.columns.len(), 3);
            assert_eq!(query.columns[0].name, "id");
            assert_eq!(query.columns[0].datatype, "int4");
            assert_eq!(query.columns[1].name, "name");
            assert_eq!(query.columns[1].datatype, "text");
            assert_eq!(query.columns[2].name, "nums");
            assert_eq!(query.columns[2].datatype, "_int4");
            assert!(query.rows.is_empty());

            let query = conn
                .query("SELECT 1::int4 AS first_value; SELECT 2::int4 AS second_value")
                .await?;
            assert_eq!(query.columns.len(), 1);
            assert_eq!(query.columns[0].name, "first_value");
            assert_value_i64(&query.rows[0][0], 1);

            match to_query(Vec::new()) {
                Ok(_) => panic!("expected empty simple query results to fail"),
                Err(err) => assert_eq!(err, "simple query returned no results"),
            }

            conn.transaction(&[
                "INSERT INTO test (name) VALUES ('bob')",
                "INSERT INTO test (name) VALUES ('carol')",
            ])
            .await?;
            let err = conn
                .transaction(&[
                    "INSERT INTO test (name) VALUES ('dave')",
                    "INSERT INTO missing VALUES (1)",
                ])
                .await
                .unwrap_err();
            assert!(err.contains("missing"));

            let query = conn.query("SELECT count(*) FROM test").await?;
            assert_value_i64(&query.rows[0][0], 3);

            let query = conn
                .query("UPDATE test SET score = 2.5 WHERE name = 'alice'")
                .await?;
            assert_eq!(query.columns.len(), 0);
            assert_eq!(query.rows.len(), 0);
            assert_eq!(query.rows_affected, 1);

            let query = conn
                .query("SELECT score FROM test WHERE name = 'alice'")
                .await?;
            assert_value_f64(&query.rows[0][0], 2.5);

            let query = conn
                .query("CREATE TABLE ddl_check (id int PRIMARY KEY)")
                .await?;
            assert_eq!(query.columns.len(), 0);
            assert_eq!(query.rows.len(), 0);
            assert_eq!(query.rows_affected, 0);

            conn.close().await?;
            Ok::<(), String>(())
        });

        cleanup(&path);
        result.unwrap();
    }

    fn assert_value_i64(value: &QueryValue, expected: i64) {
        match value {
            QueryValue::I64(value) => assert_eq!(*value, expected),
            _ => panic!("expected i64 value"),
        }
    }

    fn assert_value_bool(value: &QueryValue, expected: bool) {
        match value {
            QueryValue::Bool(value) => assert_eq!(*value, expected),
            _ => panic!("expected bool value"),
        }
    }

    fn assert_value_f64(value: &QueryValue, expected: f64) {
        match value {
            QueryValue::F64(value) => assert_eq!(*value, expected),
            _ => panic!("expected f64 value"),
        }
    }

    fn assert_value_u32(value: &QueryValue, expected: u32) {
        match value {
            QueryValue::U32(value) => assert_eq!(*value, expected),
            _ => panic!("expected u32 value"),
        }
    }

    fn assert_value_bytes(value: &QueryValue, expected: &[u8]) {
        match value {
            QueryValue::Bytes(value) => assert_eq!(value, expected),
            _ => panic!("expected bytes value"),
        }
    }

    fn assert_value_string(value: &QueryValue, expected: &str) {
        match value {
            QueryValue::String(value) => assert_eq!(value, expected),
            _ => panic!("expected string value"),
        }
    }

    fn assert_value_null(value: &QueryValue) {
        match value {
            QueryValue::Null => {}
            _ => panic!("expected null value"),
        }
    }
}
