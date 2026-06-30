//! A small, synchronous Rust binding for the PGlite WASI build.

use std::path::Path;

#[macro_use]
mod error;
#[allow(dead_code)]
mod pglite;
mod runtime;

pub use error::{Error, Result};
use postgres_types::Type;
use runtime::Runtime;

/// Metadata for one result column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub datatype: Option<Type>,
}

/// One result produced by PostgreSQL's simple-query protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Option<String>>>,
    pub command_tag: String,
}

/// A persistent, single-connection PGlite database.
pub struct Connection {
    runtime: Runtime,
}

impl Connection {
    /// Opens or creates a database rooted at `path`.
    pub fn open_with(path: impl AsRef<Path>) -> Result<Self> {
        Runtime::open(path).map(|runtime| Self { runtime })
    }

    /// Executes one or more SQL statements using PostgreSQL's simple-query protocol.
    pub fn query(&mut self, sql: &str) -> Result<Vec<QueryResult>> {
        self.runtime.query(sql)
    }

    /// Executes each SQL statement in one transaction.
    pub fn transaction(&mut self, sqls: &[&str]) -> Result<Vec<QueryResult>> {
        self.query("BEGIN")?;
        let mut results = Vec::new();
        for sql in sqls {
            match self.query(sql) {
                Ok(query_results) => results.extend(query_results),
                Err(error) => {
                    let _ = self.query("ROLLBACK");
                    return Err(error);
                }
            }
        }
        if let Err(error) = self.query("COMMIT") {
            let _ = self.query("ROLLBACK");
            return Err(error);
        }
        Ok(results)
    }
}
