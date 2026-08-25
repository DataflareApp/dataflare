mod ffi;

use dylib::Dylib;
use dylib::driver::{Error, Result};
use dylib::ffi::{ErrorMessage, StringRef};
use ffi::*;
use query::{Query, QueryColumn, Value};
use std::{ffi::c_void, sync::Mutex, time::Instant};

// NOTE:
// Do not update manually
// Use `node ./src-dylib/driver-update.mjs` update the sha256 values.

const PGLITE_DRIVER_VERSION: &str = "20260825";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const PGLITE_SHA256: &str = "17e62671100bc92898c3775cc9991bd1346c77a838aa0588007467e950736c97";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const PGLITE_SHA256: &str = "1acfe2a7702eb27144a7609f6f04584cc3607ff03f214809253ae4169c5d2924";
#[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "gnu"))]
const PGLITE_SHA256: &str = "d81df6e47f434f3849b517f2b5367cea93b6d18593588062d39d9cbbbb6901dd";
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
const PGLITE_SHA256: &str = "38a2769d31fe09e2c6c1faada51f9fb03951f431df6208314f094d2cfe32a141";
#[cfg(all(target_os = "windows", target_arch = "aarch64", target_env = "msvc"))]
const PGLITE_SHA256: &str = "5425fb18497c0c3b258ec56bcbfe3533ae268bcaa0b1dcfb625cb81d134b471a";
#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
const PGLITE_SHA256: &str = "af2290312e4ced4f5d1a0fe852cc340293a5cc4144cd85fb116cc1b06f238303";

#[derive(Debug)]
pub struct Connection {
    conn: Mutex<*mut c_void>,
    dylib: Dylib,
    path: String,
}

unsafe impl Send for Connection {}
unsafe impl Sync for Connection {}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn free_error(dylib: &Dylib, error: ErrorMessage) -> Result<Option<Error>, Error> {
    if !error.is_null() {
        let message = error.as_str().to_string();
        dylib.symbol::<FreeErrorFn>(FREE_ERROR)?(error);
        return Ok(Some(Error::Message(message)));
    }
    Ok(None)
}

