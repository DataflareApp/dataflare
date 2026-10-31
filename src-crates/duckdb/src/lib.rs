mod ffi;

use dylib::Dylib;
use dylib::driver::{Error, Result};
use dylib::ffi::{ErrorMessage, StringRef};
use ffi::*;
use query::{Query, QueryColumn, Value};
use std::ffi::c_void;

// NOTE:
// Do not update manually
// Use `node ./src-dylib/driver-update.mjs` update the sha256 values.

const DUCKDB_DRIVER_VERSION: &str = "20261031";
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const DUCKDB_SHA256: &str = "0b1b044d0be2c766fcf554951da6eb6f771e978c51d2bcdff1ad3c92ce8317e8";
#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const DUCKDB_SHA256: &str = "6825eddd6e70bf188d0d806e04e7077e25f42d683ef2d6a09ab6466d8c321218";
#[cfg(all(target_os = "linux", target_arch = "aarch64", target_env = "gnu"))]
const DUCKDB_SHA256: &str = "570dcb3aaeb72f463a66521db108fd668cb8da382c67f5b0bfd8c8a3e37dfaae";
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
const DUCKDB_SHA256: &str = "a7c7b02e94b596429d86ae8bc74f912412c64e58752a0b5b311cf33780e767ef";
#[cfg(all(target_os = "windows", target_arch = "aarch64", target_env = "msvc"))]
const DUCKDB_SHA256: &str = "c0dc792631abf1d4b6c7f41c2746b784352c47a01b9adfd9c974457266a72798";
#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
const DUCKDB_SHA256: &str = "5c1b2ebbce266a8a6802df59485886de8b5e9e17c2e4eb25e6da92e700878e73";

#[derive(Debug)]
pub struct Connection {
    conn: *mut c_void,
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

impl Connection {
    pub async fn connect(path: &str, readonly: bool) -> Result<Self> {
        let dylib = Dylib::try_load("duckdb", DUCKDB_DRIVER_VERSION, DUCKDB_SHA256).await?;
        let mut error = ErrorMessage::null();
        let conn = dylib.symbol::<ConnectFn>(CONNECT)?(StringRef::new(path), readonly, &mut error);
        if let Some(err) = free_error(&dylib, error)? {
            return Err(err);
        }
        Ok(Self { conn, dylib })
    }

    fn close(&self) -> Result<(), Error> {
        self.dylib.symbol::<CloseFn>(CLOSE)?(self.conn);
        Ok(())
    }

    pub fn try_clone(&self) -> Result<Self> {
        let dylib = self.dylib.clone();
        let mut error = ErrorMessage::null();
        let conn = dylib.symbol::<CloneFn>(CLONE)?(self.conn, &mut error);
        if let Some(err) = free_error(&dylib, error)? {
            return Err(err);
        }
        Ok(Self { conn, dylib })
    }

    pub fn execute(&self, sql: &str) -> Result<(), Error> {
        let mut error = ErrorMessage::null();
        self.dylib.symbol::<ExecuteFn>(EXECUTE)?(self.conn, StringRef::new(sql), &mut error);
        if let Some(err) = free_error(&self.dylib, error)? {
            return Err(err);
        }
        Ok(())
    }

    pub fn query(&self, sql: &str) -> Result<Query, Error> {
        let mut error = ErrorMessage::null();
        let query =
            self.dylib.symbol::<QueryFn>(QUERY)?(self.conn, StringRef::new(sql), &mut error);
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
                        DataKind::Bytes => Value::Bytes(data.value.bytes.as_bytes().into()),
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
            duration: meta.duration,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    use query::{QueryColumn, Value};

    #[tokio::test]
    async fn test_query() {
        let conn = Connection::connect("", false).await.unwrap();
        let mut query = conn.query("select 'hello' as hello").unwrap();
        assert_eq!(query.columns.len(), 1);
        assert_eq!(
            query.columns.remove(0),
            QueryColumn {
                name: "hello".into(),
                datatype: "varchar".into()
            }
        );
        assert_eq!(query.rows.len(), 1);
        assert_eq!(query.rows[0].len(), 1);
        assert_eq!(
            query.rows.remove(0).remove(0),
            Value::String("hello".into())
        );
        assert_eq!(query.rows_affected, Some(0));
    }

    #[tokio::test]
    async fn test_extension() {
        let conn = Connection::connect("", false).await.unwrap();
        let query = conn
            .query("SELECT * FROM 'https://duckdb.org/data/records.json'")
            .unwrap();
        assert_eq!(query.columns.len(), 2);
        assert_eq!(query.rows.len(), 3);
    }
}
