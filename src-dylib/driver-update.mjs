import crypto from 'crypto'

// The drivers for SQLCipher, LibSQL, Turso, DuckDB, chDB and PGlite are dynamically loaded
// This script extracts the sha256 values for the corresponding platform
// src-crates/sqlcipher/src/lib.rs
// src-crates/libsql-local/src/lib.rs
// src-crates/turso-local/src/lib.rs
// src-crates/duckdb/src/lib.rs
// src-crates/chdb/src/lib.rs
// src-crates/pglite/src/lib.rs

const name = 'turso'
const version = '20260717'

const base = 'https://assets.dataflare.app/drivers/'
const items = [
    {
        prefix: 'lib',
        suffix: 'dylib',
        target: 'aarch64-apple-darwin',
        cfg: 'target_os = "macos", target_arch = "aarch64"'
    },
    {
        prefix: 'lib',
        suffix: 'dylib',
        target: 'x86_64-apple-darwin',
        cfg: 'target_os = "macos", target_arch = "x86_64"'
    },
    {
        prefix: 'lib',
        suffix: 'so',
        target: 'aarch64-unknown-linux-gnu',
        cfg: 'target_os = "linux", target_arch = "aarch64", target_env = "gnu"'
    },
    {
        prefix: 'lib',
        suffix: 'so',
        target: 'x86_64-unknown-linux-gnu',
        cfg: 'target_os = "linux", target_arch = "x86_64", target_env = "gnu"'
    },
    {
        prefix: '',
        suffix: 'dll',
        target: 'aarch64-pc-windows-msvc',
        cfg: 'target_os = "windows", target_arch = "aarch64", target_env = "msvc"'
    },
    {
        prefix: '',
        suffix: 'dll',
        target: 'x86_64-pc-windows-msvc',
        cfg: 'target_os = "windows", target_arch = "x86_64", target_env = "msvc"'
    }
]

const sha256 = async ({ prefix, target, suffix, cfg }) => {
    const url = `${base}${prefix}${name}-${target}-${version}.${suffix}`
    const res = await fetch(url)
    let hex = ''
    switch (res.status) {
        case 200: {
            const hash = crypto.createHash('sha256')
            for await (const chunk of res.body) {
                hash.update(chunk)
            }
            hex = `"${hash.digest('hex')}";`
            break
        }
        case 404: {
            hex = '""; // Unsupported platform'
            break
        }
        default: {
            throw new Error(`HTTP Error: ${res.status}, ${url}`)
        }
    }
    return `
#[cfg(all(${cfg}))]
const ${name.toUpperCase()}_SHA256: &str = ${hex}
    `.trim()
}

const content = (await Promise.all(items.map(sha256))).join('\n')
console.log(`${name} - ${version}`)
console.log(content)
