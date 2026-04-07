# deepwiki-dl 实现计划

**目标:** 实现一个 Rust CLI 工具，通过 DeepWiki MCP Server API 获取开源仓库文档并保存为本地 Markdown 文件。
**架构:** 同步 HTTP 客户端（ureq）调用 MCP JSON-RPC API，解析响应后输出到 stdout / 目录 / 单文件。lib.rs + main.rs 分离，支持作为库使用。
**技术栈:** Rust, clap, ureq, serde, indicatif, yansi, thiserror, anyhow
**设计文档:** docs/designs/2026-04-07-dwmd-mcp-api.md
**测试模式:** TDD（关键路径） + 非 TDD（脚手架/配置）

**新增依赖：**

| 依赖 | 用途 | 许可证 |
|------|------|--------|
| clap (derive) | CLI 参数解析 | MIT/Apache-2.0 |
| ureq | 同步 HTTP 客户端 | MIT/Apache-2.0 |
| serde + serde_json | JSON 序列化/反序列化 | MIT/Apache-2.0 |
| indicatif | 进度 spinner | MIT |
| yansi | 彩色终端输出 | MIT/Apache-2.0 |
| thiserror | library 层类型化错误 | MIT/Apache-2.0 |
| anyhow | application 层错误处理 | MIT/Apache-2.0 |
| regex | 页面分隔符/链接匹配 | MIT/Apache-2.0 |
| assert_cmd (dev) | CLI 集成测试 | MIT/Apache-2.0 |
| predicates (dev) | 测试断言 | MIT/Apache-2.0 |

---

### Task 1: 项目脚手架  ✅

**文件:**
- 创建: `Cargo.toml`
- 创建: `src/main.rs`
- 创建: `src/lib.rs`
- 创建: `src/types.rs`

**Step 1: 初始化 Cargo 项目**
在项目根目录运行 `cargo init --name deepwiki-dl`，然后编辑 `Cargo.toml` 添加所有依赖和 release profile。

`Cargo.toml` 关键内容：
```toml
[package]
name = "deepwiki-dl"
version = "0.1.0"
edition = "2021"
description = "Download DeepWiki documentation to local Markdown files"
license = "MIT"

[dependencies]
clap = { version = "4", features = ["derive"] }
ureq = { version = "3", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
indicatif = "0.17"
yansi = "1"
thiserror = "2"
anyhow = "1"
regex = "1"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"

[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

**Step 2: 创建 src/types.rs**
定义 `RepoId`（实现 `FromStr`，支持多种 URL 格式归一化）和 `WikiPage`、`WikiStructure` 领域模型。

```rust
// RepoId: owner + repo，实现 FromStr 支持:
//   "owner/repo"
//   "https://deepwiki.com/owner/repo"
//   "https://deepwiki.com/owner/repo/1-some-section"
//   "https://github.com/owner/repo"
//   "https://github.com/owner/repo.git"

