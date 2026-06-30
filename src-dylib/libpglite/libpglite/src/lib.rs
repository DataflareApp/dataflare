#[path = "../../../../src-crates/dylib/src/ffi.rs"]
mod ffi;

mod decode;

use ffi::{BytesRef, ErrorMessage, StringRef, TypedValue};
use pglite::Connection;
use std::ptr::null_mut;

use crate::decode::to_query;

pub(crate) type Result<T, E = String> = std::result::Result<T, E>;

trait StringError<T> {
    fn string_err(self) -> Result<T>;
}

impl<T, E> StringError<T> for Result<T, E>
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct QueryColumn {
    pub(crate) name: String,
    pub(crate) datatype: String,
}

#[derive(Debug, PartialEq)]
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
        let conn = Connection::open_with(path).string_err()?;
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
    let _ = unsafe { Box::from_raw(conn) };
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
        let conn = unsafe { &mut *conn };
        let sqls = sqls.iter().map(|sql| sql.as_str()).collect::<Vec<_>>();
        conn.transaction(&sqls).string_err()?;
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
        let conn = unsafe { &mut *conn };
        conn.query(sql.as_str()).map(to_query).string_err()?
    };
    call()
        .map(|query| Box::into_raw(Box::new(query)))
        .map_err(|err| unsafe {
            *error = ErrorMessage::new(err);
        })
        .unwrap_or(null_mut())
}

#[unsafe(no_mangle)]
extern "C" fn df_execute(handle: *mut Connection, sql: StringRef, error: *mut ErrorMessage) {
    let call = || {
        let conn = unsafe { &mut *handle };
        let _ = conn.query(sql.as_str()).string_err()?;
        Ok(())
    };
    if let Err(err) = call() {
        unsafe { *error = ErrorMessage::new(err) }
    }
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

#[cfg(test)]
mod tests {
    use crate::*;

    macro_rules! assert_column {
        ($query:expr, $index:expr, $name:expr, $datatype:expr) => {{
            let column = df_query_column($query, $index);
            assert_eq!(column.name.as_str(), $name);
            assert_eq!(column.datatype.as_str(), $datatype);
        }};
    }

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
                    42::int4 AS integer_value,
                    3.5::float8 AS float_value,
                    true AS bool_value,
                    random() AS random_value,
                    TIMESTAMP '2025-01-02 03:04:05' AS timestamp_value",
            ),
            &mut error,
        );
        assert!(error.is_null());
        assert!(!query.is_null());

        let meta = df_query_meta(query);
        assert_eq!(meta.column_count, 5);
        assert_eq!(meta.row_count, 1);
        assert_eq!(meta.rows_affected, 0);

        assert_column!(query, 0, "integer_value", "int4");
        assert_column!(query, 1, "float_value", "float8");
        assert_column!(query, 2, "bool_value", "bool");
        assert_column!(query, 3, "random_value", "float8");
        assert_column!(query, 4, "timestamp_value", "timestamp");

        unsafe {
            let integer = df_query_value(query, 0, 0);
            assert_eq!(integer.kind, DataKind::I64);
            assert_eq!(integer.value.i64, 42);

            let float = df_query_value(query, 0, 1);
            assert_eq!(float.kind, DataKind::F64);
            assert_eq!(float.value.f64, 3.5);

            let boolean = df_query_value(query, 0, 2);
            assert_eq!(boolean.kind, DataKind::Bool);
            assert!(boolean.value.bool);

            let random = df_query_value(query, 0, 3);
            assert_eq!(random.kind, DataKind::F64);
            assert!((0.0..1.0).contains(&random.value.f64));

            let timestamp = df_query_value(query, 0, 4);
            assert_eq!(timestamp.kind, DataKind::String);
            assert_eq!(timestamp.value.string.as_str(), "2025-01-02 03:04:05");
        }

        df_free_query(query);
        df_close(conn);

        let _ = std::fs::remove_dir_all(&path);
    }
}
