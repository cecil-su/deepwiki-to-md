# deepwiki-dl — DeepWiki to Markdown CLI（MCP API 方案）

**日期:** 2026-04-07

## 背景

DeepWiki (https://deepwiki.com) 为开源仓库提供 AI 生成的分析文档。本工具通过 DeepWiki 官方 MCP Server API 获取文档内容，转为本地 Markdown 文件。

### 方案演进

初始方案使用 Playwright 无头浏览器抓取 DeepWiki 页面。经多角色评审后发现：
1. **合规风险**：Cognition Labs 的 AUP 明确禁止 crawling/scraping/harvesting
2. **官方 API 存在**：DeepWiki 提供免费、无需认证的 MCP Server（`https://mcp.deepwiki.com/mcp`），返回 Markdown 格式内容
3. **架构大幅简化**：无需浏览器、无需 HTML→Markdown 转换、无需 DOM 选择器管理

因此切换至 MCP API 方案，同时因浏览器自动化不再是约束，技术栈从 Node.js/TypeScript 切换至 Rust（单二进制零依赖分发）。

### 与 dw2md 的关系

已有先例工具 [dw2md](https://github.com/tnguyen21/dw2md)（Rust，MIT）。本项目独立发布，差异化功能：
- **目录模式输出**（dw2md 仅单文件）
- **内部链接改写**（DeepWiki URL → 本地相对路径）
- **Mermaid 可选渲染为图片**
- **同时作为 Rust library crate 发布**

## 讨论

### 技术栈选型

初始讨论了 Node.js/TypeScript、Python、Rust、Go：
- 最初选定 Node.js（因 Playwright 一等支持）
- 切换至 MCP API 后，浏览器自动化不再是约束
- Rust 的单二进制零依赖分发优势凸显，且 dw2md 已验证 Rust + MCP API 的可行性
- 最终选定 **Rust**

### 数据获取策略

MCP Server 提供两个关键工具：
- `read_wiki_structure`：返回章节目录文本
- `read_wiki_contents`：**一次返回所有页面 Markdown 内容**

只需 2-3 次 HTTP 请求（initialize + structure + contents），无需并发控制。因此采用**同步方案**（ureq），不需要 tokio 异步运行时。

### 写入策略

每次全量覆盖，不做增量对比。原因：MCP API 每次返回全部内容，无法减少网络请求。

## 方案

### CLI 命令设计

```
deepwiki-dl <owner/repo>                          # 默认输出到 stdout
deepwiki-dl <owner/repo> -o ./docs/               # 输出到目录（自动目录模式）
deepwiki-dl <owner/repo> -o wiki.md               # 输出到文件（单文件模式）
deepwiki-dl pull <owner/repo>                     # 同上，pull 为默认命令可省略
deepwiki-dl pull <owner/repo> -o ./docs/ --mermaid svg  # mermaid 转图片（需指定 -o）
deepwiki-dl pull <owner/repo> --pages 1.1,2.3     # 只拉取指定章节
deepwiki-dl pull <owner/repo> --exclude 7         # 排除指定章节
deepwiki-dl list <owner/repo>                     # 列出章节目录
deepwiki-dl list <owner/repo> --json              # JSON 格式输出
```

**输出模式逻辑：**
- 不指定 `-o` → stdout（管道友好）
- `-o` 以 `/` 结尾或指向已存在的目录 → 目录模式（每页一个 .md）
- `-o` 其他（如 `wiki.md`）→ 单文件模式

**输入格式容错** — 以下格式均可接受，自动归一化为 `owner/repo`：
- `owner/repo`
- `https://deepwiki.com/owner/repo`
- `https://deepwiki.com/owner/repo/1-some-section`
- `https://github.com/owner/repo`
- `https://github.com/owner/repo.git`

### 参数一览

| 参数 | 缩写 | 默认值 | 说明 |
|------|------|--------|------|
| `--output` | `-o` | stdout | 输出目录或文件路径（自动推断输出模式） |
| `--mermaid` | | 不启用 | 将 mermaid 渲染为指定格式（`svg` 或 `png`），需配合 `-o` 使用 |
| `--pages` | `-p` | 全部 | 只拉取指定章节（逗号分隔 slug） |
| `--exclude` | `-x` | 无 | 排除指定章节 |
| `--timeout` | `-t` | 30s（连接）/ 120s（读取） | 请求超时 |
| `--verbose` | `-v` | false | 显示详细日志 |
| `--quiet` | `-q` | false | 静默模式，只输出错误 |
| `--no-color` | | false | 禁用颜色输出 |

### 核心流程

```
用户输入 owner/repo（或 URL，自动归一化为 RepoId）
      ↓
MCP 握手：POST initialize → 获取 session_id
      ↓
调用 read_wiki_structure → 解析章节目录
      ↓
应用 --pages / --exclude 过滤
      ↓
调用 read_wiki_contents → 获取全部页面 Markdown
      ↓
按 "# Page: <title>" 分隔符切分为独立页面
  └─ 与 structure 返回的页面列表交叉校验切分结果
      ↓
根据输出模式：
  ├─ stdout → 直接输出（管道友好）
  ├─ 目录模式 → 写入独立文件 + 改写内部链接
  └─ 单文件模式 → 拼接写入
      ↓
可选：--mermaid → 调用系统 mmdc 渲染图片
      ↓
完成报告（stderr，不污染 stdout 管道）
```

### MCP Server API 细节

**端点：** `https://mcp.deepwiki.com/mcp`（可通过 `DEEPWIKI_DL_MCP_ENDPOINT` 环境变量覆盖）

**协议：** JSON-RPC 2.0 over HTTP POST

**请求头：**
```
Content-Type: application/json
Accept: application/json, text/event-stream
Mcp-Session-Id: <session_id>   // 初始化后携带（大小写不敏感匹配）
User-Agent: deepwiki-dl/<version>
```

**初始化握手（两步）：**

第一步 — `initialize` 请求：
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-03-26",
    "capabilities": {},
    "clientInfo": { "name": "deepwiki-dl", "version": "0.1.0" }
  }
}
```

第二步 — `notifications/initialized` 通知（fire-and-forget，但检查 HTTP 状态码）：
```json
{
  "jsonrpc": "2.0",
  "method": "notifications/initialized",
  "params": {}
}
```

**工具调用格式：**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "read_wiki_structure",
    "arguments": { "repoName": "owner/repo" }
  }
}
```