// WikiPage: slug, title, depth, content
// WikiStructure: repo, pages: Vec<WikiPageMeta>
// WikiPageMeta: slug, title, depth (无 content)
```

**Step 3: 创建 src/lib.rs**
导出模块声明（mod mcp, wiki, pipeline, writer, types），暂时为空壳。

**Step 4: 创建 src/main.rs**
最小 clap 定义，能解析 `pull` 和 `list` 子命令及所有参数。`pull` 作为默认命令。暂时只打印解析结果。

**验证:**
```bash
cargo build
cargo run -- --help
cargo run -- list anthropics/claude-code
# 预期：编译成功，--help 显示所有参数，list 子命令打印解析结果
```

---

### Task 2: MCP 传输层 — JSON-RPC 类型和 HTTP POST  ✅

**文件:**
- 创建: `src/mcp/mod.rs`
- 创建: `src/mcp/types.rs`
- 创建: `src/mcp/transport.rs`

**Step 1: 写 mcp/types.rs**
定义 JSON-RPC 2.0 类型（Serialize/Deserialize）：
- `JsonRpcRequest { jsonrpc, id, method, params }`
- `JsonRpcResponse { jsonrpc, id, result, error }`
- `JsonRpcError { code, message, data }`
- `ToolResult { content: Vec<ContentBlock>, is_error }`
- `ContentBlock::Text { text }`

**Step 2: 写 mcp/transport.rs**
实现 `McpTransport` struct：
- `new(endpoint, timeout_connect, timeout_read)` → 创建 ureq agent
- `post(request, session_id) -> Result<McpResponse>` → 发送 POST，处理 application/json 和 text/event-stream 两种响应
- SSE 解析器：完整实现（事件边界、多 data 行连接、注释行忽略）
- 响应体大小检查（50MB 上限）
- User-Agent 头：`deepwiki-dl/<version>`
- `Mcp-Session-Id` 头（大小写不敏感提取）

**Step 3: 写 SSE 解析器的单元测试**
覆盖：单事件、多事件、多 data 行、注释行、空行边界、\r\n 处理。

**验证:**
```bash
cargo test mcp::transport
# 预期：SSE 解析器所有测试通过
```

---

### Task 3: MCP 客户端 — 握手、工具调用、重试  ✅

**文件:**
- 修改: `src/mcp/mod.rs`

**Step 1: 实现 McpClient**
- `connect(endpoint) -> Result<McpClient>` — initialize 握手 + notifications/initialized
- `call_tool(name, arguments) -> Result<String>` — tools/call，提取 content[0].text
- session_id 管理（从 initialize 响应头提取，后续请求携带）
- JSON-RPC id 单调递增

**Step 2: 实现重试逻辑**
通用 `retry_with_backoff` 辅助函数：
- 最多 3 次（间隔 1s/2s/4s）
- 区分错误类型：5xx/超时 → 重试，4xx（非 429）→ 不重试，429 → Retry-After
- session 疑似过期 → 重新握手一次

**Step 3: 定义 McpError（thiserror）**
```rust
pub enum McpError {
    HandshakeFailed { attempts: u32, source: ... },
    RepoNotFound { repo: String },
    RpcError { code: i64, message: String },
    ResponseTooLarge { size: u64, max: u64 },
    Timeout { ... },
    Transport { ... },
}
```

**验证:**
```bash
cargo build
# 预期：编译成功，McpClient 的公共 API 可从 lib.rs 导出
```

---

### Task 4: Wiki 结构解析和页面切分（TDD）  ✅

**文件:**
- 创建: `src/wiki/mod.rs`
- 创建: `src/wiki/filter.rs`

**Step 1: 写失败的测试 — 结构解析**
测试 `parse_wiki_structure(text) -> Vec<WikiPageMeta>`：
- 解析 `read_wiki_structure` 返回的文本格式（缩进层级 + 编号 + 标题）
- 提取 slug、title、depth

**Step 2: 跑测试确认失败**
```bash
cargo test wiki::tests::test_parse_structure
# 预期: FAIL
```

**Step 3: 实现 parse_wiki_structure**
宽松解析：用缩进/数字前缀启发式提取层级关系。

**Step 4: 跑测试确认通过**
```bash
cargo test wiki::tests::test_parse_structure
# 预期: PASS
```

**Step 5: 写失败的测试 — 页面切分**
测试 `split_pages(content, structure) -> Vec<WikiPage>`：
- 按 `# Page: <title>` 分隔符切分
- 与 structure 交叉校验数量
- 边界情况：title 含特殊字符、\r\n、第一个分隔符前有前导内容

**Step 6: 实现 split_pages**
正则 `^\s*#\s+Page:\s*(.+?)\s*$`，处理 `\r\n` 和 `\n`。切分后与 structure 数量对比，不一致输出警告。

**Step 7: 跑测试确认通过**
```bash
cargo test wiki::tests
# 预期: 所有测试 PASS
```

**Step 8: 写失败的测试 — 过滤**
测试 `filter_pages(pages, include, exclude) -> Vec<WikiPage>`：
- `--pages 1.1,2.3` 只保留匹配 slug
- `--exclude 7` 排除匹配 slug

**Step 9: 实现 filter.rs**

**Step 10: 跑测试确认通过**
```bash
cargo test wiki
# 预期: 所有测试 PASS
```

---

### Task 5: Pipeline — Markdown 格式化和链接改写（TDD）  ✅

**文件:**
- 创建: `src/pipeline/mod.rs`
- 创建: `src/pipeline/markdown.rs`
- 创建: `src/pipeline/json.rs`

