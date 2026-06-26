#[path = "../../../src-crates/dylib/src/ffi.rs"]
mod ffi;

mod connection;

use connection::Connection;
use ffi::{BytesRef, ErrorMessage, StringRef, TypedValue};
use std::ptr::null_mut;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create async runtime")
});

pub(crate) type Result<T, E = String> = std::result::Result<T, E>;

pub(crate) trait StringError<T> {
    fn string_err(self) -> Result<T>;
}

impl<T, E> StringError<T> for std::result::Result<T, E>
where
    E: ToString,
{
    fn string_err(self) -> Result<T> {
        self.map_err(|err| err.to_string())
    }
}

pub(crate) struct Query {
    pub(crate) columns: Vec<QueryColumn>,
    pub(crate) rows: Vec<Vec<QueryValue>>,
    pub(crate) rows_affected: u64,
}

pub(crate) struct QueryColumn {
    pub(crate) name: String,
    pub(crate) datatype: String,
}

pub(crate) enum QueryValue {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    U32(u32),
    Bytes(Vec<u8>),
    String(String),
}

#[repr(C)]
pub struct Meta {
    pub column_count: usize,
    pub row_count: usize,
    pub rows_affected: u64,
}

#[repr(C)]
pub struct Column {
    pub name: StringRef,
    pub datatype: StringRef,
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq)]
pub enum DataKind {
    Null,
    Bool,
    I64,
    F64,
    U32,
    Bytes,
    String,
}

#[repr(C)]
pub union Data {
    pub null: (),
    pub bool: bool,
    pub i64: i64,
    pub f64: f64,
    pub u32: u32,
    pub bytes: BytesRef,
    pub string: StringRef,
}

#[repr(C)]
struct ConnectOptions {
    pub path: StringRef,
}

#[unsafe(no_mangle)]
extern "C" fn df_connect(options: ConnectOptions, error: *mut ErrorMessage) -> *mut Connection {
    let call = || {
        let path = options.path.as_str();
        let conn = RUNTIME.block_on(Connection::connect(path))?;
        Ok(Box::into_raw(Box::new(conn)))
    };
    call()
        .map_err(|err| unsafe {
            *error = ErrorMessage::new(err);
        })
        .unwrap_or(null_mut())
}

#[unsafe(no_mangle)]
extern "C" fn df_close(conn: *mut Connection) {
    let conn = unsafe { Box::from_raw(conn) };
    let _ = RUNTIME.block_on(conn.close());
}

#[unsafe(no_mangle)]
extern "C" fn df_execute(conn: *mut Connection, sql: StringRef, error: *mut ErrorMessage) {
    let call = || {
        let conn = unsafe { &*conn };
        RUNTIME.block_on(conn.execute(sql.as_str()))?;
        Ok(())
    };
    if let Err(err) = call() {
        unsafe { *error = ErrorMessage::new(err) }
    }
}

#[unsafe(no_mangle)]
extern "C" fn df_execute_batch(conn: *mut Connection, sql: StringRef, error: *mut ErrorMessage) {
    let call = || {
        let conn = unsafe { &*conn };
        RUNTIME.block_on(conn.execute_batch(sql.as_str()))?;
        Ok(())
    };
    if let Err(err) = call() {
        unsafe { *error = ErrorMessage::new(err) }
    }
}

#[unsafe(no_mangle)]
extern "C" fn df_transaction(
    conn: *mut Connection,
    sqls: *const StringRef,
    sqls_len: usize,
    error: *mut ErrorMessage,
) {
    let sqls = unsafe { std::slice::from_raw_parts(sqls, sqls_len) };
    let call = || {
        let conn = unsafe { &*conn };
        let sqls = sqls.iter().map(|sql| sql.as_str()).collect::<Vec<_>>();
        RUNTIME.block_on(conn.transaction(&sqls))?;
        Ok(())
    };
    if let Err(err) = call() {
        unsafe { *error = ErrorMessage::new(err) }
    }
}

#[unsafe(no_mangle)]
extern "C" fn df_query(
    conn: *mut Connection,
    sql: StringRef,
    error: *mut ErrorMessage,
) -> *mut Query {
    let call = || {
        let conn = unsafe { &*conn };
        RUNTIME.block_on(conn.query(sql.as_str()))
    };
    call()
        .map(|query| Box::into_raw(Box::new(query)))
        .map_err(|err| unsafe {
            *error = ErrorMessage::new(err);
        })
        .unwrap_or(null_mut())
}

