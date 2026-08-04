use crate::utils::RowsExt;
use crate::{
    ChunkInsert, ConnectionInfo, Database, Error, LOCALHOST, PostgresConfig, QuestDbConfig, Result,
    Value,
};
use bytes::Bytes;
use pgsq::{
    Column, ColumnType, Config, Connection, DecodeError, Decoder, Response, Type, load_client_cert,
    load_root_cert,
};
use query::{Query, QueryColumn, QueryValueExt};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

const POSTGRES_DEFAULT_PORT: u16 = 5432;
const COCKROACH_DEFAULT_PORT: u16 = 26257;
const QUESTDB_DEFAULT_PORT: u16 = 8812;

#[derive(Debug, Clone, Copy)]
pub enum Flavor {
    PostgreSql,
    CockroachDb,
    QuestDb,
}

#[derive(Debug, Clone)]
pub struct PostgresConnection {
    conn: Arc<Mutex<Connection>>,
    flavor: Flavor,
}

impl PostgresConnection {
    pub(crate) async fn test_postgres(config: PostgresConfig) -> Result<Option<String>> {
        let config = Self::make_postgres_config(config, POSTGRES_DEFAULT_PORT)?;
        Self::test(config, Flavor::PostgreSql).await
    }

    pub(crate) async fn test_cockroachdb(config: PostgresConfig) -> Result<Option<String>> {
        let config = Self::make_postgres_config(config, COCKROACH_DEFAULT_PORT)?;
        Self::test(config, Flavor::CockroachDb).await
    }

    pub(crate) async fn test_questdb(config: QuestDbConfig) -> Result<Option<String>> {
        let config = Self::make_questdb_config(config)?;
        Self::test(config, Flavor::QuestDb).await
    }

    async fn test(config: Config, flavor: Flavor) -> Result<Option<String>> {
        let conn = Self {
            conn: Arc::new(Mutex::new(Connection::connect(config).await?)),
            flavor,
        };
        let sql = match flavor {
            Flavor::QuestDb => "SELECT build();",
            Flavor::PostgreSql | Flavor::CockroachDb => "SELECT version();",
        };
        let version = conn
            .select(sql.into())
            .await?
            .first_cell_string()
            .map(Some)?;
        Ok(version)
    }

    pub(crate) async fn connect_postgres(config: PostgresConfig) -> Result<Database> {
        let config = Self::make_postgres_config(config, POSTGRES_DEFAULT_PORT)?;
        Self::connect(config, Flavor::PostgreSql).await
    }

    pub(crate) async fn connect_cockroachdb(config: PostgresConfig) -> Result<Database> {
        let config = Self::make_postgres_config(config, COCKROACH_DEFAULT_PORT)?;
        Self::connect(config, Flavor::CockroachDb).await
    }

    pub(crate) async fn connect_questdb(config: QuestDbConfig) -> Result<Database> {
        let config = Self::make_questdb_config(config)?;
        Self::connect(config, Flavor::QuestDb).await
    }

    async fn connect(config: Config, flavor: Flavor) -> Result<Database> {
        Ok(Database::Postgres(Self {
            conn: Arc::new(Mutex::new(Connection::connect(config).await?)),
            flavor,
        }))
    }

    fn make_postgres_config(config: PostgresConfig, default_port: u16) -> Result<Config> {
        let mut pc = Config {
            username: config.user,
            password: Some(config.password),
            host: config.host.unwrap_or(LOCALHOST.into()),
            port: config.port.unwrap_or(default_port),
            database: config.database,
            initial_sql: config.initial,
            application_name: Some("Dataflare".into()),
            tls_mode: config.tls.mode.into(),
            proxy: config.proxy,
            ..Default::default()
        };
        if let (Some(cert), Some(key)) = (config.tls.config.cert, config.tls.config.key) {
            pc.tls_client_cert = Some(load_client_cert(&cert, &key)?);
        }
        if let Some(ca) = config.tls.config.ca {
            pc.tls_root_cert = Some(load_root_cert(&ca)?);
        }
        Ok(pc)
    }

    fn make_questdb_config(config: QuestDbConfig) -> Result<Config> {
        let mut pc = Config {
            username: config.user,
            password: Some(config.password),
            host: config.host.unwrap_or(LOCALHOST.into()),
            port: config.port.unwrap_or(QUESTDB_DEFAULT_PORT),
            database: "".into(),
            application_name: Some("Dataflare".into()),
            tls_mode: config.tls.mode.into(),
            proxy: config.proxy,
            ..Default::default()
        };
        if let (Some(cert), Some(key)) = (config.tls.config.cert, config.tls.config.key) {
            pc.tls_client_cert = Some(load_client_cert(&cert, &key)?);
        }
        if let Some(ca) = config.tls.config.ca {
            pc.tls_root_cert = Some(load_root_cert(&ca)?);
        }
        Ok(pc)
    }