**Step 1: 写失败的测试 — 内部链接改写**
测试 `rewrite_internal_links(content, repo, known_slugs) -> String`：
- `https://deepwiki.com/owner/repo/1.1-xxx` → `./1.1-xxx.md`
- 未知 slug 链接保留原始 URL
- Markdown 链接格式 `[text](url)` 和裸 URL 都处理

**Step 2: 实现链接改写**
用 regex 匹配 `https://deepwiki.com/{owner}/{repo}/{slug}` 模式，检查 slug 是否在已知列表中。

**Step 3: 跑测试确认通过**
```bash
cargo test pipeline::markdown::tests
# 预期: PASS
```

**Step 4: 写失败的测试 — 文件名清理**
测试 `sanitize_filename(slug) -> String`：
- 清理 `/`、`\`、`:`、`?`、`*`、`"`、`<`、`>`、`|`
- Windows/Linux/macOS 兼容

**Step 5: 实现文件名清理**

**Step 6: 写失败的测试 — Markdown 格式化**
测试 `format_directory(pages, repo) -> Vec<(PathBuf, String)>`：
- 每页一个 (path, content) 元组
- 链接已改写

测试 `format_single_file(pages) -> String`：
- `<<< SECTION: Title [slug] >>>` 分隔符

测试 `format_stdout(pages) -> String`：
- 直接拼接，无分隔符

**Step 7: 实现三种格式化器**

**Step 8: 实现 json.rs**
`format_json_list(structure, repo) -> String`：JSON 输出给 `list --json`。

**Step 9: 跑测试确认全部通过**
```bash
cargo test pipeline
# 预期: 所有测试 PASS
```

---

### Task 6: Writer — 文件 I/O  ✅

**文件:**
- 创建: `src/writer.rs`

**Step 1: 实现 writer**
```rust
pub enum Output {
    Stdout(String),
    SingleFile { path: PathBuf, content: String },
    Directory { files: Vec<(PathBuf, String)> },
}

pub fn write_output(output: Output) -> Result<()>
```
- `Stdout` → print to stdout
- `SingleFile` → 创建父目录 + 写入文件
- `Directory` → 创建目录结构 + 逐文件写入

**验证:**
```bash
cargo test writer
# 预期: PASS（测试用 tempdir）
```

---

### Task 7: Pipeline 编排 — 串联所有模块  ✅

**文件:**
- 修改: `src/pipeline/mod.rs`
- 修改: `src/lib.rs`

**Step 1: 实现 pipeline/mod.rs 编排逻辑**
```rust
pub fn pull(repo: &RepoId, options: &PullOptions) -> Result<Output>
// 1. McpClient::connect()
// 2. call read_wiki_structure → parse
// 3. filter pages
// 4. call read_wiki_contents → split
// 5. 根据 output_mode 格式化
// 6. 返回 Output

pub fn list(repo: &RepoId, json: bool) -> Result<String>
// 1. McpClient::connect()
// 2. call read_wiki_structure → parse
// 3. 格式化为 text 或 JSON
```

**Step 2: 在 lib.rs 中导出公共 API**
```rust
pub use types::{RepoId, WikiPage, WikiStructure};
pub use pipeline::{pull, list, PullOptions, OutputMode};
pub use writer::{write_output, Output};
```

**Step 3: 在 main.rs 中接入**
将 clap 解析结果传递给 lib.rs 的 `pull`/`list`，调用 `write_output` 输出。进度 spinner（indicatif）和彩色输出（yansi）在 main.rs 中处理。

**验证:**
```bash
cargo build
cargo run -- list anthropics/claude-code
# 预期：输出 claude-code 的章节目录（实际调用 DeepWiki MCP API）

cargo run -- anthropics/claude-code
# 预期：stdout 输出完整 Markdown 内容

cargo run -- anthropics/claude-code -o ./test-output/
# 预期：创建 test-output/ 目录，每个章节一个 .md 文件
```

---

### Task 8: Mermaid 渲染  ✅

**文件:**
- 创建: `src/pipeline/mermaid.rs`
- 修改: `src/pipeline/mod.rs`