#[unsafe(no_mangle)]
extern "C" fn df_query_meta(query: *mut Query) -> Meta {
    unsafe {
        let query = &*query;
        Meta {
            column_count: query.columns.len(),
            row_count: query.rows.len(),
            rows_affected: query.rows_affected,
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn df_query_column(query: *mut Query, index: usize) -> Column {
    unsafe {
        let column = &(&*query).columns[index];
        Column {
            name: StringRef::new(&column.name),
            datatype: StringRef::new(&column.datatype),
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn df_query_value(
    query: *mut Query,
    row: usize,
    col: usize,
) -> TypedValue<DataKind, Data> {
    unsafe {
        match &(&*query).rows[row][col] {
            QueryValue::Null => TypedValue::new(DataKind::Null, Data { null: () }),
            QueryValue::Bool(value) => TypedValue::new(DataKind::Bool, Data { bool: *value }),
            QueryValue::I64(value) => TypedValue::new(DataKind::I64, Data { i64: *value }),
            QueryValue::F64(value) => TypedValue::new(DataKind::F64, Data { f64: *value }),
            QueryValue::U32(value) => TypedValue::new(DataKind::U32, Data { u32: *value }),
            QueryValue::Bytes(value) => TypedValue::new(
                DataKind::Bytes,
                Data {
                    bytes: BytesRef::new(value),
                },
            ),
            QueryValue::String(value) => TypedValue::new(
                DataKind::String,
                Data {
                    string: StringRef::new(value),
                },
            ),
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn df_free_query(query: *mut Query) {
    unsafe {
        let _ = Box::from_raw(query);
    }
}

#[unsafe(no_mangle)]
extern "C" fn df_free_error(error: ErrorMessage) {
    error.free();
}

// pglite-rs can only boot once in a process, so run this test module separately:
// cargo test query_columns_and_values
#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn query_columns_and_values() {
        let path =
            std::env::temp_dir().join(format!("dataflare-libpglite-ffi-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let path = path.to_string_lossy().to_string();

        let mut error = ErrorMessage::null();
        let conn = df_connect(
            ConnectOptions {
                path: StringRef::new(&path),
            },
            &mut error,
        );
        assert!(error.is_null());
        assert!(!conn.is_null());

        let query = df_query(
            conn,
            StringRef::new(
                "SELECT
                    NULL::text AS empty,
                    true AS ok,
                    1::int4 AS id,
                    3.14::float8 AS score,
                    decode('6869', 'hex') AS payload,
                    'alice'::text AS name,
                    42::oid AS oid",
            ),
            &mut error,
        );
        assert!(error.is_null());
        assert!(!query.is_null());

        let meta = df_query_meta(query);
        assert_eq!(meta.column_count, 7);
        assert_eq!(meta.row_count, 1);
        assert_eq!(meta.rows_affected, 0);

        assert_column(query, 0, "empty", "text");
        assert_column(query, 1, "ok", "bool");
        assert_column(query, 2, "id", "int4");
        assert_column(query, 3, "score", "float8");
        assert_column(query, 4, "payload", "bytea");
        assert_column(query, 5, "name", "text");
        assert_column(query, 6, "oid", "oid");

        assert_eq!(df_query_value(query, 0, 0).kind, DataKind::Null);
        unsafe {
            let ok = df_query_value(query, 0, 1);
            assert_eq!(ok.kind, DataKind::Bool);
            assert!(ok.value.bool);

            let id = df_query_value(query, 0, 2);
            assert_eq!(id.kind, DataKind::I64);
            assert_eq!(id.value.i64, 1);

            let score = df_query_value(query, 0, 3);
            assert_eq!(score.kind, DataKind::F64);
            assert_eq!(score.value.f64, 3.14);

            let payload = df_query_value(query, 0, 4);
            assert_eq!(payload.kind, DataKind::Bytes);
            assert_eq!(payload.value.bytes.as_bytes(), b"hi");

            let name = df_query_value(query, 0, 5);
            assert_eq!(name.kind, DataKind::String);
            assert_eq!(name.value.string.as_str(), "alice");

            let oid = df_query_value(query, 0, 6);
            assert_eq!(oid.kind, DataKind::U32);
            assert_eq!(oid.value.u32, 42);
        }

        df_free_query(query);
        df_close(conn);
        let _ = std::fs::remove_dir_all(&path);
    }

    fn assert_column(query: *mut Query, index: usize, name: &str, datatype: &str) {
        let column = df_query_column(query, index);
        assert_eq!(column.name.as_str(), name);
        assert_eq!(column.datatype.as_str(), datatype);
    }
}