**响应格式：**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [{ "type": "text", "text": "..." }],
    "isError": false
  }
}
```

响应可能为 `application/json` 或 `text/event-stream`（SSE），需完整 SSE 解析器（非简单逐行分割）。

**Session 管理：**
- 从 initialize 响应头提取 `mcp-session-id`（大小写不敏感）
- 后续请求失败且疑似 session 过期时，自动重新握手一次

**JSON-RPC ID 管理：**
- 单调递增，每次重试使用新 id

## 约束与非功能需求

- 仅支持公开仓库
- 使用 DeepWiki 官方 MCP Server API，合规使用
- 连接超时 30s，读取超时 120s（SSE 流式响应可能耗时较长）
- 重试 3 次（指数退避 1s/2s/4s），区分错误类型：5xx/超时重试、4xx 不重试、429 遵守 Retry-After、session 错误重新握手
- 响应体大小上限 50MB
- 文件名清理特殊字符（`/`、`\`、`:`、`?` 等），兼容 Windows/Linux/macOS
- 所有输出文件 UTF-8 编码
- 进度/错误信息输出到 stderr，不污染 stdout 管道
- README 包含免责声明

## 架构

### 技术栈

| 用途 | 选型 |
|------|------|
| 语言 | Rust（同步方案，无 async） |
| CLI 框架 | `clap`（derive 模式） |
| HTTP 客户端 | `ureq`（同步，rustls TLS） |
| JSON 处理 | `serde` + `serde_json` |
| 进度展示 | `indicatif`（spinner） |
| 彩色输出 | `yansi` |
| 错误处理 | `thiserror`（library 层）+ `anyhow`（application 层） |
| Mermaid 渲染 | 调用系统 `mmdc` 命令（可选） |
| 测试 | 内置 `#[test]` + `assert_cmd` + `predicates` |

### 模块划分

```
src/
  lib.rs            # 库入口，导出公共 API
  main.rs           # CLI 入口，clap 参数解析，调用 lib.rs
  mcp/
    mod.rs           # MCP 客户端：初始化握手、工具调用、session 管理、重试
    transport.rs     # HTTP 传输层：POST 请求、完整 SSE 解析器
    types.rs         # JSON-RPC 2.0 类型定义（JsonRpcRequest/Response/Error, ToolResult）
  wiki/
    mod.rs           # 文档结构解析、页面内容切分（# Page: 分隔 + 交叉校验）
    filter.rs        # --pages / --exclude 过滤逻辑
  pipeline/
    mod.rs           # 编排层：fetch → filter → render → write
    markdown.rs      # Markdown 格式化：目录模式 / 单文件模式 / 链接改写
    json.rs          # JSON 输出格式
  writer.rs          # 文件 I/O：目录创建、文件写入（接收格式化后的内容）
  types.rs           # 领域模型：RepoId（实现 FromStr）、WikiPage、WikiStructure
```

**关键设计原则：**
- `main.rs` 只做 CLI 解析，所有逻辑在 `lib.rs` 导出的函数中
- pipeline 的 formatter 输出 `Vec<(PathBuf, String)>`，writer 只做 I/O
- 领域类型（`types.rs`）和传输类型（`mcp/types.rs`）分离

### 输出模式

**stdout（默认，不指定 -o）：**
```
直接输出 Markdown 内容到 stdout，管道友好
进度/错误信息输出到 stderr
```

**目录模式（-o 指向目录）：**
```
{owner}-{repo}/
  README.md                          # 仓库首页/概览
  1-overview.md
  1.1-system-architecture.md
  ...
  assets/
    mermaid/                         # --mermaid 时生成
      1.1-001.svg
```

