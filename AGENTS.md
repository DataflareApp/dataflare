# AGENTS.md

Dataflare is a database GUI client written in Tauri.

## Code Style

- No comments for obvious code

## For TypeScript

- Prefer arrow functions
- Callback arrow functions with block bodies must use an explicit `return` when returning a value; single-expression callbacks like `=> a.b.c` are exempt
- When checking value of finite enum type, prefer `switch` over `if`

## For Rust

- For paths that are used multiple times within a mod, they should be imported using a `use` declaration
- By default, do not use `--release` flag
- When running checks or tests, use the smallest relevant scope. Only run full-workspace checks or tests when necessary
