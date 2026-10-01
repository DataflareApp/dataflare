use crate::{BasinSqlConfig, ChunkInsert, ConnectionInfo, Database, Result, Value};
use query::Query;
use r2sql::{Connection, Error};

#[derive(Debug, Clone)]
pub struct BasinSqlConnection {
    conn: Connection,
}

impl BasinSqlConnection {
    pub(crate) async fn test(config: BasinSqlConfig) -> Result<Option<String>> {
        let conn = Connection::new(config.account_id, config.bucket_name, config.api_token)?;
        conn.query("SHOW NAMESPACES;".into()).await?;
        Ok(None)
    }

    pub(crate) async fn connect(config: BasinSqlConfig) -> Result<Database> {
        let conn = Connection::new(config.account_id, config.bucket_name, config.api_token)?;
        Ok(Database::BasinSql(Self { conn }))
    }

    pub(crate) async fn info(&self) -> Result<ConnectionInfo> {
        let url = self.conn.api_url();
        let mut info = ConnectionInfo::new("Basin SQL");
        info.push_server(
            url.scheme(),
            url.host_str().unwrap_or_default(),
            url.port_or_known_default().unwrap_or_default(),
        );
        info.push_text("Account ID", self.conn.account_id());
        info.push_text("Bucket", self.conn.bucket_name());
        info.push_url("Dashboard", self.conn.dashboard_url());
        Ok(info)
    }

    pub(crate) async fn query(&self, sql: String) -> Result<Query> {
        let query = self.conn.query(sql).await?;
        Ok(query)
    }

    pub(crate) async fn select(&self, sql: String) -> Result<Vec<Vec<Value>>> {
        let rows = self.conn.query(sql).await?.rows;
        Ok(rows)
    }

    fn readonly_error() -> Result<()> {
        Err(Error::Message("Basin SQL is read-only.".into()).into())
    }

    pub(crate) async fn execute(&self, _: String) -> Result<()> {
        Self::readonly_error()
    }

    pub(crate) async fn transaction(&self, _: Vec<String>) -> Result<()> {
        Self::readonly_error()
    }

    pub(crate) async fn batch_insert(&self, _: ChunkInsert) -> Result<()> {
        Self::readonly_error()
    }
}
