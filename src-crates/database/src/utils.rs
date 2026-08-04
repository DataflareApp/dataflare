use crate::{Error, Result};
use futures_util::future::BoxFuture;
use futures_util::stream::{FuturesUnordered, StreamExt};
use humansize::{DECIMAL, format_size};
use query::Value;
use std::io::Error as IoError;

pub(crate) trait RowsExt {
    // Get the value of the first cell from the query result
    // Used to get the database version when testing a database connection
    fn first_cell_string(&mut self) -> Result<String>;
    fn first_cell_string_optional(&mut self) -> Result<Option<String>>;
    fn first_row_strings<const N: usize>(&mut self) -> Result<[String; N]>;
}

impl RowsExt for Vec<Vec<Value>> {
    fn first_cell_string(&mut self) -> Result<String> {
        if !self.is_empty() && !self[0].is_empty() {
            if let Value::String(s) = self.remove(0).remove(0) {
                return Ok(s);
            }
        }
        Err(Error::Io(IoError::other(
            "Received no result of type 'String'",
        )))
    }

    fn first_cell_string_optional(&mut self) -> Result<Option<String>> {
        if self.is_empty() || self[0].is_empty() {
            return Ok(None);
        }
        match self.remove(0).remove(0) {
            Value::String(value) => Ok(Some(value)),
            Value::Null => Ok(None),
            _ => Err(Error::Io(IoError::other("Received a non-string value"))),
        }
    }

    fn first_row_strings<const N: usize>(&mut self) -> Result<[String; N]> {
        if !self.is_empty() {
            let values = self
                .remove(0)
                .into_iter()
                .map(|value| match value {
                    Value::String(value) => Ok(value),
                    _ => Err(Error::Io(IoError::other("Received a non-string value"))),
                })
                .collect::<Result<Vec<_>>>()?;
            if let Ok(values) = values.try_into() {
                return Ok(values);
            }
        }
        Err(Error::Io(IoError::other(format!(
            "Received no row with {N} string values"
        ))))
    }
}

pub(crate) async fn unordered_tasks<E, I, D, F>(
    max_task: usize,
    mut iter: I,
    run: F,
) -> Result<(), E>
where
    I: Iterator<Item = D>,
    F: Fn(D) -> BoxFuture<'static, Result<(), E>>,
{
    let mut tasks = FuturesUnordered::new();
    loop {
        let task_len = tasks.len();
        if task_len < max_task {
            for _ in 0..max_task - task_len {
                match iter.next() {
                    None => break,
                    Some(sql) => {
                        tasks.push(run(sql));
                    }
                }
            }
        }
        match tasks.next().await {
            None => break,
            Some(rst) => rst?,
        };
    }
    Ok(())
}

pub(crate) fn empty_if<T: Into<String>>(val: String, fallback: T) -> String {
    if val.is_empty() { fallback.into() } else { val }
}

pub(crate) fn format_bytes(bytes: String) -> Result<String> {
    let bytes = bytes
        .parse::<u64>()
        .map_err(|err| Error::Io(IoError::other(err)))?;
    Ok(format_size(bytes, DECIMAL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::FutureExt;

    #[tokio::test]
    async fn test_unordered_tasks() {
        let rst = unordered_tasks(2, vec![1, 2, 3].into_iter(), |n| {
            async move { Err(n) }.boxed()
        })
        .await;
        assert_eq!(rst, Err(1));
    }

    #[test]
    fn first_cell_string_optional_accepts_null() {
        let mut rows = vec![vec![Value::Null]];
        assert_eq!(rows.first_cell_string_optional().unwrap(), None);
    }

    #[test]
    fn format_bytes_uses_decimal_units() {
        assert_eq!(format_bytes("4000000".into()).unwrap(), "4 MB");
    }
}
