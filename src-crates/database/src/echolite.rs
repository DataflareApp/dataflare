use crate::utils::{RowsExt, format_bytes};
use crate::{ChunkInsert, ConnectionInfo, Database, EchoLiteConfig, LOCALHOST, Result};
use echolite::{
    Connection, DEFAULT_SERVER_PORT, Error as EchoError, Flags, ProtocolError, SQLITE_OPEN_CREATE,
    SQLITE_OPEN_READONLY, SQLITE_OPEN_READWRITE, Value as EchoValue,
};
use proxy::{ProxyConfig, SshStream};
use query::{Query, QueryColumn, Value};
use std::fmt::Debug;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ErrorKind, ReadBuf, Result as IoResult};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct EchoLiteConnection {
    config: Arc<EchoLiteConfig>,
    conn: Arc<Mutex<Connection<Stream>>>,
    reconnecting: Arc<AtomicBool>,
}

impl Drop for EchoLiteConnection {
    fn drop(&mut self) {
        if Arc::get_mut(&mut self.conn).is_some() {
            let conn = self.conn.clone();
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    let _ = conn.lock().await.disconnect().await;
                });
            }
        }
    }
}

impl EchoLiteConnection {
    // From sqlite.rs
    pub(crate) async fn test(config: EchoLiteConfig) -> Result<Option<String>> {
        let mut conn = Self::make_connection(&config).await?;
        conn.query("SELECT * FROM sqlite_master LIMIT 0;").await?;

        let echolite_version = conn.version();
        let query = conn.query("SELECT sqlite_version();").await?;

        conn.disconnect().await?;

        let sqlite_version =
            convert_to_rows(query.values, query.columns.len())?.first_cell_string()?;

        Ok(Some(format!(
            "EchoLite version: {echolite_version}\nSQLite version: {sqlite_version}"
        )))
    }

    pub(crate) async fn connect(config: EchoLiteConfig) -> Result<Database> {
        let conn = Self::make_connection(&config).await?;
        Ok(Database::EchoLite(Self {
            config: Arc::new(config),
            conn: Arc::new(Mutex::new(conn)),
            reconnecting: Arc::new(AtomicBool::new(false)),
        }))
    }

    async fn make_connection(config: &EchoLiteConfig) -> Result<Connection<Stream>> {
        let host = config
            .host
            .as_ref()
            .cloned()
            .unwrap_or_else(|| LOCALHOST.into());
        let port = config.port.unwrap_or(DEFAULT_SERVER_PORT);
        let mut flags = Flags::default();
        if config.readonly {
            flags.set(SQLITE_OPEN_READWRITE, false);
            flags.set(SQLITE_OPEN_CREATE, false);
            flags.set(SQLITE_OPEN_READONLY, true);
        }
        let stream = Stream::connect_stream(&host, port, &config.proxy).await?;
        let mut conn = Connection::connect(stream, &config.password, &config.path, flags).await?;
        if let Some(sql) = &config.initial {
            conn.execute(sql).await?;
        }
        Ok(conn)
    }

    pub(crate) async fn info(&self) -> Result<ConnectionInfo> {
        let [path, version, database_size] = self
            .select(
                "SELECT file, sqlite_version(), CAST(page_count * page_size AS TEXT) FROM pragma_database_list, pragma_page_count('main'), pragma_page_size('main') WHERE name = 'main';".into(),
            )
            .await?
            .first_row_strings::<3>()?;
        let mut info = ConnectionInfo::new("EchoLite");
        info.push_server(
            "tcp",
            self.config.host.as_deref().unwrap_or(LOCALHOST),
            self.config.port.unwrap_or(DEFAULT_SERVER_PORT),
        );
        info.push_db_path(path);
        info.push_text("Size", format_bytes(database_size)?);
        info.push_text("EchoLite", self.conn.lock().await.version());
        info.push_text("SQLite", version);
        Ok(info)
    }

    pub(crate) async fn select(&self, sql: String) -> Result<Vec<Vec<Value>>> {
        let rows = self.query(sql).await?.rows;
        Ok(rows)
    }

    pub(crate) async fn execute(&self, sql: String) -> Result<()> {
        self.conn
            .lock()
            .await
            .execute(&sql)
            .await
            .map_err(|err| self.maybe_reconnect(err))?;
        Ok(())
    }

