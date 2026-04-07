# deepwiki-dl

CLI tool to download DeepWiki analysis documents for open-source repositories to local markdown files via official MCP Server API.

## Project Overview

- **Purpose**: Pull documentation from DeepWiki via MCP Server API (`https://mcp.deepwiki.com/mcp`) and save as local markdown files
- **Language**: Rust
- **Data Source**: DeepWiki MCP Server (JSON-RPC 2.0, no auth required)

## Development

```bash
cargo build          # Build
cargo run -- pull owner/repo  # Run
cargo test           # Run tests
cargo clippy         # Lint
```

## Project Structure

```
src/
  lib.rs            # Library entry, public API
  main.rs           # CLI entry, clap parsing
  mcp/              # MCP client (handshake, tool calls, SSE parsing)
  wiki/             # Wiki structure parsing and filtering
  pipeline/         # Orchestration: fetch → filter → render → write
  writer.rs         # File I/O
  types.rs          # Domain models (RepoId, WikiPage, WikiStructure)
```

## Conventions

- Synchronous HTTP (ureq), no async runtime
- Error handling: thiserror (library) + anyhow (application)
- Progress/errors to stderr, content to stdout
- All output files use UTF-8 encoding
- Error messages should be user-friendly and actionable
