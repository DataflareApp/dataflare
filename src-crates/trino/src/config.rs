use crate::Result;
use proxy::ProxyConfig;
use reqwest::Url;

#[derive(Debug)]
pub struct Config {
    pub https: bool,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: AuthConfig,
    pub catalog: String,
    pub schema: String,
    pub allow_invalid_certs: bool,
    pub proxy: Option<ProxyConfig>,
}

#[derive(Debug, Clone)]
pub enum AuthConfig {
    None,
    Password { password: String },
    Jwt { token: String },
}

impl Config {
    fn protocol(&self) -> &str {
        if self.https { "https" } else { "http" }
    }

    pub(crate) fn query_url(&self) -> Result<Url> {
        let endpoint = endpoint::join_with_scheme(self.protocol(), &self.host, self.port);
        let url = format!("{endpoint}/v1/statement").parse()?;
        Ok(url)
    }
}