    pub(crate) async fn info(&self) -> Result<ConnectionInfo> {
        let name = match self.flavor {
            Flavor::PostgreSql => "PostgreSQL",
            Flavor::CockroachDb => "CockroachDB",
            Flavor::QuestDb => "QuestDB",
        };
        let mut info = ConnectionInfo::new(name);
        {
            let conn = self.conn.lock().await;
            info.push_server("wire", &conn.config.host, conn.config.port);
        }
        match self.flavor {
            Flavor::PostgreSql => {
                let [user, database, schema, timezone, version, tls] = self
                    .select(
                        r#"SELECT current_user, current_database(), COALESCE(current_schema(), ''), current_setting('TimeZone'), version(), COALESCE((SELECT version || ', ' || cipher || ' (' || bits::text || ' bits)' FROM pg_stat_ssl WHERE pid = pg_backend_pid()), 'None')"#.into(),
                    )
                    .await?
                    .first_row_strings::<6>()?;
                info.push_text("User", user);
                info.push_text("Database", database);
                info.push_text("Schema", schema);
                info.push_text("Timezone", timezone);
                info.push_text("TLS", tls);
                info.push_text("Version", version);
            }
            Flavor::CockroachDb => {
                let [user, database, schema, timezone, version] = self
                    .select(
                        r#"SELECT current_user, current_database(), COALESCE(current_schema(), ''), current_setting('TimeZone'), version()"#.into(),
                    )
                    .await?
                    .first_row_strings::<5>()?;
                info.push_text("User", user);
                info.push_text("Database", database);
                info.push_text("Schema", schema);
                info.push_text("Timezone", timezone);
                info.push_text("Version", version);
            }
            Flavor::QuestDb => {
                let [user, version] = self
                    .select("SELECT current_user(), build();".into())
                    .await?
                    .first_row_strings::<2>()?;
                info.push_text("User", user);
                info.push_text("Version", version);
            }
        }
        Ok(info)
    }

    pub(crate) async fn select(&self, sql: String) -> Result<Vec<Vec<Value>>> {
        let resps = { self.conn.lock().await.query(sql).await? };
        let resp = resps.into_iter().find_map(|resp| match resp {
            Response::Query(columns, rows) => Some((columns, rows)),
            _ => None,
        });
        let (columns, rows) = match resp {
            Some((column, rows)) => (column, rows),
            _ => return Ok(vec![]),
        };
        let mut rows_value = Vec::with_capacity(rows.len());
        for row in rows {
            let mut row_value = Vec::with_capacity(row.len());
            for (i, cell) in row.into_iter().enumerate() {
                row_value.push(decode_value(&columns[i], cell)?);
            }
            rows_value.push(row_value);
        }
        Ok(rows_value)
    }

    pub(crate) async fn execute(&self, sql: String) -> Result<()> {
        self.conn.lock().await.query(sql).await?;
        Ok(())
    }

    pub(crate) async fn transaction(&self, sqls: Vec<String>) -> Result<()> {
        self.conn.lock().await.query(sqls.join("")).await?;
        Ok(())
    }

    pub(crate) async fn query(&self, sql: String) -> Result<Query> {
        let mut query = Query::default();
        let resps = {
            let mut conn = self.conn.lock().await;
            let now = Instant::now();
            let resp = conn.query(sql).await?;
            query.duration = now.elapsed().as_millis() as u32;
            resp
        };
        query.rows_affected = Some(
            resps
                .iter()
                .filter_map(|resp| match resp {
                    Response::CommandComplete(n) => Some(n),
                    Response::Query(..) => None,
                })
                .sum(),
        );
        let resp = resps.into_iter().find_map(|resp| match resp {
            Response::Query(columns, rows) => Some((columns, rows)),
            _ => None,
        });
        let (columns, rows) = match resp {
            Some((column, rows)) => (column, rows),
            _ => return Ok(query),
        };
        let mut rows_value = Vec::with_capacity(rows.len());
        for row in rows {
            let mut row_value = Vec::with_capacity(row.len());
            for (i, cell) in row.into_iter().enumerate() {
                row_value.push(decode_value(&columns[i], cell)?);
            }
            rows_value.push(row_value);
        }
        query.rows = rows_value;
        query.columns = columns
            .into_iter()
            .map(|col| QueryColumn {
                name: col.name,
                datatype: col.datatype.into(),
            })
            .collect::<Vec<_>>();
        Ok(query)
    }

    // TODO: Consider opening a new connection pool for faster batch insert
    pub(crate) async fn batch_insert(&self, insert: ChunkInsert) -> Result<()> {
        let config = { self.conn.lock().await.config.clone() };
        let mut conn = Connection::connect(config).await?;
        for sql in insert {
            conn.raw_query(sql).await?;
        }
        Ok(())
    }
}

fn decode_value(col: &Column, val: Option<Bytes>) -> Result<Value, Error> {
    let bytes = match val {
        Some(v) => v,
        None => return Ok(Value::Null),
    };
    // https://www.postgresql.org/docs/current/protocol-flow.html
    // In simple Query mode, the format of retrieved values is always text, except when the given command is a FETCH from a cursor declared with the BINARY option. In that case, the retrieved values are in binary format. The format codes given in the RowDescription message tell which format is being used.
    if !col.text_format {
        return Err(Error::PostgresDecoder(DecodeError::UnsupportedFormat));
    }
    let val = match col.datatype {
        ColumnType::BuiltIn(Type::int2) => Value::I16(Decoder::decode(&bytes)?),
        ColumnType::BuiltIn(Type::int4) => Value::I32(Decoder::decode(&bytes)?),
        ColumnType::BuiltIn(Type::int8) => Value::I64(Decoder::decode(&bytes)?),
        ColumnType::BuiltIn(Type::float4) => Value::F32(Decoder::decode(&bytes)?),
        ColumnType::BuiltIn(Type::float8) => Value::F64(Decoder::decode(&bytes)?),
        ColumnType::BuiltIn(Type::oid) => Value::U32(Decoder::decode(&bytes)?),
        ColumnType::BuiltIn(Type::bool) => Value::Bool(Decoder::decode(&bytes)?),
        ColumnType::BuiltIn(Type::bytea) => Value::from_bytes(Decoder::decode(&bytes)?),
        // JSONB is always in a compressed format; we prettify it here. For JSON we leave it as-is since it's stored verbatim
        ColumnType::BuiltIn(Type::jsonb) => {
            Value::pretty_json(Decoder::<serde_json::Value>::decode(&bytes)?)
        }
        _ => Value::String(Decoder::decode(&bytes)?),
    };
    Ok(val)
}
