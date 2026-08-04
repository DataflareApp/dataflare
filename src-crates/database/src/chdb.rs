use crate::utils::RowsExt;
use crate::{ChDbConfig, ChunkInsert, ConnectionInfo, Database, Result, Value};
use chdb::Connection;
use query::Query;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ChDbConnection {
    conn: Arc<Connection>,
}

impl ChDbConnection {
    async fn conn(config: ChDbConfig) -> Result<Connection> {
        let conn = Connection::connect(&config.path, &config.database).await?;
        if let Some(sql) = config.initial {
            conn.execute(&sql)?;
        }
        Ok(conn)
    }

    pub(crate) async fn test(config: ChDbConfig) -> Result<Option<String>> {
        Self::conn(config)
            .await?
            .query("SELECT concat('chDB version: ', version());")?
            .rows
            .first_cell_string()
            .map(Some)
    }

    pub(crate) async fn info(&self) -> Result<ConnectionInfo> {
        let [database, version] = self
            .conn
            .query("SELECT currentDatabase(), version();")?
            .rows
            .first_row_strings::<2>()?;
        let mut info = ConnectionInfo::new("chDB");
        let path = self.conn.path();
        info.push_db_path(path);
        info.push_text("Version", version);
        info.push_text("Database", database);
        Ok(info)
    }

    pub(crate) async fn connect(config: ChDbConfig) -> Result<Database> {
        Ok(Database::ChDb(Self {
            conn: Arc::new(Self::conn(config).await?),
        }))
    }

    pub(crate) async fn select(&self, sql: String) -> Result<Vec<Vec<Value>>> {
        let query = self.conn.query(&sql)?;
        Ok(query.rows)
    }

    pub(crate) async fn execute(&self, sql: String) -> Result<()> {
        self.conn.execute(&sql)?;
        Ok(())
    }

    // TODO
    pub(crate) async fn transaction(&self, sqls: Vec<String>) -> Result<()> {
        for sql in sqls {
            self.conn.execute(&sql)?;
        }
        Ok(())
    }

    pub(crate) async fn query(&self, sql: String) -> Result<Query> {
        let query = self.conn.query(&sql)?;
        Ok(query)
    }

    pub(crate) async fn batch_insert(&self, insert: ChunkInsert) -> Result<()> {
        for sql in insert {
            self.conn.execute(&sql)?;
        }
        Ok(())
    }
}
