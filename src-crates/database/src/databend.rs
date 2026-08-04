use crate::utils::{RowsExt, empty_if, unordered_tasks};
use crate::{
    ChunkInsert, ConnectProtocol, ConnectionInfo, Database, DatabendConfig, Error, LOCALHOST,
    Result, Value,
};
use databend::{Config, Connection};
use futures_util::FutureExt;
use query::Query;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct DatabendConnection {
    conn: Arc<Mutex<Connection>>,
}

impl DatabendConnection {
    pub(crate) async fn test(config: DatabendConfig) -> Result<Option<String>> {
        let config = Self::make_options(config);
        let mut conn = Connection::open_with(config)?;
        conn.query("SELECT version();")
            .await?
            .rows
            .first_cell_string()
            .map(Some)
    }

    pub(crate) async fn connect(config: DatabendConfig) -> Result<Database> {
        let config = Self::make_options(config);
        Ok(Database::Databend(Self {
            conn: Arc::new(Mutex::new(Connection::open_with(config)?)),
        }))
    }

    fn make_options(config: DatabendConfig) -> Config {
        let host = empty_if(config.host, LOCALHOST);
        let port = match config.port {
            Some(port) => port,
            None => match config.protocol {
                ConnectProtocol::Http => 8000,
                ConnectProtocol::Https => 443,
            },
        };
        let username = empty_if(config.user, "root");
        let database = empty_if(config.database, "default");
        Config {
            https: config.protocol == ConnectProtocol::Https,
            host,
            port,
            username,
            password: config.password,
            database,
            proxy: config.proxy,
        }
    }

    pub(crate) async fn info(&self) -> Result<ConnectionInfo> {
        let mut conn = self.conn.lock().await;
        let [user, catalog, database, version, timezone] = conn
            .query(
                r#"SELECT
                    current_user(),
                    current_catalog(),
                    database(),
                    version(),
                    timezone()
                FROM system.databases
                WHERE catalog = current_catalog()
                  AND name = database()
                LIMIT 1;"#,
            )
            .await?
            .rows
            .first_row_strings::<5>()?;
        let url = conn.url();
        let mut info = ConnectionInfo::new("Databend");
        info.push_server(
            url.scheme(),
            url.host_str().unwrap_or_default(),
            url.port_or_known_default().unwrap_or_default(),
        );
        info.push_text("User", user);
        info.push_text("Catalog", catalog);
        info.push_text("Database", database);
        info.push_text("Timezone", timezone);
        info.push_text("Version", version);
        Ok(info)
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
        let mut conn = self.conn.lock().await;
        conn.query("BEGIN").await?;
        for sql in sqls {
            if let Err(err) = conn.query(&sql).await {
                conn.query("ROLLBACK").await?;
                return Err(Error::Databend(err));
            }
        }
        conn.query("COMMIT").await?;
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
