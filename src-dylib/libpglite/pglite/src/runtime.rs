use std::fs::OpenOptions;
use std::mem::take;
use std::path::Path;

use bytes::{Buf, BytesMut};
use pgwire::messages::{
    DecodeContext, PgWireBackendMessage, PgWireFrontendMessage, data::DataRow, simplequery::Query,
    terminate::Terminate,
};
use postgres_types::Type;
use tempfile::TempDir;

use crate::error::Context;
use crate::pglite::{
    base::{PglitePaths, install_into},
    postgres_mod::PostgresMod,
    transport::Transport,
};
use crate::{Column, Error, QueryResult, Result};

pub(crate) struct Runtime {
    postgres: PostgresMod,
    transport: Transport,
    // Keeps the temporary WASI runtime files alive for the lifetime of the database.
    _runtime_dir: TempDir,
}

impl Runtime {
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self> {
        let outcome = install_into(path.as_ref())?;
        let paths = outcome.paths;
        let runtime_dir = outcome.runtime_dir;
        if !paths.is_cluster_initialized() {
            let mut initializer = PostgresMod::new(paths.clone(), "template1")?;
            initializer.ensure_cluster()?;
        }

        Self::ensure_default_database(paths.clone())?;
        let mut postgres = PostgresMod::new(paths, "postgres")?;
        postgres.ensure_cluster()?;
        let transport = Transport::prepare(&mut postgres)?;
        Ok(Self {
            postgres,
            transport,
            _runtime_dir: runtime_dir,
        })
    }

    pub(crate) fn query(&mut self, sql: &str) -> Result<Vec<QueryResult>> {
        let response = match send_query(&mut self.postgres, &self.transport, sql) {
            Ok(response) => response,
            Err(error) if error.to_string().contains("_interactive_one") => {
                self.restart()?;
                return Err(Error::Database {
                    severity: "ERROR".to_owned(),
                    code: "XX000".to_owned(),
                    message: "PGlite query failed without a complete PostgreSQL error response"
                        .to_owned(),
                });
            }
            Err(error) => return Err(error),
        };

        let parsed = parse_response(response.bytes, response.trapped);
        self.sync_to_fs()?;
        if response.trapped {
            self.restart()?;
        }
        parsed
    }

    fn shutdown(&mut self) -> Result<()> {
        shutdown_postgres(&mut self.postgres, &self.transport)
    }

    fn restart(&mut self) -> Result<()> {
        let paths = self.postgres.paths().clone();
        let mut postgres = PostgresMod::new(paths, "postgres")?;
        postgres.ensure_cluster()?;
        let transport = Transport::prepare(&mut postgres)?;
        self.postgres = postgres;
        self.transport = transport;
        Ok(())
    }

    fn ensure_default_database(paths: PglitePaths) -> Result<()> {
        let mut postgres = PostgresMod::new(paths, "template1")?;
        postgres.ensure_cluster()?;
        let transport = Transport::prepare(&mut postgres)?;
        let response = send_query(
            &mut postgres,
            &transport,
            "SELECT count(*) FROM pg_database WHERE datname = 'postgres'",
        )?;
        let result = parse_response(response.bytes, response.trapped)?;
        if result[0].rows != vec![vec![Some("1".to_owned())]] {
            let response = send_query(&mut postgres, &transport, "CREATE DATABASE postgres")?;
            parse_response(response.bytes, response.trapped)?;
        }
        shutdown_postgres(&mut postgres, &transport)
    }

    fn sync_to_fs(&self) -> Result<()> {
        let pgdata = self.postgres.paths().pgdata.as_path();
        Self::sync_pgdata(pgdata)
    }