impl Connection {
    pub async fn connect(path: &str) -> Result<Self> {
        Connection::check_path(path)?;

        let dylib = Dylib::try_load("pglite", PGLITE_DRIVER_VERSION, PGLITE_SHA256).await?;
        let options = ConnectOptions {
            path: StringRef::new(path),
        };
        let mut error = ErrorMessage::null();
        let conn = dylib.symbol::<ConnectFn>(CONNECT)?(options, &mut error);
        if let Some(error) = free_error(&dylib, error)? {
            return Err(error);
        }
        Ok(Self {
            conn: Mutex::new(conn),
            dylib,
            path: path.to_string(),
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    fn check_path(path: &str) -> Result<()> {
        let path = path.trim();
        if path.is_empty() || path == ":memory:" {
            return Err(Error::Message(
                "PGlite path cannot be empty or :memory:".into(),
            ));
        }
        Ok(())
    }

    fn close(&self) -> Result<(), Error> {
        let conn = self.conn.lock().map_err(|_| Error::Mutex)?;
        self.dylib.symbol::<CloseFn>(CLOSE)?(*conn);
        Ok(())
    }

    pub fn execute(&self, sql: &str) -> Result<(), Error> {
        let conn = self.conn.lock().map_err(|_| Error::Mutex)?;
        let mut error = ErrorMessage::null();
        self.dylib.symbol::<ExecuteFn>(EXECUTE)?(*conn, StringRef::new(sql), &mut error);
        if let Some(error) = free_error(&self.dylib, error)? {
            return Err(error);
        }
        Ok(())
    }

    pub fn transaction(&self, sqls: &[String]) -> Result<(), Error> {
        let conn = self.conn.lock().map_err(|_| Error::Mutex)?;
        let sqls = sqls
            .iter()
            .map(|sql| StringRef::new(sql))
            .collect::<Vec<_>>();
        let mut error = ErrorMessage::null();
        self.dylib.symbol::<TransactionFn>(TRANSACTION)?(
            *conn,
            sqls.as_ptr(),
            sqls.len(),
            &mut error,
        );
        if let Some(error) = free_error(&self.dylib, error)? {
            return Err(error);
        }
        Ok(())
    }

    pub fn query(&self, sql: &str) -> Result<Query, Error> {
        let conn = self.conn.lock().map_err(|_| Error::Mutex)?;
        let start = Instant::now();
        let mut error = ErrorMessage::null();
        let query = self.dylib.symbol::<QueryFn>(QUERY)?(*conn, StringRef::new(sql), &mut error);
        if let Some(error) = free_error(&self.dylib, error)? {
            return Err(error);
        }

        let meta = self.dylib.symbol::<QueryMetaFn>(QUERY_META)?(query);
        let query_column = self.dylib.symbol::<QueryColumnFn>(QUERY_COLUMN)?;
        let query_value = self.dylib.symbol::<QueryValueFn>(QUERY_VALUE)?;
        let columns = (0..meta.column_count)
            .map(|index| {
                let column = query_column(query, index);
                QueryColumn {
                    name: column.name.as_str().to_string(),
                    datatype: column.datatype.as_str().to_string(),
                }
            })
            .collect::<Vec<_>>();

        let mut rows = Vec::with_capacity(meta.row_count);
        for row_index in 0..meta.row_count {
            let mut row = Vec::with_capacity(meta.column_count);
            for column_index in 0..meta.column_count {
                let data = query_value(query, row_index, column_index);
                row.push(unsafe {
                    match data.kind {
                        DataKind::Null => Value::Null,
                        DataKind::Bool => Value::Bool(data.value.bool),
                        DataKind::I64 => Value::I64(data.value.i64),
                        DataKind::F64 => Value::F64(data.value.f64),
                        DataKind::U32 => Value::U32(data.value.u32),
                        DataKind::Bytes => Value::from_bytes(data.value.bytes.as_bytes().to_vec()),
                        DataKind::String => Value::String(data.value.string.as_str().to_string()),
                    }
                });
            }
            rows.push(row);
        }
        self.dylib.symbol::<FreeQueryFn>(FREE_QUERY)?(query);

        Ok(Query {
            columns,
            rows,
            rows_affected: Some(meta.rows_affected),
            duration: start.elapsed().as_millis() as u32,
        })
    }

    pub fn select(&self, sql: &str) -> Result<Vec<Vec<Value>>, Error> {
        self.query(sql).map(|query| query.rows)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    fn database_path(name: &str) -> String {
        std::env::temp_dir()
            .join(format!("dataflare-pglite-{name}-{}", std::process::id()))
            .to_string_lossy()
            .to_string()
    }

    #[tokio::test]
    async fn query_returns_typed_values() {
        let path = database_path("query");
        let _ = std::fs::remove_dir_all(&path);
        let connection = Connection::connect(&path).await.unwrap();
        let mut query = connection
            .query(
                "SELECT 42::int4 AS number, true AS enabled, 'hello'::text AS message, decode('CAFE', 'hex') AS bytes",
            )
            .unwrap();

        assert_eq!(
            query.columns,
            [
                QueryColumn::new("number", "int4"),
                QueryColumn::new("enabled", "bool"),
                QueryColumn::new("message", "text"),
                QueryColumn::new("bytes", "bytea"),
            ]
        );
        assert_eq!(
            query.rows.remove(0),
            [
                Value::I64(42),
                Value::Bool(true),
                Value::String("hello".into()),
                Value::from_bytes(vec![0xca, 0xfe]),
            ]
        );
        drop(connection);
        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn information_schema_is_available() {
        let path = database_path("information-schema");
        let _ = std::fs::remove_dir_all(&path);
        let connection = Connection::connect(&path).await.unwrap();

        let rows = connection
            .select(
                "
                SELECT c.schema_name, t.table_name
                FROM information_schema.schemata AS c
                LEFT JOIN information_schema.tables AS t ON t.table_schema = c.schema_name
                ORDER BY c.schema_name, t.table_name
                ",
            )
            .unwrap();

        assert!(!rows.is_empty());
        drop(connection);
        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn transaction_commits_statements() {
        let path = database_path("transaction");
        let _ = std::fs::remove_dir_all(&path);
        let connection = Connection::connect(&path).await.unwrap();
        connection
            .transaction(&[
                "CREATE TABLE items (id integer PRIMARY KEY, name text NOT NULL)".into(),
                "INSERT INTO items VALUES (1, 'one')".into(),
            ])
            .unwrap();

        assert_eq!(
            connection.select("SELECT name FROM items").unwrap(),
            [vec![Value::String("one".into())]]
        );
        drop(connection);
        let _ = std::fs::remove_dir_all(path);
    }
}
