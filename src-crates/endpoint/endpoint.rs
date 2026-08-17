pub fn join<S: AsRef<str>>(host: S, port: u16) -> String {
    let host = host.as_ref();
    let bracketed = host.starts_with('[') && host.ends_with(']');
    if host.contains(':') && !bracketed {
        return format!("[{host}]:{port}");
    }
    format!("{host}:{port}")
}

pub fn join_with_scheme<S: AsRef<str>, H: AsRef<str>>(scheme: S, host: H, port: u16) -> String {
    format!("{}://{}", scheme.as_ref(), join(host, port))
}

#[cfg(test)]
mod tests {
    use super::{join, join_with_scheme};

    #[test]
    fn joins_host_and_port() {
        assert_eq!(join("localhost", 5432), "localhost:5432");
        assert_eq!(join("example.com", 80), "example.com:80");
        assert_eq!(join("127.0.0.1", 5432), "127.0.0.1:5432");
        assert_eq!(join("::1", 5432), "[::1]:5432");
        assert_eq!(join("[::1]", 5432), "[::1]:5432");
        assert_eq!(join("2001:db8::1", 443), "[2001:db8::1]:443");
    }

    #[test]
    fn joins_scheme_host_and_port() {
        assert_eq!(
            join_with_scheme("https", "example.com", 443),
            "https://example.com:443"
        );
        assert_eq!(join_with_scheme("http", "::1", 8080), "http://[::1]:8080");
        assert_eq!(join_with_scheme("http", "[::1]", 8080), "http://[::1]:8080");
    }
}
