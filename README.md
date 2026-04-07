# deepwiki-dl

Download [DeepWiki](https://deepwiki.com) documentation for open-source repositories to local Markdown files.

Uses the official DeepWiki MCP Server API — no browser automation, no scraping.

## Installation

### From GitHub Releases

Download the latest binary for your platform from [Releases](https://github.com/cecil-su/deepwiki-to-md/releases).

### From Cargo

```bash
cargo install deepwiki-dl
```

### From Source

```bash
git clone https://github.com/cecil-su/deepwiki-to-md.git
cd deepwiki-to-md
cargo install --path .
```

## Usage

### Pull documentation

```bash
# Output to stdout (pipe-friendly)
deepwiki-dl anthropics/claude-code

# Save to directory (one .md file per section)
deepwiki-dl anthropics/claude-code -o ./docs/

# Save as single file
deepwiki-dl anthropics/claude-code -o wiki.md

# Only specific sections
deepwiki-dl anthropics/claude-code --pages 1,2 -o ./docs/

# Exclude sections
deepwiki-dl anthropics/claude-code --exclude 7 -o ./docs/

# Render mermaid diagrams to SVG (requires mmdc)
deepwiki-dl anthropics/claude-code -o ./docs/ --mermaid svg
```

### List sections

```bash
# Human-readable list
deepwiki-dl list anthropics/claude-code

# JSON output
deepwiki-dl list anthropics/claude-code --json
```

### URL input

You can paste URLs directly:

```bash
deepwiki-dl https://deepwiki.com/anthropics/claude-code
deepwiki-dl https://github.com/anthropics/claude-code
```

## Output Modes

| Usage | Mode |
|-------|------|
| No `-o` | stdout (pipe to other tools) |
| `-o ./dir/` | Directory (one .md per section) |
| `-o file.md` | Single file with section separators |

## Options

| Option | Short | Description |
|--------|-------|-------------|
| `--output` | `-o` | Output directory or file path |
| `--mermaid FORMAT` | | Render mermaid to `svg` or `png` (requires `-o` and `mmdc`) |
| `--pages SLUGS` | `-p` | Only fetch specific sections (comma-separated) |
| `--exclude SLUGS` | `-x` | Exclude specific sections (comma-separated) |
| `--timeout SECS` | `-t` | Request timeout in seconds (default: 30) |
| `--verbose` | `-v` | Show detailed logs |
| `--quiet` | `-q` | Only output errors |
| `--no-color` | | Disable colored output |
| `--json` | | JSON output (for `list` command) |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `DEEPWIKI_DL_MCP_ENDPOINT` | Override MCP server endpoint (default: `https://mcp.deepwiki.com/mcp`) |

## Disclaimer

This tool uses the official DeepWiki MCP Server API provided by Cognition Labs. It does not scrape or crawl the DeepWiki website. Users are responsible for complying with [Cognition Labs' Terms of Service](https://cognition.ai/terms-of-service) and [Acceptable Use Policy](https://cognition.ai/acceptable-use-policy). This tool is not affiliated with or endorsed by Cognition Labs.

## License

MIT
