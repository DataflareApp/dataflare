use crate::utils::FirstCell;
use crate::{ChDbConfig, ChunkInsert, Database, Result, Value};
use chdb::Connection;
use query::Query;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ChDbConnection {
    conn: Arc<Connection>,
}

impl ChDbConnection {
    async fn conn(config: ChDbConfig) -> Result<Connection> {
        let conn = Connection::connect(&config.path).await?;
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

    // TODO
    pub(crate) async fn batch_insert(&self, insert: ChunkInsert) -> Result<()> {
        for sql in insert {
            self.conn.execute(&sql)?;
        }
        Ok(())
    }
}
