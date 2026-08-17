use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub items: Vec<ConnectionInfoItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfoItem {
    pub name: String,
    pub value: ConnectionInfoValue,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ConnectionInfoValue {
    Text { text: String },
    File { path: String },
    Url { url: String },
    Server { protocol: String, server: String },
}

impl ConnectionInfo {
    pub fn new(driver: &'static str) -> Self {
        let mut info = Self::default();
        info.push_text("Driver", driver);
        info
    }

    pub fn push_text(&mut self, name: impl ToString, value: impl ToString) {
        self.items.push(ConnectionInfoItem {
            name: name.to_string(),
            value: ConnectionInfoValue::Text {
                text: value.to_string(),
            },
        });
    }

    pub fn push_file(&mut self, name: impl ToString, value: impl ToString) {
        self.items.push(ConnectionInfoItem {
            name: name.to_string(),
            value: ConnectionInfoValue::File {
                path: value.to_string(),
            },
        });
    }

    pub fn push_db_path(&mut self, path: impl ToString) {
        let path = path.to_string();
        if path.trim().is_empty() {
            self.push_text("Path", path);
        } else {
            self.push_file("Path", path);
        }
    }

    pub fn push_url(&mut self, name: impl ToString, value: impl ToString) {
        self.items.push(ConnectionInfoItem {
            name: name.to_string(),
            value: ConnectionInfoValue::Url {
                url: value.to_string(),
            },
        });
    }

    pub fn push_server(&mut self, protocol: impl ToString, server: impl ToString, port: u16) {
        let server = endpoint::join(server.to_string(), port);
        self.items.push(ConnectionInfoItem {
            name: "Server".into(),
            value: ConnectionInfoValue::Server {
                protocol: protocol.to_string(),
                server,
            },
        });
    }
}
