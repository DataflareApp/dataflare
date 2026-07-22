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

const TURSO_DRIVER_VERSION: &str = "20260722";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const TURSO_SHA256: &str = "4a86915df94d384bc5e2adf6da7ac1d9285346849adb610bd619840aa662bd10";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const TURSO_SHA256: &str = "9a2cc9d9255e69d18ed1c253c2ada8bfa484cab89276be23c9278abd98df29ca";
#[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "gnu"))]
const TURSO_SHA256: &str = "1f353cc0117ac23e25e35c4e6d774831ed36564fcfbd0c13608254b05424dfe1";
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
const TURSO_SHA256: &str = "291c88ff8d7402d124052ea1db2be0004338ca757678982bf38536b796c0ce94";
#[cfg(all(target_os = "windows", target_arch = "aarch64", target_env = "msvc"))]
const TURSO_SHA256: &str = "df2ec850003e812603ec4aacbd62c909341cc1613e6dd81317398dacf68900b7";
#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
const TURSO_SHA256: &str = "4ea7be86a2c79159e7f51c5decb99bf647b642a735faa79bf9e36984afdaa446";

#[derive(Debug)]
pub struct Connection {
    conn: Mutex<*mut c_void>,
    dylib: Dylib,
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

#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    pub cipher: String,
    pub key: String,
}

impl Connection {
    pub async fn connect(path: &str, encryption: Option<EncryptionConfig>) -> Result<Self> {
        let mut error = ErrorMessage::null();
        let dylib = Dylib::try_load("turso", TURSO_DRIVER_VERSION, TURSO_SHA256).await?;
        let enc_opts = encryption.as_ref().map(|e| EncryptionOptions {
            cipher: StringRef::new(&e.cipher),
            hexkey: StringRef::new(&e.key),
        });
        let options = ConnectOptions {
            path: StringRef::new(path),
            encryption: enc_opts
                .as_ref()
                .map(|e| e as *const EncryptionOptions)
                .unwrap_or(std::ptr::null()),
        };
        let conn = dylib.symbol::<ConnectFn>(CONNECT)?(options, &mut error);
        if let Some(err) = free_error(&dylib, error)? {
            return Err(err);
        }
        Ok(Self {
            conn: Mutex::new(conn),
            dylib,
        })
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

    pub fn execute_batch(&self, sql: &str) -> Result<(), Error> {
        let conn = self.conn.lock().map_err(|_| Error::Mutex)?;
        let mut error = ErrorMessage::null();
        self.dylib.symbol::<ExecuteBatchFn>(EXECUTE_BATCH)?(*conn, StringRef::new(sql), &mut error);
        if let Some(err) = free_error(&self.dylib, error)? {
            return Err(err);
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
        if let Some(err) = free_error(&self.dylib, error)? {
            return Err(err);
        }
        Ok(())
    }

    pub fn query(&self, sql: &str) -> Result<Query, Error> {
        let conn = self.conn.lock().map_err(|_| Error::Mutex)?;
        let now = Instant::now();
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
                        DataKind::I64 => Value::I64(data.value.i64),
                        DataKind::F64 => Value::F64(data.value.f64),
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
            duration: now.elapsed().as_millis() as u32,
        })
    }

    pub fn select(&self, sql: &str) -> Result<Vec<Vec<Value>>, Error> {
        self.query(sql).map(|q| q.rows)
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    #[tokio::test]
    async fn test_query() {
        let conn = Connection::connect(":memory:", None).await.unwrap();
        let mut query = conn
            .query(
                "SELECT 1 as int, 3.14 as float, 'hello' as text, x'FF00FF' as blob, NULL as empty, TRUE, FALSE",
            )
            .unwrap();
        let row = query.rows.remove(0);
        assert_eq!(row[0], Value::I64(1));
        assert_eq!(row[1], Value::F64(3.14));
        assert_eq!(row[2], Value::String("hello".into()));
        assert_eq!(row[3], Value::from_bytes(vec![0xFF, 0x00, 0xFF]));
        assert_eq!(row[4], Value::Null);
        assert_eq!(row[5], Value::I64(1));
        assert_eq!(row[6], Value::I64(0));
        assert_eq!(query.rows_affected, Some(0));
    }
}
