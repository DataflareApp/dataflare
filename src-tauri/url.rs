use indexmap::IndexMap;
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use tauri::command;
use url::Url;

// Browser (Chrome) URL parsing/encoding has limitations, e.g. postgres:// cannot be parsed at all, so this plugin was written as an alternative
// 2025-04-19: This module is actually not needed, it can be fully implemented with URL in JS, keeping it as-is to avoid issues

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UrlOption {
    scheme: String,
    host: Option<String>,
    username: String,
    password: Option<String>,
    port: Option<u16>,
    path: String,
    query: IndexMap<String, String>,
}

#[command]
pub fn decode_url(url: &str) -> Result<UrlOption, String> {
    let url = Url::parse(url).map_err(|err| err.to_string())?;
    let mut query = IndexMap::new();
    for (k, v) in url.query_pairs() {
        query.insert(k.to_string(), v.to_string());
    }
    let rst = UrlOption {
        scheme: url.scheme().to_string(),
        host: url.host().map(|v| v.to_string()),
        username: decode_component(url.username(), "username")?,
        password: url
            .password()
            .map(|value| decode_component(value, "password"))
            .transpose()?,
        port: url.port(),
        path: decode_component(url.path(), "path")?,
        query,
    };
    Ok(rst)
}

#[command]
pub fn encode_url(option: UrlOption) -> Result<String, String> {
    let mut url = Url::parse("protoctl://localhost/").unwrap();
    url.set_scheme(&option.scheme)
        .map_err(|_| "Invalid URL Scheme")?;
    match option.host {
        None => {
            url.set_host(None).map_err(|_| "Cannot set host to empty")?;
        }
        Some(host) => {
            match host.parse::<IpAddr>() {
                Ok(ip) => url.set_ip_host(ip).map_err(|_| "Invalid IP Host")?,
                Err(_) => url.set_host(Some(&host)).map_err(|err| err.to_string())?,
            };
            url.set_username(&option.username)
                .map_err(|_| "Invalid URL Username")?;
            url.set_password(option.password.as_deref())
                .map_err(|_| "Invalid URL Password")?;
            url.set_port(option.port).map_err(|_| "Invalid URL Port")?;
        }
    };
    url.set_path(&option.path);
    for (k, v) in option.query {
        url.query_pairs_mut().append_pair(&k, &v);
    }
    Ok(url.to_string())
}

fn decode_component(value: &str, component: &str) -> Result<String, String> {
    percent_decode_str(value)
        .decode_utf8()
        .map(|value| value.into_owned())
        .map_err(|err| format!("Invalid UTF-8 in URL {component}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_pg_url() {
        let mut query = IndexMap::new();
        query.insert("name".into(), "Name".into());
        query.insert("desc".into(), "你好".into());
        let option = UrlOption {
            scheme: "postgres".into(),
            host: Some("localhost".into()),
            username: "username".into(),
            password: Some("password".into()),
            port: Some(5432),
            path: "/database".into(),
            query,
        };
        assert_eq!(
            encode_url(option).unwrap(),
            "postgres://username:password@localhost:5432/database?name=Name&desc=%E4%BD%A0%E5%A5%BD"
        );
    }

    #[test]
    fn decode_pg_url() {
        let url = decode_url("postgres://username:password@localhost:5432/database?name=Name&desc=%E4%BD%A0%E5%A5%BD").unwrap();
        let mut query = IndexMap::new();
        query.insert("name".into(), "Name".into());
        query.insert("desc".into(), "你好".into());
        assert_eq!(
            url,
            UrlOption {
                scheme: "postgres".into(),
                host: Some("localhost".into()),
                username: "username".into(),
                password: Some("password".into()),
                port: Some(5432),
                path: "/database".into(),
                query,
            }
        );
    }

    #[test]
    fn decode_percent_encoded_pg_url_components() {
        let url = decode_url(
            "postgresql://user%40example.com:p%40ss%3Aword%2F100%25@localhost/db%20name?name=My+DB",
        )
        .unwrap();
        let mut query = IndexMap::new();
        query.insert("name".into(), "My DB".into());
        assert_eq!(
            url,
            UrlOption {
                scheme: "postgresql".into(),
                host: Some("localhost".into()),
                username: "user@example.com".into(),
                password: Some("p@ss:word/100%".into()),
                port: None,
                path: "/db name".into(),
                query,
            }
        );
    }

    #[test]
    fn decode_1password_secret_reference_password() {
        let url = decode_url(
            "postgresql://owner:exec%3Aop%20read%20%22op%3A%2F%2FPersonal%2FMy%2Fdatabase%20password%22@example.com/mydb?name=My+PostgreSQL",
        )
        .unwrap();
        assert_eq!(
            url.password.as_deref(),
            Some("exec:op read \"op://Personal/My/database password\"")
        );
        assert_eq!(
            url.query.get("name").map(String::as_str),
            Some("My PostgreSQL")
        );
    }

    #[test]
    fn percent_encoded_components_round_trip() {
        let option = UrlOption {
            scheme: "postgresql".into(),
            host: Some("localhost".into()),
            username: "user@example.com".into(),
            password: Some("p@ss:word/100%".into()),
            port: Some(5432),
            path: "/db name".into(),
            query: IndexMap::new(),
        };
        let encoded = encode_url(option).unwrap();

        assert_eq!(
            decode_url(&encoded).unwrap(),
            UrlOption {
                scheme: "postgresql".into(),
                host: Some("localhost".into()),
                username: "user@example.com".into(),
                password: Some("p@ss:word/100%".into()),
                port: Some(5432),
                path: "/db name".into(),
                query: IndexMap::new(),
            }
        );
    }

    #[test]
    fn encode_sqlite_url() {
        let mut query = IndexMap::new();
        query.insert("Hello".into(), "world".into());
        let options = UrlOption {
            scheme: "sqlite".into(),
            host: None,
            username: "".into(),
            password: None,
            port: None,
            path: "/root/data.db".into(),
            query,
        };
        assert_eq!(
            encode_url(options).unwrap(),
            "sqlite:/root/data.db?Hello=world"
        );
    }

    #[test]
    fn decode_sqlite_url() {
        let url = decode_url("SQlitE:///root/data.db").unwrap();
        assert_eq!(
            url,
            UrlOption {
                scheme: "sqlite".into(),
                host: None,
                username: "".into(),
                password: None,
                port: None,
                path: "/root/data.db".into(),
                query: IndexMap::new(),
            }
        );
    }

    #[test]
    fn encode_ipv6_host() {
        let options = UrlOption {
            scheme: "custom".into(),
            host: Some("::1".into()),
            username: "".into(),
            password: None,
            port: Some(1122),
            path: "".into(),
            query: IndexMap::new(),
        };
        assert_eq!(encode_url(options).unwrap(), "custom://[::1]:1122");
    }

    #[test]
    fn encode_ipv4_host() {
        let options = UrlOption {
            scheme: "asd".into(),
            host: Some("127.0.0.1".into()),
            username: "u".into(),
            password: Some("p".into()),
            port: None,
            path: "/path/file".into(),
            query: IndexMap::new(),
        };
        assert_eq!(
            encode_url(options).unwrap(),
            "asd://u:p@127.0.0.1/path/file"
        );
    }
}