    fn sync_pgdata(pgdata: &Path) -> Result<()> {
        if let Ok(file) = OpenOptions::new().read(true).open(pgdata) {
            file.sync_all()
                .with_context(|| format!("sync {}", pgdata.display()))?;
        }
        Ok(())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn shutdown_postgres(postgres: &mut PostgresMod, transport: &Transport) -> Result<()> {
    let mut request = BytesMut::new();
    PgWireFrontendMessage::Terminate(Terminate::new())
        .encode(&mut request)
        .map_err(|error| Error::message(format!("encode terminate message: {error}")))?;
    let _ = transport.send(postgres, &request)?;
    postgres.shutdown()?;
    Runtime::sync_pgdata(postgres.paths().pgdata.as_path())
}

fn send_query(
    postgres: &mut PostgresMod,
    transport: &Transport,
    sql: &str,
) -> Result<crate::pglite::transport::TransportResponse> {
    let mut request = BytesMut::new();
    PgWireFrontendMessage::Query(Query::new(sql.to_owned()))
        .encode(&mut request)
        .map_err(|error| Error::message(format!("encode query: {error}")))?;
    transport.send(postgres, &request)
}

fn parse_response(response: Vec<u8>, trapped: bool) -> Result<Vec<QueryResult>> {
    let mut buffer = BytesMut::from(response.as_slice());
    let context = DecodeContext::default();
    let mut results = Vec::new();
    let mut columns = Vec::new();
    let mut rows = Vec::new();
    let mut database_error = None;

    while !buffer.is_empty() {
        let before = buffer.len();
        let message = PgWireBackendMessage::decode(&mut buffer, &context)
            .map_err(|error| Error::message(format!("decode backend message: {error}")))?
            .ok_or_else(|| {
                Error::message(format!(
                    "incomplete backend message: {} bytes remain",
                    buffer.len()
                ))
            })?;
        if buffer.len() >= before {
            return Err(Error::message("backend decoder made no progress"));
        }

        match message {
            PgWireBackendMessage::RowDescription(description) => {
                columns = description
                    .fields
                    .into_iter()
                    .map(|field| Column {
                        name: field.name,
                        datatype: Type::from_oid(field.type_id),
                    })
                    .collect();
            }
            PgWireBackendMessage::DataRow(row) => rows.push(decode_row(row)?),
            PgWireBackendMessage::CommandComplete(command) => {
                results.push(QueryResult {
                    columns: take(&mut columns),
                    rows: take(&mut rows),
                    command_tag: command.tag,
                });
            }
            PgWireBackendMessage::EmptyQueryResponse(_) => {
                results.push(QueryResult {
                    columns: Vec::new(),
                    rows: Vec::new(),
                    command_tag: String::new(),
                });
            }
            PgWireBackendMessage::ErrorResponse(error) => {
                database_error = Some(decode_database_error(error.fields));
            }
            PgWireBackendMessage::NoticeResponse(notice) if trapped => {
                let mut error = decode_database_error(notice.fields);
                if let Error::Database { severity, .. } = &mut error {
                    *severity = "ERROR".to_owned();
                }
                database_error = Some(error);
            }
            _ => {}
        }
    }

    if let Some(error) = database_error {
        return Err(error);
    }
    if trapped {
        return Err(Error::Database {
            severity: "ERROR".to_owned(),
            code: "XX000".to_owned(),
            message: "PGlite query failed without a complete PostgreSQL error response".to_owned(),
        });
    }
    Ok(results)
}

fn decode_row(row: DataRow) -> Result<Vec<Option<String>>> {
    let mut data = row.data;
    let mut values = Vec::with_capacity(row.field_count as usize);
    for _ in 0..row.field_count {
        if data.remaining() < 4 {
            return Err(Error::message("truncated DataRow field length"));
        }
        let length = data.get_i32();
        if length == -1 {
            values.push(None);
            continue;
        }
        if length < 0 {
            return Err(Error::message(format!(
                "invalid DataRow field length {length}"
            )));
        }
        let length = length as usize;
        if data.remaining() < length {
            return Err(Error::message(format!(
                "truncated DataRow field: expected {length} bytes"
            )));
        }
        let value = String::from_utf8(data.copy_to_bytes(length).to_vec())
            .context("DataRow contains invalid UTF-8")?;
        values.push(Some(value));
    }
    if data.has_remaining() {
        return Err(Error::message("unexpected trailing DataRow bytes"));
    }
    Ok(values)
}

fn decode_database_error(fields: Vec<(u8, String)>) -> Error {
    let field = |code| {
        fields
            .iter()
            .find_map(|(field_code, value)| (*field_code == code).then(|| value.clone()))
    };
    Error::Database {
        severity: field(b'S').unwrap_or_else(|| "ERROR".to_owned()),
        code: field(b'C').unwrap_or_else(|| "XX000".to_owned()),
        message: field(b'M').unwrap_or_else(|| "PostgreSQL error".to_owned()),
    }
}
