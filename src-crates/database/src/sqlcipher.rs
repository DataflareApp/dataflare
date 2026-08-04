use crate::utils::{RowsExt, format_bytes};
use crate::{ChunkInsert, ConnectionInfo, Database, Result, SqlCipherConfig};
use query::{Query, Value};
use sqlcipher::Connection;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct SqlCipherConnection {
    conn: Arc<Connection>,
}

impl SqlCipherConnection {
    pub(crate) async fn test(config: SqlCipherConfig) -> Result<Option<String>> {
        let conn =
            Self::make_connection(config.path, config.readonly, config.initial, config.key).await?;
        // Ensure it's a valid database
        conn.select("SELECT * FROM sqlite_master LIMIT 0;")?;
        let sqlite = conn
            .select("SELECT sqlite_version();")?
            .first_cell_string()?;
        let cipher = conn.select("PRAGMA cipher_version;")?.first_cell_string()?;
        Ok(Some(format!("SQLite: {sqlite}\nSQLCipher: {cipher}")))
    }

    pub(crate) async fn connect(config: SqlCipherConfig) -> Result<Database> {
        let conn =
            Self::make_connection(config.path, config.readonly, config.initial, config.key).await?;
        Ok(Database::SqlCipher(Self {
            conn: Arc::new(conn),
        }))
    }

    async fn make_connection(
        path: String,
        readonly: bool,
        initial: Option<String>,
        key: String,
    ) -> Result<Connection> {
        let conn = Connection::connect(&path, readonly, &key).await?;
        if let Some(sql) = initial {
            conn.execute_batch(&sql)?;
        }
        Ok(conn)
    }

    pub(crate) async fn info(&self) -> Result<ConnectionInfo> {
        let mut info = ConnectionInfo::new("SQLCipher");
        let [path, sqlite_version, database_size] = self
            .conn
            .select("SELECT file, sqlite_version(), CAST(page_count * page_size AS TEXT) FROM pragma_database_list, pragma_page_count('main'), pragma_page_size('main') WHERE name = 'main';")?
            .first_row_strings::<3>()?;
        info.push_db_path(path);
        info.push_text("Size", format_bytes(database_size)?);
        info.push_text("SQLite", sqlite_version);
        let sqlcipher_version = self
            .conn
            .select("PRAGMA cipher_version;")?
            .first_cell_string()?;
        info.push_text("SQLCipher", sqlcipher_version);
        let cipher = self
            .conn
            .select("PRAGMA cipher;")?
            .first_cell_string_optional()?
            .unwrap_or_else(|| "None".into());
        info.push_text("Cipher", cipher);
        Ok(info)
    }

    pub(crate) async fn select(&self, sql: String) -> Result<Vec<Vec<Value>>> {
        let rows = self.conn.select(&sql)?;
        Ok(rows)
    }

    pub(crate) async fn execute(&self, sql: String) -> Result<()> {
        self.conn.execute(&sql)?;
        Ok(())
    }

    pub(crate) async fn transaction(&self, sqls: Vec<String>) -> Result<()> {
        self.conn.transaction(&sqls)?;
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
