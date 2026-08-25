use crate::{Query, QueryColumn, QueryValue, Result};
use pglite::QueryResult;
use postgres_types::Type;

pub(crate) fn to_query(mut queries: Vec<QueryResult>) -> Result<Query> {
    if queries.is_empty() {
        return Err("simple query returned no results".to_string());
    }
    let query = queries.remove(0);
    let rows_affected = rows_affected_from_tag(&query.command_tag).unwrap_or_default();

    let mut rows = Vec::with_capacity(query.rows.len());
    for row in &query.rows {
        let mut values = Vec::with_capacity(row.len());
        for (index, value) in row.iter().enumerate() {
            values.push(decode_value(
                value.as_deref(),
                &query.columns[index].datatype,
            )?);
        }
        rows.push(values);
    }

    let columns = query
        .columns
        .into_iter()
        .map(|col| QueryColumn {
            name: col.name,
            datatype: col
                .datatype
                .map(|t| t.to_string())
                .unwrap_or_else(|| "unknown".into()),
        })
        .collect::<Vec<_>>();

    Ok(Query {
        columns,
        rows,
        rows_affected,
    })
}

fn decode_value(value: Option<&str>, datatype: &Option<Type>) -> Result<QueryValue> {
    let Some(value) = value else {
        return Ok(QueryValue::Null);
    };
    match *datatype {
        Some(Type::BOOL) => decode_bool(value).map(QueryValue::Bool),
        Some(Type::INT2) | Some(Type::INT4) | Some(Type::INT8) => value
            .parse::<i64>()
            .map(QueryValue::I64)
            .map_err(|err| err.to_string()),
        Some(Type::FLOAT4) | Some(Type::FLOAT8) => value
            .parse::<f64>()
            .map(QueryValue::F64)
            .map_err(|err| err.to_string()),
        Some(Type::BYTEA) => decode_bytea(value).map(QueryValue::Bytes),
        Some(Type::OID) => value
            .parse::<u32>()
            .map(QueryValue::U32)
            .map_err(|err| err.to_string()),
        _ => Ok(QueryValue::String(value.to_string())),
    }
}

fn decode_bool(value: &str) -> Result<bool> {
    match value {
        "t" => Ok(true),
        "f" => Ok(false),
        _ => Err(format!("invalid bool value '{value}'")),
    }
}

fn decode_bytea(value: &str) -> Result<Vec<u8>> {
    let value = value
        .strip_prefix("\\x")
        .ok_or_else(|| "bytea value must start with '\\x'".to_string())?;
    const_hex::decode(value).map_err(|err| err.to_string())
}

fn rows_affected_from_tag(tag: &str) -> Option<u64> {
    let mut parts = tag.split_ascii_whitespace();
    let command = parts.next()?;
    match command {
        "COPY" | "DELETE" | "INSERT" | "MERGE" | "UPDATE" => {
            parts.next_back().and_then(|s| s.parse::<u64>().ok())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(name: &str, sql: &str) -> Query {
        let path = std::env::temp_dir().join(format!(
            "dataflare-libpglite-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        let mut database = pglite::Connection::open_with(&path).unwrap();
        let results = database.query(sql).unwrap();
        let query = to_query(results).unwrap();
        drop(database);
        let _ = std::fs::remove_dir_all(path);
        query
    }

    #[test]
    fn converts_basic_query_values() {
        let query = query(
            "basic-values",
            "
            SELECT
                NULL::text AS null_value,
                true AS true_value,
                false AS false_value,
                1::int2 AS int2_value,
                2::int4 AS int4_value,
                3::int8 AS int8_value,
                1.5::float4 AS float4_value,
                2.5::float8 AS float8_value,
                decode('6869', 'hex') AS bytes_value,
                'hello'::text AS string_value,
                7::oid AS oid_value
            ",
        );

        assert_eq!(query.rows_affected, 0);
        assert_eq!(
            query.columns,
            [
                QueryColumn {
                    name: "null_value".into(),
                    datatype: "text".into(),
                },
                QueryColumn {
                    name: "true_value".into(),
                    datatype: "bool".into(),
                },
                QueryColumn {
                    name: "false_value".into(),
                    datatype: "bool".into(),
                },
                QueryColumn {
                    name: "int2_value".into(),
                    datatype: "int2".into(),
                },
                QueryColumn {
                    name: "int4_value".into(),
                    datatype: "int4".into(),
                },
                QueryColumn {
                    name: "int8_value".into(),
                    datatype: "int8".into(),
                },
                QueryColumn {
                    name: "float4_value".into(),
                    datatype: "float4".into(),
                },
                QueryColumn {
                    name: "float8_value".into(),
                    datatype: "float8".into(),
                },
                QueryColumn {
                    name: "bytes_value".into(),
                    datatype: "bytea".into(),
                },
                QueryColumn {
                    name: "string_value".into(),
                    datatype: "text".into(),
                },
                QueryColumn {
                    name: "oid_value".into(),
                    datatype: "oid".into(),
                },
            ]
        );
        assert_eq!(
            query.rows,
            [vec![
                QueryValue::Null,
                QueryValue::Bool(true),
                QueryValue::Bool(false),
                QueryValue::I64(1),
                QueryValue::I64(2),
                QueryValue::I64(3),
                QueryValue::F64(1.5),
                QueryValue::F64(2.5),
                QueryValue::Bytes(b"hi".to_vec()),
                QueryValue::String("hello".into()),
                QueryValue::U32(7),
            ]]
        );
    }

    #[test]
    fn keeps_unsupported_query_values_as_strings() {
        let query = query(
            "string-values",
            "
            SELECT
                ARRAY[1, 2, 3]::int4[] AS array_value,
                '{\"name\":\"dataflare\",\"enabled\":true}'::json AS json_value,
                TIMESTAMP '2025-01-02 03:04:05' AS timestamp_value
            ",
        );

        assert_eq!(
            query.columns,
            [
                QueryColumn {
                    name: "array_value".into(),
                    datatype: "_int4".into(),
                },
                QueryColumn {
                    name: "json_value".into(),
                    datatype: "json".into(),
                },
                QueryColumn {
                    name: "timestamp_value".into(),
                    datatype: "timestamp".into(),
                },
            ]
        );
        assert_eq!(
            query.rows,
            [vec![
                QueryValue::String("{1,2,3}".into()),
                QueryValue::String("{\"name\":\"dataflare\",\"enabled\":true}".into()),
                QueryValue::String("2025-01-02 03:04:05".into()),
            ]]
        );
    }
}
