use crate::Result;
use proxy::ProxyConfig;
use reqwest::Url;

#[derive(Debug)]
pub struct Config {
    pub https: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database: String,
    pub proxy: Option<ProxyConfig>,
}

impl Config {
    fn protocol(&self) -> &str {
        if self.https { "https" } else { "http" }
    }

    pub(crate) fn url(&self) -> Result<Url> {
        let url = endpoint::join_with_scheme(self.protocol(), &self.host, self.port).parse()?;
        Ok(url)
    }

    pub(crate) fn query_url(&self) -> Result<Url> {
        let endpoint = endpoint::join_with_scheme(self.protocol(), &self.host, self.port);
        let url = format!("{endpoint}/v1/query").parse()?;
        Ok(url)
    }
}
