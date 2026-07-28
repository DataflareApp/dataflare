use crate::client::{Client, Error, WidgetConfig};
use database::{ConnectionConfig, SqliteConfig};
use serde::Deserialize;
use std::path::Path;
use tokio::fs;

#[cfg(debug_assertions)]
const DATABASE: &[u8] = &[];
#[cfg(not(debug_assertions))]
const DATABASE: &[u8] = include_bytes!("assets/chinook.db");

const QUERY: &str = include_str!("assets/query.sql");
const DASHBOARD: &str = include_str!("assets/dashboard.json");

const DATABASE_FILE: &str = "chinook.db";
const CONNECTION_ID: &str = "00000000-0000-0000-0000-000000000000";
const CONNECTION_NAME: &str = "Demo Database";
const QUERY_NAME: &str = "Getting Started";

#[derive(Deserialize)]
struct DashboardWidget {
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    config: WidgetConfig,
}

pub async fn initialize(client: &Client, client_database_path: &Path) -> Result<(), Error> {
    let path = client_database_path.with_file_name(DATABASE_FILE);
    fs::write(&path, DATABASE).await?;

    let config = ConnectionConfig::SQLite(SqliteConfig {
        path: path.display().to_string(),
        readonly: false,
        initial: None,
    });

    let cid = client
        .create_connection_with_id(CONNECTION_ID.into(), CONNECTION_NAME.into(), config)
        .await?;

    client
        .create_query(cid.clone(), QUERY_NAME.into(), QUERY.into())
        .await?;

    let widgets = serde_json::from_str::<Vec<DashboardWidget>>(DASHBOARD)?;
    for widget in widgets {
        client
            .create_widget(
                cid.clone(),
                widget.width,
                widget.height,
                widget.x,
                widget.y,
                widget.config,
            )
            .await?;
    }

    Ok(())
}
