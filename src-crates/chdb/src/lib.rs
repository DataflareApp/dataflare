mod ffi;

use dylib::Dylib;
use dylib::driver::{Error, Result};
use dylib::ffi::{ErrorMessage, StringRef};
use ffi::*;
use query::{Query, QueryColumn, Value};
use std::ffi::c_void;
use std::sync::Mutex;

// NOTE:
// Do not update manually
// Use `node ./src-dylib/driver-update.mjs` update the sha256 values.

const CHDB_DRIVER_VERSION: &str = "20260825";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const CHDB_SHA256: &str = "830cc396ca54b61c8e8aea4b5a3f72b18b16a7e5437534edb70064dfd7a23476";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const CHDB_SHA256: &str = "a65c639717fee36505a0b50afa7fadde64561b1721baca04dd17cd7002886be4";
#[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "gnu"))]
const CHDB_SHA256: &str = "07322d38805f9167edadc5351075bcab82ff7b20e0f251f34354b2a6f0aa3aa2";
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
const CHDB_SHA256: &str = "8843ce6a0ae3d2c0ee5cec0c4be9a9e824a12217070dc8941423159afd2b217e";
#[cfg(all(target_os = "windows", target_arch = "aarch64", target_env = "msvc"))]
const CHDB_SHA256: &str = ""; // Unsupported platform
#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
const CHDB_SHA256: &str = ""; // Unsupported platform

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
        let msg = error.as_str().to_string();
        dylib.symbol::<FreeErrorFn>(FREE_ERROR)?(error);
        return Ok(Some(Error::Message(msg)));
    }
    Ok(None)
}

impl Connection {
    pub async fn connect(path: &str, database: &str) -> Result<Self> {
        if CHDB_SHA256.is_empty() {
            return Err(Error::Message(
                "chDB is not supported on the current platform.".into(),
            ));
        }
        let mut error = ErrorMessage::null();
        let dylib = Dylib::try_load("chdb", CHDB_DRIVER_VERSION, CHDB_SHA256).await?;
        let options = ConnectOptions {
            path: StringRef::new(path),
        };
        let conn = dylib.symbol::<ConnectFn>(CONNECT)?(options, &mut error);
        if let Some(err) = free_error(&dylib, error)? {
            return Err(err);
        }
        let conn = Self {
            conn: Mutex::new(conn),
            dylib,
            path: path.to_string(),
        };
        {
            // TODO: Migrate the database field to the dynamic library in the future.
            let db = database.trim();
            if !db.is_empty() {
                let escaped = db.replace('"', "\"\"");
                conn.execute(&format!("USE \"{}\"", escaped))?;
            }
        }
        Ok(conn)
    }

    pub fn path(&self) -> &str {
        &self.path
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
        if let Some(err) = free_error(&self.dylib, error)? {
            return Err(err);
        }
        Ok(())
    }

    pub fn query(&self, sql: &str) -> Result<Query, Error> {
        let conn = self.conn.lock().map_err(|_| Error::Mutex)?;
        let mut error = ErrorMessage::null();
        let query = self.dylib.symbol::<QueryFn>(QUERY)?(*conn, StringRef::new(sql), &mut error);
        if let Some(err) = free_error(&self.dylib, error)? {
            return Err(err);
        }
        let meta = self.dylib.symbol::<QueryMetaFn>(QUERY_META)?(query);
        let query_column = self.dylib.symbol::<QueryColumnFn>(QUERY_COLUMN)?;
        let query_value = self.dylib.symbol::<QueryValueFn>(QUERY_VALUE)?;
        let columns = (0..meta.column_count)
            .map(|i| {
                let col = query_column(query, i);
                QueryColumn {
                    name: col.name.as_str().to_string(),
                    datatype: col.datatype.as_str().to_string(),
                }
            })
            .collect::<Vec<_>>();
        let mut rows = Vec::with_capacity(meta.row_count);
        for y in 0..meta.row_count {
            let mut row = Vec::with_capacity(meta.column_count);
            for x in 0..meta.column_count {
                let data = query_value(query, y, x);
                row.push(unsafe {
                    match data.kind {
                        DataKind::Null => Value::Null,
                        DataKind::Bool => Value::Bool(data.value.bool),
                        DataKind::I8 => Value::I8(data.value.i8),
                        DataKind::I16 => Value::I16(data.value.i16),
                        DataKind::I32 => Value::I32(data.value.i32),
                        DataKind::I64 => Value::I64(data.value.i64),
                        DataKind::U8 => Value::U8(data.value.u8),
                        DataKind::U16 => Value::U16(data.value.u16),
                        DataKind::U32 => Value::U32(data.value.u32),
                        DataKind::U64 => Value::U64(data.value.u64),
                        DataKind::F32 => Value::F32(data.value.f32),
                        DataKind::F64 => Value::F64(data.value.f64),
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
            // TODO
            rows_affected: None,
            duration: meta.duration,
        })
    }

    pub fn select(&self, sql: &str) -> Result<Vec<Vec<Value>>, Error> {
        self.query(sql).map(|q| q.rows)
    }
}

// NOTE: use `cargo test -p chdb -- --test-threads 1` run test
#[cfg(test)]
mod tests {
    use crate::*;
    use query::{QueryColumn, Value};

    #[tokio::test]
    async fn test_query() {
        let conn = Connection::connect(":memory:", "").await.unwrap();
        let mut query = conn.query("select 'hello' as hello").unwrap();
        assert_eq!(query.columns.len(), 1);
        assert_eq!(
            query.columns.remove(0),
            QueryColumn {
                name: "hello".into(),
                datatype: "String".into()
            }
        );
        assert_eq!(query.rows.len(), 1);
        assert_eq!(query.rows[0].len(), 1);
        assert_eq!(
            query.rows.remove(0).remove(0),
            Value::String("hello".into())
        );
        assert_eq!(query.rows_affected, None);
    }

    #[tokio::test]
    async fn test_multiples_memory() {
        let _conn1 = Connection::connect(":memory:", "").await.unwrap();
        let _conn2 = Connection::connect(":memory:", "").await.unwrap();
        let _conn3 = Connection::connect(":memory:", "").await.unwrap();
    }

    #[tokio::test]
    async fn test_multiples_persistent_database() {
        let _conn1 = Connection::connect("./test-ch1.db", "").await.unwrap();
        let conn2 = Connection::connect("./test-ch2.db", "").await;
        assert!(conn2.is_err());
    }

    #[tokio::test]
    async fn test_memory_database_error() {
        let file = Connection::connect("./test-ch2.db", "").await;

        let memory = Connection::connect(":memory:", "").await;
        assert!(memory.is_err());

        // Drop the file connection and try again.
        drop(file);

        let memory = Connection::connect(":memory:", "").await;
        assert!(memory.is_ok());
    }

    #[tokio::test]
    async fn test_persistent_database_error() {
        let memory = Connection::connect(":memory:", "").await.unwrap();

        // Persistent connection cannot coexist with in-memory databases.
        let file = Connection::connect("./test-ch.db", "").await;
        assert!(file.is_err());

        // Drop the in-memory connection and try again.
        drop(memory);

        let file = Connection::connect("./test-ch.db", "").await;
        assert!(file.is_ok());
    }
}