    pub(crate) async fn query(&self, sql: String) -> Result<Query> {
        let query = self
            .conn
            .lock()
            .await
            .query(&sql)
            .await
            .map_err(|err| self.maybe_reconnect(err))?;
        let columns = query
            .columns
            .into_iter()
            .map(|c| QueryColumn {
                name: c.name,
                datatype: c.datatype,
            })
            .collect::<Vec<_>>();
        let rows = convert_to_rows(query.values, columns.len())?;
        Ok(Query {
            columns,
            rows,
            rows_affected: Some(query.rows_affected),
            duration: query.duration as u32,
        })
    }

    pub(crate) async fn transaction(&self, sqls: Vec<String>) -> Result<()> {
        self.conn
            .lock()
            .await
            .transaction(sqls)
            .await
            .map_err(|err| self.maybe_reconnect(err))?;
        Ok(())
    }

    pub(crate) async fn batch_insert(&self, insert: ChunkInsert) -> Result<()> {
        for sql in insert {
            self.execute(sql).await?;
        }
        Ok(())
    }

    fn maybe_reconnect(&self, err: EchoError) -> EchoError {
        let connect = match &err {
            EchoError::Protocol(err) => match err {
                ProtocolError::IoError(err) => match err.kind() {
                    ErrorKind::ConnectionReset
                    | ErrorKind::ConnectionAborted
                    | ErrorKind::BrokenPipe
                    | ErrorKind::UnexpectedEof
                    | ErrorKind::NetworkUnreachable
                    | ErrorKind::HostUnreachable
                    | ErrorKind::TimedOut
                    | ErrorKind::ConnectionRefused => true,
                    _ => false,
                },
                _ => false,
            },
            _ => false,
        };
        if connect {
            let conn = self.clone();
            tokio::spawn(async move {
                if conn.reconnecting.swap(true, Ordering::SeqCst) {
                    return;
                }
                let mut mutex = conn.conn.lock().await;
                match Self::make_connection(&conn.config).await {
                    Ok(new_conn) => {
                        *mutex = new_conn;
                    }
                    Err(_) => {}
                };
                conn.reconnecting.store(false, Ordering::SeqCst);
            });
        }
        err
    }
}

fn convert_to_rows(values: Vec<EchoValue>, col_count: usize) -> Result<Vec<Vec<Value>>> {
    if col_count == 0 || values.is_empty() {
        return Ok(Vec::new());
    }
    let mut rows = Vec::with_capacity(values.len() / col_count);
    for (i, v) in values.into_iter().enumerate() {
        if i % col_count == 0 {
            rows.push(Vec::with_capacity(col_count));
        }
        let v = match v {
            EchoValue::Null => Value::Null,
            EchoValue::I64(v) => Value::I64(v),
            EchoValue::F64(v) => Value::F64(v),
            EchoValue::Text(v) => {
                Value::String(String::from_utf8(v).map_err(|_| EchoError::InvalidUtf8)?)
            }
            EchoValue::Bytes(v) => Value::from_bytes(v),
        };
        unsafe { rows.last_mut().unwrap_unchecked() }.push(v);
    }
    Ok(rows)
}

enum Stream {
    Tcp(TcpStream),
    Ssh(SshStream),
}

impl Stream {
    async fn connect_stream(host: &str, port: u16, proxy: &Option<ProxyConfig>) -> Result<Stream> {
        if let Some(proxy) = proxy {
            let stream = proxy.connect_stream(host, port).await?;
            return Ok(Stream::Ssh(stream));
        }
        let stream = TcpStream::connect(endpoint::join(host, port)).await?;
        stream.set_nodelay(true)?;
        Ok(Stream::Tcp(stream))
    }
}

impl Debug for Stream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tcp(_) => write!(f, "TcpStream"),
            Self::Ssh(_) => write!(f, "SshStream"),
        }
    }
}

impl AsyncRead for Stream {
    #[inline]
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        match &mut self.get_mut() {
            Stream::Tcp(stream) => Pin::new(stream).poll_read(cx, buf),
            Stream::Ssh(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for Stream {
    #[inline]
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        match &mut self.get_mut() {
            Stream::Tcp(stream) => Pin::new(stream).poll_write(cx, buf),
            Stream::Ssh(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    #[inline]
    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        match &mut self.get_mut() {
            Stream::Tcp(stream) => Pin::new(stream).poll_flush(cx),
            Stream::Ssh(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    #[inline]
    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        match &mut self.get_mut() {
            Stream::Tcp(stream) => Pin::new(stream).poll_shutdown(cx),
            Stream::Ssh(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}
