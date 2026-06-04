use pgsq::TlsMode;
use proxy::ProxyConfig;
use secret_resolve::Secret;
use serde::{Deserialize, Serialize};
use strum::{EnumProperty, IntoStaticStr};

#[derive(Debug, Clone, Serialize, Deserialize, IntoStaticStr, EnumProperty)]
#[serde(tag = "type", content = "options")]
pub enum ConnectionConfig {
    SQLite(SqliteConfig),
    SQLCipher(SqlCipherConfig),
    PostgreSQL(PostgresConfig),
    CockroachDB(PostgresConfig),
    QuestDB(QuestDbConfig),
    MySQL(MySqlConfig),
    MariaDB(MySqlConfig),
    #[serde(rename = "Manticore Search")]
    #[strum(serialize = "Manticore Search")]
    ManticoreSearch(ManticoreSearchConfig),
    MSSQL(MsSqlConfig),
    ClickHouse(ClickHouseConfig),
    #[serde(rename = "chDB")]
    #[strum(serialize = "chDB")]
    ChDb(ChDbConfig),
    Databend(DatabendConfig),
    BigQuery(BigQueryConfig),
    Trino(TrinoConfig),
    Presto(PrestoConfig),
    Databricks(DatabricksConfig),
    DuckDB(DuckDbConfig),
    // alias: The purpose is to keep compatibility after renaming libSQL to Turso
    #[serde(alias = "LibSQL")]
    Turso(TursoConfig),
    // alias: The purpose is to allow importing database connections (type = FastSQLite) directly as Rqlite
    #[serde(alias = "FastSQLite")]
    Rqlite(RqliteConfig),
    EchoLite(EchoLiteConfig),
    #[serde(rename = "Cloudflare D1")]
    #[strum(serialize = "Cloudflare D1")]
    CloudflareD1(CloudflareD1Config),
    #[serde(rename = "Workers Analytics Engine")]
    #[strum(serialize = "Workers Analytics Engine")]
    WorkersAnalyticsEngine(WorkersAnalyticsEngineConfig),
    #[serde(rename = "R2 SQL")]
    #[strum(serialize = "R2 SQL")]
    R2Sql(R2SqlConfig),

    #[serde(rename = "Cloudflare Workers KV")]
    #[strum(serialize = "Cloudflare Workers KV", props(kv = "true"))]
    CloudflareKv(CloudflareKvConfig),
    #[strum(props(kv = "true"))]
    Redis(RedisConfig),
    #[strum(props(kv = "true"))]
    S3(S3Config),
}

