use crate::utils::{RowsExt, empty_if, unordered_tasks};
use crate::{ChunkInsert, ConnectionInfo, Database, LOCALHOST, Result, TrinoConfig, Value};
use connection_config::{ConnectProtocol, TrinoAuth};
use futures_util::FutureExt;
use query::Query;
use std::sync::Arc;
use tokio::sync::Mutex;
use trino::{AuthConfig, Config, Connection};

#[derive(Debug, Clone)]
pub struct TrinoConnection {
    conn: Arc<Mutex<Connection>>,
}

impl TrinoConnection {
    pub(crate) async fn info(&self) -> Result<ConnectionInfo> {
        let mut conn = self.conn.lock().await;
        let [user, catalog, schema, timezone, connector, version, nodes] = conn
            .query(
                r#"
                    SELECT
                        current_user,
                        COALESCE(current_catalog, ''),
                        COALESCE(current_schema, ''),
                        current_timezone(),
                        COALESCE((
                            SELECT connector_name
                            FROM system.metadata.catalogs
                            WHERE catalog_name = current_catalog
                        ), ''),
                        version(),
                        CAST((SELECT count(*) FROM system.runtime.nodes) AS VARCHAR)
                "#,
            )
            .await?
            .rows
            .first_row_strings::<7>()?;
        let url = conn.url();
        let mut info = ConnectionInfo::new("Trino");
        info.push_server(
            url.scheme(),
            url.host_str().unwrap_or_default(),
            url.port_or_known_default().unwrap_or_default(),
        );
        info.push_text("User", user);
        info.push_text("Catalog", catalog);
        info.push_text("Schema", schema);
        info.push_text("Connector", connector);
        info.push_text("Timezone", timezone);
        info.push_text("Nodes", nodes);
        info.push_text("Version", version);
        Ok(info)
    }

    pub(crate) async fn test(config: TrinoConfig) -> Result<Option<String>> {
        let mut conn = Self::make_conn(config).await?;
        conn.query("SELECT concat('Trino version: ', version())")
            .await?
            .rows
            .first_cell_string()
            .map(Some)
    }

    pub(crate) async fn connect(config: TrinoConfig) -> Result<Database> {
        let conn = Self::make_conn(config).await?;
        Ok(Database::Trino(Self {
            conn: Arc::new(Mutex::new(conn)),
        }))
    }

    async fn make_conn(config: TrinoConfig) -> Result<Connection> {
        let port = match config.port {
            Some(port) => port,
            None => match config.protocol {
                ConnectProtocol::Http => 8080,
                ConnectProtocol::Https => 443,
            },
        };
        let config = Config {
            https: config.protocol == ConnectProtocol::Https,
            host: empty_if(config.host, LOCALHOST),
            port,
            user: config.user,
            auth: match config.auth {
                TrinoAuth::None => AuthConfig::None,
                TrinoAuth::Password { password } => AuthConfig::Password { password },
                TrinoAuth::Jwt { token } => AuthConfig::Jwt { token },
            },
            catalog: config.catalog,
            schema: config.schema,
            allow_invalid_certs: config.allow_invalid_certs,
            proxy: config.proxy,
        };
        let conn = Connection::open_with(config)?;
        Ok(conn)
    }

    pub(crate) async fn select(&self, sql: String) -> Result<Vec<Vec<Value>>> {
        let rows = self.conn.lock().await.query(sql).await?.rows;
        Ok(rows)
    }

    pub(crate) async fn execute(&self, sql: String) -> Result<()> {
        self.conn.lock().await.query(sql).await?;
        Ok(())
    }

    pub(crate) async fn transaction(&self, sqls: Vec<String>) -> Result<()> {
        self.execute("START TRANSACTION;".into()).await?;
        for sql in sqls {
            if let Err(err) = self.execute(sql).await {
                self.execute("ROLLBACK;".into()).await?;
                return Err(err);
            }
        }
        self.execute("COMMIT;".into()).await?;
        Ok(())
    }

    pub(crate) async fn query(&self, sql: String) -> Result<Query> {
        let query = self.conn.lock().await.query(sql).await?;
        Ok(query)
    }

    pub(crate) async fn batch_insert(&self, insert: ChunkInsert) -> Result<()> {
        let conn = { self.conn.lock().await.snapshot() };
        unordered_tasks(100, insert, |sql| {
            let mut conn = conn.snapshot();
            async move { conn.query(sql).await.map(|_| ()) }.boxed()
        })
        .await?;
        Ok(())
    }
}
