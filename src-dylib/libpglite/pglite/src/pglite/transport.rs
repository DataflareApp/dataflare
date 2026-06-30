use crate::error::{Context, Result};
use std::fs;
use std::thread;
use std::time::{Duration, Instant};

use super::postgres_mod::PostgresMod;

pub enum Transport {
    Cma {
        buffer_addr: usize,
        buffer_len: usize,
    },
    #[allow(dead_code)]
    File,
}

pub struct TransportResponse {
    pub bytes: Vec<u8>,
    pub trapped: bool,
}

impl Transport {
    pub fn from_postgres_mod(pg: &PostgresMod) -> Result<Self> {
        if let (Some(addr), Some(len)) = (pg.buffer_addr(), pg.buffer_len()) {
            Ok(Self::Cma {
                buffer_addr: addr,
                buffer_len: len,
            })
        } else {
            Ok(Self::File)
        }
    }

    pub fn prepare(pg: &mut PostgresMod) -> Result<Self> {
        pg.use_wire(true)?;
        pg.backend()?;
        Self::from_postgres_mod(pg)
    }

    pub fn send(&self, pg: &mut PostgresMod, payload: &[u8]) -> Result<TransportResponse> {
        match self {
            Transport::Cma {
                buffer_addr,
                buffer_len,
            } => send_cma(pg, *buffer_addr, *buffer_len, payload),
            Transport::File => send_file(pg, payload),
        }
    }
}

fn send_cma(
    pg: &mut PostgresMod,
    buffer_addr: usize,
    buffer_len: usize,
    payload: &[u8],
) -> Result<TransportResponse> {
    ensure!(
        payload.len() <= buffer_len,
        "payload of {} bytes exceeds CMA buffer ({} bytes)",
        payload.len(),
        buffer_len
    );

    pg.interactive_write(payload.len() as i32)?;
    if !payload.is_empty() {
        pg.write_memory(buffer_addr, payload)?;
    }
    // PostgreSQL uses its non-local error path for SQL errors. In this WASI
    // build that can surface as a Wasmtime trap even though an ErrorResponse
    // has already been written to the interactive channel.
    let execution = pg.interactive_one();

    let trapped = execution.is_err();
    let available = pg.interactive_read()?;
    if available <= 0 {
        if trapped {
            let bytes = recover_trapped_response(pg, buffer_addr, buffer_len, payload.len())?;
            if !bytes.is_empty() {
                pg.interactive_write(0)?;
                return Ok(TransportResponse {
                    bytes,
                    trapped: true,
                });
            }
            return Ok(TransportResponse {
                bytes: Vec::new(),
                trapped: true,
            });
        }
        execution?;
        return Ok(TransportResponse {
            bytes: Vec::new(),
            trapped: false,
        });
    }

    let response_len = available as usize;
    let response_addr = buffer_addr + payload.len() + 1;
    ensure!(
        response_addr + response_len <= buffer_addr + buffer_len,
        "response range [{}..{}) exceeds CMA buffer [{}..{})",
        response_addr,
        response_addr + response_len,
        buffer_addr,
        buffer_addr + buffer_len
    );

    let mut response = vec![0; response_len];
    pg.read_memory(response_addr, &mut response)?;
    pg.interactive_write(0)?;

    Ok(TransportResponse {
        bytes: response,
        trapped,
    })
}

fn recover_trapped_response(
    pg: &mut PostgresMod,
    buffer_addr: usize,
    buffer_len: usize,
    payload_len: usize,
) -> Result<Vec<u8>> {
    let mut memory = vec![0; buffer_len];
    pg.read_memory(buffer_addr, &mut memory)?;

    let expected = payload_len + 1;
    if let Some(length) = message_sequence_len(&memory[expected..]) {
        return Ok(memory[expected..expected + length].to_vec());
    }

    for offset in 0..memory.len().saturating_sub(5) {
        if matches!(memory[offset], b'E' | b'N')
            && let Some(length) = message_sequence_len(&memory[offset..])
        {
            return Ok(memory[offset..offset + length].to_vec());
        }
    }
    Ok(Vec::new())
}

fn message_sequence_len(bytes: &[u8]) -> Option<usize> {
    let mut offset = 0;
    while offset + 5 <= bytes.len() && bytes[offset] != 0 {
        let length = u32::from_be_bytes(bytes[offset + 1..offset + 5].try_into().ok()?) as usize;
        if length < 4 || offset + 1 + length > bytes.len() {
            break;
        }
        offset += 1 + length;
    }
    (offset > 0).then_some(offset)
}

fn send_file(pg: &mut PostgresMod, payload: &[u8]) -> Result<TransportResponse> {
    let base = pg.paths().pgroot.join("pglite/base");
    let lock_in = base.join(".s.PGSQL.5432.lck.in");
    let in_path = base.join(".s.PGSQL.5432.in");
    let out_path = base.join(".s.PGSQL.5432.out");

    if out_path.exists() {
        let _ = fs::remove_file(&out_path);
    }

    fs::write(&lock_in, payload)
        .with_context(|| format!("write payload to {}", lock_in.display()))?;
    fs::rename(&lock_in, &in_path)
        .with_context(|| format!("rename {} -> {}", lock_in.display(), in_path.display()))?;

    let start = Instant::now();
    let timeout = Duration::from_secs(5);
    loop {
        if out_path.exists() {
            let bytes = fs::read(&out_path)
                .with_context(|| format!("read response from {}", out_path.display()))?;
            let _ = fs::remove_file(&out_path);
            return Ok(TransportResponse {
                bytes,
                trapped: false,
            });
        }
        if start.elapsed() > timeout {
            bail!("file transport timed out waiting for response");
        }
        thread::sleep(Duration::from_millis(2));
    }
}