impl ConnectionConfig {
    pub fn product_name(&self) -> &'static str {
        self.into()
    }

    pub fn is_kv(&self) -> bool {
        self.get_str("kv").is_some()
    }

    pub async fn secret_resolve(mut self) -> Result<Self, secret_resolve::Error> {
        match &mut self {
            ConnectionConfig::SQLite(_) => {}
            ConnectionConfig::SQLCipher(config) => {
                config.key = Secret::resolve(&config.key).await?;
            }
            ConnectionConfig::PostgreSQL(config) | ConnectionConfig::CockroachDB(config) => {
                config.password = Secret::resolve(&config.password).await?;
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::QuestDB(config) => {
                config.password = Secret::resolve(&config.password).await?;
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::MySQL(config) | ConnectionConfig::MariaDB(config) => {
                config.password = Secret::resolve_option(config.password.take()).await?;
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::ManticoreSearch(config) => {
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::MSSQL(config) => {
                if let MsSqlAuthConfig::SqlServer { password, .. } = &mut config.auth {
                    *password = Secret::resolve(&password).await?;
                }
            }
            ConnectionConfig::ClickHouse(config) => {
                config.password = Secret::resolve(&config.password).await?;
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::ChDb(_) => {}
            ConnectionConfig::Databend(config) => {
                config.password = Secret::resolve(&config.password).await?;
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::BigQuery(config) => {
                let BigQueryAuth::JsonKey { content } = &mut config.auth;
                *content = Secret::resolve_option(content.take()).await?;
            }
            ConnectionConfig::Trino(config) => {
                match &mut config.auth {
                    TrinoAuth::Password { password } => {
                        *password = Secret::resolve(&password).await?;
                    }
                    TrinoAuth::Jwt { token } => {
                        *token = Secret::resolve(&token).await?;
                    }
                    TrinoAuth::None => {}
                }
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::Presto(config) => {
                match &mut config.auth {
                    PrestoAuth::Password { password } => {
                        *password = Secret::resolve(&password).await?;
                    }
                    PrestoAuth::Jwt { token } => {
                        *token = Secret::resolve(&token).await?;
                    }
                    PrestoAuth::None => {}
                }
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::Databricks(config) => {
                let DatabricksAuth::Token { token } = &mut config.auth;
                *token = Secret::resolve(&token).await?;
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::DuckDB(_) => {}
            ConnectionConfig::Turso(config) => match &mut config.database {
                TursoDatabaseConfig::Remote { token, .. } => {
                    *token = Secret::resolve(&token).await?;
                }
                TursoDatabaseConfig::Turso { encryption, .. } => {
                    if let Some(TursoEncryptionConfig { key, .. }) = encryption {
                        *key = Secret::resolve(&key).await?;
                    }
                }
                _ => {}
            },
            ConnectionConfig::Rqlite(config) => {
                config.password = Secret::resolve_option(config.password.take()).await?;
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::EchoLite(config) => {
                config.password = Secret::resolve(&config.password).await?;
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::CloudflareD1(config) => {
                config.api_token = Secret::resolve(&config.api_token).await?;
            }
            ConnectionConfig::WorkersAnalyticsEngine(config) => {
                config.api_token = Secret::resolve(&config.api_token).await?;
            }
            ConnectionConfig::R2Sql(config) => {
                config.api_token = Secret::resolve(&config.api_token).await?;
            }
            ConnectionConfig::CloudflareKv(config) => {
                config.api_token = Secret::resolve(&config.api_token).await?;
            }
            ConnectionConfig::Redis(config) => {
                config.password = Secret::resolve_option(config.password.take()).await?;
                ProxyConfig::secret_resolve(&mut config.proxy).await?;
            }
            ConnectionConfig::S3(config) => {
                config.secret_key = Secret::resolve(&config.secret_key).await?;
            }
        }
        Ok(self)
    }
}

// Must be consistent with the types in ConnectionConfig, but only for SQL databases
#[derive(Debug, Deserialize, Clone, Copy)]
pub enum SqlDatabaseType {
    SQLite,
    SQLCipher,
    PostgreSQL,
    CockroachDB,
    QuestDB,
    MySQL,
    MariaDB,
    #[serde(rename = "Manticore Search")]
    ManticoreSearch,
    MSSQL,
    ClickHouse,
    #[serde(rename = "chDB")]
    ChDb,
    Databend,
    BigQuery,
    Trino,
    Presto,
    Databricks,
    DuckDB,
    Turso,
    Rqlite,
    EchoLite,
    #[serde(rename = "Cloudflare D1")]
    CloudflareD1,
    #[serde(rename = "Workers Analytics Engine")]
    WorkersAnalyticsEngine,
    #[serde(rename = "R2 SQL")]
    R2Sql,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub path: String,
    pub readonly: bool,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectProtocol {
    Http,
    Https,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClickHouseConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub user: String,
    pub password: String,
    pub database: String,
    pub readonly: bool,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChDbConfig {
    pub path: String,
    pub readonly: bool,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabendConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub user: String,
    pub password: String,
    pub database: String,
    pub readonly: bool,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BigQueryConfig {
    pub project_id: Option<String>,
    pub dataset: Option<String>,
    pub auth: BigQueryAuth,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum BigQueryAuth {
    JsonKey { content: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabricksConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub http_path: String,
    pub auth: DatabricksAuth,
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub allow_invalid_certs: bool,
    pub readonly: bool,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum DatabricksAuth {
    Token { token: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrinoConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub user: String,
    pub auth: TrinoAuth,
    pub catalog: String,
    pub schema: String,
    pub allow_invalid_certs: bool,
    pub readonly: bool,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum TrinoAuth {
    None,
    Password { password: String },
    Jwt { token: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrestoConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub user: String,
    pub auth: PrestoAuth,
    pub catalog: String,
    pub schema: String,
    pub allow_invalid_certs: bool,
    pub readonly: bool,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum PrestoAuth {
    None,
    Password { password: String },
    Jwt { token: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RqliteConfig {
    pub https: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub allow_invalid_certs: bool,
    pub readonly: bool,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EchoLiteConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: String,
    pub password: String,
    pub readonly: bool,
    pub initial: Option<String>,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareD1Config {
    pub account_id: String,
    pub database_id: String,
    pub api_token: String,
    pub api_origin: Option<String>,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkersAnalyticsEngineConfig {
    pub account_id: String,
    pub api_token: String,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct R2SqlConfig {
    pub account_id: String,
    pub bucket_name: String,
    pub api_token: String,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlCipherConfig {
    pub path: String,
    pub readonly: bool,
    pub key: String,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuckDbConfig {
    pub path: String,
    #[serde(default)]
    pub readonly: bool,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: String,
    pub password: String,
    pub database: String,
    #[serde(default)]
    pub readonly: bool,
    pub initial: Option<String>,
    pub tls: PostgresTlsConfig,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestDbConfig {
    pub protocol: QuestConnectProtocol,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: String,
    pub password: String,
    #[serde(default)]
    pub readonly: bool,
    pub tls: PostgresTlsConfig,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QuestConnectProtocol {
    // Currently only PGWire is supported
    PgWire,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySqlConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: String,
    pub password: Option<String>,
    pub database: Option<String>,
    #[serde(default)]
    pub readonly: bool,
    pub initial: Option<String>,
    #[serde(default)]
    pub tls: MySqlTlsConfig,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManticoreSearchConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub readonly: bool,
    pub initial: Option<String>,
    pub tls: MySqlTlsConfig,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsSqlConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub auth: MsSqlAuthConfig,
    pub database: String,
    #[serde(default)]
    pub readonly: bool,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresTlsConfig {
    pub mode: PostgresTlsMode,
    pub config: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MySqlTlsConfig {
    pub mode: MySqlTlsMode,
    pub config: TlsConfig,
}

impl Default for MySqlTlsConfig {
    fn default() -> Self {
        Self {
            mode: MySqlTlsMode::Disabled,
            config: TlsConfig {
                cert: None,
                key: None,
                ca: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PostgresTlsMode {
    Disabled,
    Allow,
    Preferred,
    Required,
    VerifyCa,
    VerifyFull,
}

impl From<PostgresTlsMode> for TlsMode {
    fn from(val: PostgresTlsMode) -> Self {
        match val {
            PostgresTlsMode::Disabled => Self::Disabled,
            PostgresTlsMode::Allow => Self::Allow,
            PostgresTlsMode::Preferred => Self::Preferred,
            PostgresTlsMode::Required => Self::Required,
            PostgresTlsMode::VerifyCa => Self::VerifyCa,
            PostgresTlsMode::VerifyFull => Self::VerifyFull,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MySqlTlsMode {
    Required,
    RequiredVerify,
    #[serde(other)]
    Disabled,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum MsSqlAuthConfig {
    SqlServer { user: String, password: String },
    Integrated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert: Option<String>,
    pub key: Option<String>,
    pub ca: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TursoConfig {
    pub database: TursoDatabaseConfig,
    #[serde(default)]
    pub readonly: bool,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum TursoDatabaseConfig {
    // The local libsql was previously named 'file', so compatibility needs to be maintained.
    #[serde(alias = "file")]
    LibSQL {
        path: String,
    },
    Turso {
        path: String,
        encryption: Option<TursoEncryptionConfig>,
    },
    Remote {
        url: String,
        token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TursoEncryptionConfig {
    pub cipher: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareKvConfig {
    pub account_id: String,
    pub api_token: String,
    pub default_namespace: String,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub readonly: bool,
    pub tls: RedisTlsConfig,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisTlsConfig {
    pub enabled: bool,
    pub insecure: bool,
    pub config: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct S3Config {
    pub access_key: String,
    pub secret_key: String,
    pub endpoint: String,
    pub region: String,
    pub default_bucket: String,
    pub list_all_buckets: bool,
    pub readonly: bool,
}
