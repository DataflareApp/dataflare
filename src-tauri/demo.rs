use crate::client::{Client, Error};
use database::{ConnectionConfig, SqliteConfig};
use std::path::Path;
use tokio::fs;

#[cfg(debug_assertions)]
const DATABASE: &[u8] = &[];  
#[cfg(not(debug_assertions))]
const DATABASE: &[u8] = include_bytes!("assets/chinook.db");

const QUERY: &str = include_str!("assets/query.sql");

const DATABASE_FILE: &str = "chinook.db";
const CONNECTION_NAME: &str = "Demo";
const QUERY_NAME: &str = "Getting Started";

pub async fn initialize(client: &Client, client_database_path: &Path) -> Result<(), Error> {
    let path = client_database_path.with_file_name(DATABASE_FILE);
    fs::write(&path, DATABASE).await?;

    let config = ConnectionConfig::SQLite(SqliteConfig {
        path: path.display().to_string(),
        readonly: false,
        initial: None,
    });

    let cid = client
        .create_connection(CONNECTION_NAME.into(), config)
        .await?;
    client
        .create_query(cid, QUERY_NAME.into(), QUERY.into())
        .await?;

    Ok(())
}