**Step 1: 写失败的测试 — Mermaid 代码块提取**
测试 `extract_mermaid_blocks(content) -> Vec<MermaidBlock>`：
- 从 Markdown 中提取 ` ```mermaid ` 代码块及其位置

**Step 2: 实现提取逻辑**

**Step 3: 实现渲染逻辑**
`render_mermaid(blocks, format, output_dir) -> Result<Vec<(original, replacement)>>`：
- 检查 `mmdc` 是否可用（`which mmdc`）
- 逐个调用 `mmdc -i input.mmd -o output.svg -e <format>`
- 返回替换映射（原始代码块 → `![](assets/mermaid/xxx.svg)`）
- mmdc 不存在时返回警告，不替换

**Step 4: 在 pipeline 编排中集成**
`--mermaid` 且有 `-o` 时，在写入前对每页内容执行 Mermaid 渲染替换。

**验证:**
```bash
cargo test pipeline::mermaid
# 预期: 提取测试 PASS（渲染测试需 mmdc，标记 #[ignore] 或 mock）
```

---

### Task 9: 错误处理和用户体验  ✅

**文件:**
- 修改: `src/main.rs`
- 修改: `src/mcp/mod.rs`

**Step 1: 完善 main.rs 的错误展示**
- 非 verbose 模式：简洁的用户友好消息
- verbose 模式：完整错误链
- quiet 模式：只输出到 stderr
- `--no-color`：禁用 yansi 颜色

**Step 2: 完善 MCP 客户端的错误场景**
- 仓库未收录 → `"Repository not indexed by DeepWiki. Visit https://deepwiki.com to add it."`
- 响应超大 → `"Response too large (XXM). Try --pages to fetch specific sections."`
- `--mermaid` 无 `-o` → `"--mermaid requires -o to specify output directory or file."`

**Step 3: 进度展示**
- 非 quiet 模式下，每步显示 spinner：`Connecting...`、`Fetching structure...`、`Fetching contents...`、`Writing files...`
- spinner 输出到 stderr

**验证:**
```bash
cargo run -- nonexistent/repo 2>&1
# 预期：友好的错误信息

cargo run -- anthropics/claude-code -v 2>/dev/null | head -5
# 预期：stdout 只有 Markdown 内容，stderr 有详细日志
```

---

### Task 10: 集成测试  ✅

**文件:**
- 创建: `tests/cli.rs`

**Step 1: 写 CLI 集成测试**
使用 `assert_cmd` 测试完整 CLI 流程：
- `deepwiki-dl --help` → 退出码 0，包含 "deepwiki-dl"
- `deepwiki-dl --version` → 退出码 0
- `deepwiki-dl` (无参数) → 退出码非 0，stderr 包含提示
- `deepwiki-dl pull --mermaid svg` (无 -o) → 退出码非 0，stderr 包含 "--mermaid requires -o"

注意：涉及实际 MCP API 调用的测试标记 `#[ignore]`，CI 中不运行。

**验证:**
```bash
cargo test --test cli
# 预期: 所有非 ignore 测试 PASS
```

---

### Task 11: README 和分发准备  ✅

**文件:**
- 修改: `README.md`
- 创建: `.github/workflows/release.yml`

**Step 1: 编写 README.md**
包含：项目描述、安装方式（cargo install / GitHub Releases / Homebrew）、使用示例、参数说明、免责声明。

**Step 2: 创建 GitHub Actions release workflow**
- 触发：push tag `v*`
- 6 个 target 交叉编译（Linux musl 用 cross，macOS/Windows 用 cargo）
- 上传二进制到 GitHub Releases
- 发布到 crates.io

**验证:**
```bash
cargo build --release
ls -lh target/release/deepwiki-dl*
# 预期：二进制文件存在，大小 4-6MB 范围
```

---

## 任务依赖图

```
Task 1 (脚手架)
  ├→ Task 2 (MCP 传输层)
  │    └→ Task 3 (MCP 客户端)
  ├→ Task 4 (Wiki 解析) ←── 可与 Task 2/3 并行
  ├→ Task 5 (Pipeline 格式化) ←── 可与 Task 2/3 并行
  └→ Task 6 (Writer) ←── 可与 Task 2/3 并行
       │
       ↓
  Task 7 (编排串联) ←── 依赖 Task 2-6 全部完成
    ├→ Task 8 (Mermaid)
    ├→ Task 9 (错误处理/UX)
    └→ Task 10 (集成测试)
         └→ Task 11 (README/分发)
```

Task 4、5、6 之间无依赖，可并行开发。