**单文件模式（-o 指向 .md 文件）：**
```
{owner}-{repo}.md                    # <<< SECTION: Title [slug] >>> 分隔
```

单文件采用 `<<< SECTION: Title [slug] >>>` 分隔符，与 dw2md 兼容。

**JSON 输出（list --json）：**
```json
{
  "repo": "owner/repo",
  "pages": [
    { "slug": "1-overview", "title": "Overview", "depth": 0 },
    { "slug": "1.1-system-architecture", "title": "System Architecture", "depth": 1 }
  ]
}
```

### 内容切分策略

`read_wiki_contents` 返回所有页面合并的文本，用 `# Page: <title>` 分隔。

**防御性解析：**
- 正则宽松匹配：`^\s*#\s+Page:\s*(.+?)\s*$`，同时处理 `\r\n` 和 `\n`
- 切分后与 `read_wiki_structure` 返回的页面列表交叉校验数量
- 数量不一致时发出警告（stderr），不静默丢弃
- verbose 模式下输出原始响应文本，便于调试

### 内部链接改写（目录模式）

MCP 返回的 Markdown 中可能包含 DeepWiki 链接，目录模式下自动改写：
- `https://deepwiki.com/owner/repo/1.1-xxx` → `./1.1-xxx.md`
- 指向未下载页面的链接保留原始 URL
- stdout 和单文件模式下不改写

### Mermaid 处理

MCP 返回的 Markdown 已包含原始 Mermaid 代码块，默认保留。
`--mermaid <svg|png>` 时：
1. **必须配合 `-o` 使用**（stdout 模式下无处存放图片文件，报错提示）
2. 扫描 Markdown 中的 ` ```mermaid ` 代码块
3. 调用系统 `mmdc`（mermaid-cli）渲染为指定格式
4. 保存到 `assets/mermaid/`，替换代码块为图片引用
5. `mmdc` 未安装时降级为保留代码块并提示安装

### SSE 解析器

完整实现 SSE 规范，不是简单逐行分割：
- 正确处理事件边界（空行分隔）
- 多 `data:` 行用 `\n` 连接
- 忽略注释行（以 `:` 开头）
- 按 JSON-RPC `id` 匹配响应（非盲目取最后一条）
- 区分正常流结束和异常中断，异常中断触发重试

### 错误处理

| 场景 | 行为 |
|------|------|
| MCP 握手失败 | 重试 3 次（1s/2s/4s），仍失败报错退出 |
| Session 过期 | 自动重新握手一次，再失败报错 |
| `read_wiki_structure` 返回空 | 提示仓库未被 DeepWiki 收录，建议访问 deepwiki.com 手动添加 |
| `read_wiki_contents` 超时 | 读取超时 120s，重试 3 次 |
| HTTP 5xx / 连接超时 | 重试 |
| HTTP 4xx（非 429） | 不重试，直接报错 |
| HTTP 429 | 遵守 Retry-After 头 |
| JSON-RPC error | 打印错误码、消息和 data（verbose），建议用户稍后重试 |
| 响应体超过 50MB | 报错，建议使用 --pages 分批拉取 |
| 页面切分数量与目录不一致 | 警告（stderr），继续处理已切分的内容 |
| 文件写入失败 | 报错并跳过，继续处理其余页面 |
| `--mermaid` 但未指定 `-o` | 报错，提示需配合 `-o` 使用 |
| `--mermaid` 但 mmdc 未安装 | 降级为保留代码块，提示安装 |
| 非 JSON 响应体 | 输出前 500 字符帮助诊断 |

### 分发

| 渠道 | 方式 |
|------|------|
| GitHub Releases | 预编译二进制（6 target），CI 自动构建 |
| Homebrew | `brew install owner/tap/deepwiki-dl` |
| Cargo | `cargo install deepwiki-dl` |
| crates.io（库） | 同时发布为 library crate |

**交叉编译目标：**

| Target | OS | Runner | 工具 |
|--------|----|--------|------|
| x86_64-unknown-linux-musl | Linux | ubuntu-latest | cross |
| aarch64-unknown-linux-musl | Linux ARM64 | ubuntu-latest | cross |
| x86_64-apple-darwin | macOS Intel | macos-latest | cargo |
| aarch64-apple-darwin | macOS Apple Silicon | macos-latest | cargo |
| x86_64-pc-windows-msvc | Windows | windows-latest | cargo |
| aarch64-pc-windows-msvc | Windows ARM64 | windows-latest | cargo |

Linux 使用 musl 静态链接，TLS 使用 rustls（纯 Rust，零系统依赖）。

**Release profile：**
```toml
[profile.release]
opt-level = "z"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

预估二进制大小：4-6MB。

### 测试策略

- **单元测试**：URL 归一化（RepoId::FromStr）、章节目录解析、页面内容切分（含边界情况）、内部链接改写、文件名清理、`--pages`/`--exclude` 过滤、SSE 解析
- **集成测试**：mock HTTP server 模拟 MCP 端点，跑完整 CLI 流程（`assert_cmd` + `predicates`）
- 不依赖 DeepWiki 在线服务
