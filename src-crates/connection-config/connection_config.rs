mod connection_info;

use pgsq::TlsMode;
use proxy::ProxyConfig;
use secret_resolve::ResolveSecrets;
use serde::{Deserialize, Serialize};
use strum::{EnumProperty, IntoStaticStr};

pub use connection_info::*;

#[derive(Debug, Clone, Serialize, Deserialize, IntoStaticStr, EnumProperty, ResolveSecrets)]
#[serde(tag = "type", content = "options")]
pub enum ConnectionConfig {
    SQLite(SqliteConfig),
    SQLCipher(SqlCipherConfig),
    PostgreSQL(PostgresConfig),
    PGlite(PGliteConfig),
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
}

// Must be consistent with the types in ConnectionConfig, but only for SQL databases
#[derive(Debug, Deserialize, Clone, Copy)]
pub enum SqlDatabaseType {
    SQLite,
    SQLCipher,
    PostgreSQL,
    PGlite,
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

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
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

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct ClickHouseConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub user: String,
    #[secret]
    pub password: String,
    pub database: String,
    pub readonly: bool,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct ChDbConfig {
    pub path: String,
    pub database: String,
    pub readonly: bool,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct DatabendConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub user: String,
    #[secret]
    pub password: String,
    pub database: String,
    pub readonly: bool,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct BigQueryConfig {
    pub project_id: Option<String>,
    pub dataset: Option<String>,
    pub auth: BigQueryAuth,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum BigQueryAuth {
    // BigQuery JsonKey stores the content of a JSON file; it is a file selector.
    JsonKey { content: Option<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct DatabricksConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub http_path: String,
    #[secret]
    pub auth: DatabricksAuth,
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub allow_invalid_certs: bool,
    pub readonly: bool,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum DatabricksAuth {
    Token {
        #[secret]
        token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct TrinoConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub user: String,
    #[secret]
    pub auth: TrinoAuth,
    pub catalog: String,
    pub schema: String,
    pub allow_invalid_certs: bool,
    pub readonly: bool,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum TrinoAuth {
    None,
    Password {
        #[secret]
        password: String,
    },
    Jwt {
        #[secret]
        token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct PrestoConfig {
    pub protocol: ConnectProtocol,
    pub host: String,
    pub port: Option<u16>,
    pub user: String,
    #[secret]
    pub auth: PrestoAuth,
    pub catalog: String,
    pub schema: String,
    pub allow_invalid_certs: bool,
    pub readonly: bool,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum PrestoAuth {
    None,
    Password {
        #[secret]
        password: String,
    },
    Jwt {
        #[secret]
        token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct RqliteConfig {
    pub https: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: Option<String>,
    #[secret]
    pub password: Option<String>,
    pub allow_invalid_certs: bool,
    pub readonly: bool,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ResolveSecrets)]
pub struct EchoLiteConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub path: String,
    #[secret]
    pub password: String,
    pub readonly: bool,
    pub initial: Option<String>,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct CloudflareD1Config {
    pub account_id: String,
    pub database_id: String,
    #[secret]
    pub api_token: String,
    pub api_origin: Option<String>,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct WorkersAnalyticsEngineConfig {
    pub account_id: String,
    #[secret]
    pub api_token: String,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct R2SqlConfig {
    pub account_id: String,
    pub bucket_name: String,
    #[secret]
    pub api_token: String,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct SqlCipherConfig {
    pub path: String,
    pub readonly: bool,
    #[secret]
    pub key: String,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct DuckDbConfig {
    pub path: String,
    #[serde(default)]
    pub readonly: bool,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct PostgresConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: String,
    #[secret]
    pub password: String,
    pub database: String,
    #[serde(default)]
    pub readonly: bool,
    pub initial: Option<String>,
    pub tls: PostgresTlsConfig,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct PGliteConfig {
    pub path: String,
    pub readonly: bool,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct QuestDbConfig {
    pub protocol: QuestConnectProtocol,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: String,
    #[secret]
    pub password: String,
    #[serde(default)]
    pub readonly: bool,
    pub tls: PostgresTlsConfig,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QuestConnectProtocol {
    // Currently only PGWire is supported
    PgWire,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct MySqlConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub user: String,
    #[secret]
    pub password: Option<String>,
    pub database: Option<String>,
    #[serde(default)]
    pub readonly: bool,
    pub initial: Option<String>,
    #[serde(default)]
    pub tls: MySqlTlsConfig,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct ManticoreSearchConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub readonly: bool,
    pub initial: Option<String>,
    pub tls: MySqlTlsConfig,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct MsSqlConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    #[secret]
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

#[derive(Debug, Clone, Deserialize, Serialize, ResolveSecrets)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum MsSqlAuthConfig {
    SqlServer {
        user: String,
        #[secret]
        password: String,
    },
    Integrated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert: Option<String>,
    pub key: Option<String>,
    pub ca: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ResolveSecrets)]
pub struct TursoConfig {
    #[secret]
    pub database: TursoDatabaseConfig,
    #[serde(default)]
    pub readonly: bool,
    pub initial: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ResolveSecrets)]
#[serde(tag = "type", rename_all = "lowercase", content = "options")]
pub enum TursoDatabaseConfig {
    // The local libsql was previously named 'file', so compatibility needs to be maintained.
    #[serde(alias = "file")]
    LibSQL { path: String },
    Turso {
        path: String,
        #[secret]
        encryption: Option<TursoEncryptionConfig>,
    },
    Remote {
        url: String,
        #[secret]
        token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct TursoEncryptionConfig {
    pub cipher: String,
    #[secret]
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct CloudflareKvConfig {
    pub account_id: String,
    #[secret]
    pub api_token: String,
    pub default_namespace: String,
    pub readonly: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct RedisConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    #[secret]
    pub password: Option<String>,
    pub readonly: bool,
    pub tls: RedisTlsConfig,
    #[secret]
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisTlsConfig {
    pub enabled: bool,
    pub insecure: bool,
    pub config: TlsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, ResolveSecrets)]
pub struct S3Config {
    #[secret]
    pub access_key: String,
    #[secret]
    pub secret_key: String,
    pub endpoint: String,
    pub region: String,
    pub default_bucket: String,
    pub list_all_buckets: bool,
    pub readonly: bool,
}
