# deepwiki-to-md CLI 工具

**日期:** 2026-04-07

## 背景

DeepWiki (https://deepwiki.com) 为开源仓库提供 AI 生成的分析文档，内容包含架构分析、代码解读、Mermaid 图表等。但文档只能在线浏览，无法离线使用或纳入本地知识库。本工具的目标是将 DeepWiki 文档拉取到本地，转为标准 Markdown 文件。

> **限制：** 仅支持公开仓库。使用前需确认 DeepWiki 的 robots.txt 和 ToS 合规性。

## 讨论

### 技术栈选型

讨论了 Node.js/TypeScript、Python、Rust、Go 四种方案：
- **Rust**：浏览器自动化生态不成熟（无 Playwright 官方支持），开发周期长 2-3 倍
- **Go**：chromedp 可用但不如 Playwright 成熟
- **Python**：Playwright 官方支持，可行但团队偏好 Node.js
- **Node.js/TypeScript**：Playwright 一等支持，生态丰富，最终选定

### 数据获取策略

讨论了三种方案：
- **逐页抓取**：简单但慢
- **并发抓取**（选定）：Playwright 多 tab 并发，速度快 3-5 倍，p-limit 控制并发
- **拦截 RSC 流**：最快但强依赖 deepwiki 内部数据格式，易失效

### 输出格式

- 默认目录结构（每页一个 .md）+ `--single-file` 合并为单文件
- Mermaid 默认保留代码块，`--render-mermaid` 可选渲染为图片

### 分发方式

npm 发布，支持 `npx deepwiki-to-md` 直接运行和全局安装。同时注册短别名 `dwmd`。

## 方案

### CLI 命令设计

```
dwmd <owner/repo>                             # pull 为默认命令
dwmd pull <owner/repo>                        # 全量拉取（等同上面）
dwmd pull <owner/repo> --force                # 忽略缓存，强制全量拉取
dwmd pull <owner/repo> --single-file          # 合并为单文件
dwmd pull <owner/repo> --mermaid svg          # mermaid 转图片
dwmd list <owner/repo>                        # 列出章节目录
dwmd list <owner/repo> --json                 # JSON 格式输出
dwmd index <owner/repo>                       # 触发 deepwiki 索引（实验性）
```

**输入格式容错** — 以下格式均可接受，自动归一化为 `owner/repo`：
- `owner/repo`
- `https://deepwiki.com/owner/repo`
- `https://deepwiki.com/owner/repo/1-some-section`
- `https://github.com/owner/repo`
- `https://github.com/owner/repo.git`

### 参数一览

| 参数 | 缩写 | 默认值 | 说明 |
|------|------|--------|------|
| `--output` | `-o` | `./{owner}-{repo}` | 输出目录或文件路径 |
| `--single-file` | `-s` | false | 合并为单个 markdown 文件 |
| `--mermaid` | | 不启用 | 将 mermaid 渲染为指定格式（`svg` 或 `png`） |
| `--concurrency` | `-j` | 5 | 并发抓取页面数 |
| `--lang` | `-l` | `en` | 文档语言 |
| `--force` | `-f` | false | 忽略缓存，强制全量拉取 |
| `--skip-images` | | false | 跳过图片等外部资源下载 |
| `--verbose` | `-v` | false | 显示详细日志 |
| `--quiet` | `-q` | false | 静默模式，只输出错误 |
| `--no-color` | | false | 禁用颜色输出 |

### 核心流程

```
用户输入 owner/repo（或 URL，自动归一化）
      ↓
检查 DeepWiki robots.txt 合规性
      ↓
启动 Playwright (优先系统 Chrome，回退到内置 Chromium)
      ↓
访问 deepwiki.com/{owner}/{repo}
      ↓
健康检测：验证关键选择器是否有效
      ↓
等待左侧导航树渲染完成（DOM 稳定性检测）
      ↓
提取导航结构 → [{title, url, level, order}]
      ↓
检查本地 .deepwiki-meta.json（增量模式判断）
      ↓
并发(5)访问需要抓取的子页面（用完即关 tab）
      ↓
每个页面：DOM 稳定性检测 → 提取 HTML → 转 markdown
  ├─ 改写内部链接（DeepWiki URL → 本地文件路径）
  ├─ 提取 Mermaid 源码（从 RSC payload 或渲染前 DOM 拦截）
  └─ 每页完成后立即写入文件 + 更新 meta
      ↓
下载外部图片资源 → assets/images/（失败则保留原始 URL）
      ↓
完成报告（成功/跳过/失败页面汇总）
```

## 约束与非功能需求

- DeepWiki 使用 Next.js RSC 流式渲染，内容通过 JS 动态加载，不能依赖 `networkidle`，必须使用 DOM 稳定性检测（连续两次间隔 500ms 的 innerHTML 一致）
- 需要控制并发防止被 deepwiki 限流（默认 5 并发），页面间增加 500ms-1s 随机延迟
- 单页加载超时 30s，重试 1 次，仍失败则跳过并汇报
- 每页完成后立即写入文件和更新 meta，支持真正的断点续传
- 所有输出文件 UTF-8 编码
- 文件名需清理特殊字符（`/`、`\`、`:`、`?` 等），兼容 Windows/Linux/macOS

## 架构

### 技术栈

| 用途 | 选型 |
|------|------|
| CLI 框架 | `commander` |
| 浏览器自动化 | `playwright-core`（优先系统 Chrome）+ `playwright`（回退内置 Chromium） |
| HTML → Markdown | `turndown` + `turndown-plugin-gfm` + 自定义规则 |
| 并发控制 | `p-limit` |
| 进度展示 | `ora` + `chalk` |
| Mermaid 渲染 | `@mermaid-js/mermaid-cli`（可选依赖，未安装时降级为保留代码块并提示） |
| 构建 | `tsup` |
| 测试 | `vitest` |

### 模块划分

```
src/
  cli.ts            # CLI 入口，commander 命令定义和参数解析，URL 归一化
  fetcher.ts        # Playwright 页面抓取，DOM 稳定性检测，导航树提取，并发控制
  parser.ts         # HTML → Markdown 转换，自定义 turndown 规则，内部链接改写，mermaid 源码提取
  writer.ts         # 文件写入，目录创建，单文件合并，逐页 meta 更新
  selectors.ts      # DOM 选择器集中管理（导航树、内容区域、时间戳等），便于 DeepWiki 改版时快速更新
  types.ts          # 共享类型定义
```

### 选择器策略与韧性设计

所有 DOM 选择器集中在 `selectors.ts` 中管理：
- 优先使用语义化选择器（`nav a[href]`、`role=`、`text=`）
- 每个选择器提供 fallback 链（主选择器 → 备选选择器 → 报错并提示更新工具版本）
- 抓取前执行健康检测：验证导航树和内容区域的关键选择器是否有效，失败则给出明确提示

### 输出文件结构

**目录模式（默认）：**

```
{owner}-{repo}/
  README.md                          # 仓库首页内容
  1-claude-code-overview.md
  1.1-system-architecture.md
  ...
  assets/
    images/                          # 外部图片
    mermaid/                         # --mermaid 时生成
  .deepwiki-meta.json                # 增量更新元数据
```

**单文件模式：**

```
{owner}-{repo}.md                    # 所有章节按导航顺序拼接，--- 分隔
```

单文件模式下图片保留原始 URL（不下载），或使用 `assets/` 目录搭配单文件。

### 增量更新机制

默认行为即为增量：检测到 `.deepwiki-meta.json` 时自动启用。
- meta 记录每页 URL、内容 hash（MD5）和仓库级 "Last indexed" 时间戳
- 重新运行时：仓库时间戳未变 → 全部跳过；时间戳变了 → 逐页对比 hash，只重新抓取内容变化的页面
- `--force` 忽略 meta，强制全量拉取
- meta 不存在时视为首次拉取（全量）
- 每页完成后立即更新 meta（而非最后统一写入），确保中断后可续传

### 内部链接改写

页面中指向其他 DeepWiki 页面的链接自动改写为本地文件相对路径：
- `https://deepwiki.com/owner/repo/1.1-xxx` → `./1.1-xxx.md`
- 指向未下载页面的链接保留原始 URL

### index 命令（实验性）

> **注意：此命令标记为实验性，可能因 DeepWiki 界面变更而失效。**

- 优先尝试调用 DeepWiki API（如存在）
- 回退方案：用 Playwright 模拟 "Add repo" 提交流程
- 默认阻塞等待索引完成（spinner + 进度），`--no-wait` 提交后立即返回
- 失败时给出手动操作指引："请在浏览器中访问 deepwiki.com 手动添加仓库"
- 仓库已收录时直接提示

### Mermaid 处理策略

1. 通过 `page.evaluate()` 在 Mermaid.js 渲染前提取原始代码块文本
2. 备选方案：从 RSC payload（`self.__next_f`）中提取 Mermaid 源码
3. 如果两者都失败，从已渲染的 SVG 元素回退提取
4. `--mermaid <svg|png>` 调用 `@mermaid-js/mermaid-cli` 渲染为图片；未安装时降级为保留代码块并提示安装

### 浏览器策略

1. 优先使用 `playwright-core` + 系统已安装的 Chrome/Edge（`channel: 'chrome'`），避免额外下载
2. 系统无可用浏览器时，回退到 `playwright` 内置 Chromium
3. 首次下载 Chromium 时给出明确提示和进度
4. 支持 `PLAYWRIGHT_BROWSERS_PATH` 和 `PLAYWRIGHT_DOWNLOAD_HOST` 环境变量

### 错误处理

- 仓库不存在 / 未收录 → 检测 404，明确提示并给出可复制的 `index` 命令
- 选择器失效（DeepWiki 改版）→ 健康检测失败，提示用户更新工具版本或提交 issue
- 页面加载超时 → 30s 超时，重试 1 次，跳过并汇报
- 图片下载失败 → 保留原始 URL，最终报告中列出失败项
- 网络中断 → 已下载内容和 meta 已逐页保存，提示重新运行即可续传
- `--mermaid` 但未安装 mermaid-cli → 降级为保留代码块并提示安装

### Turndown 自定义规则

需要为以下 DeepWiki 特有元素编写自定义 turndown 规则：
- `<details>/<summary>` 折叠块（源文件引用）
- Mermaid SVG → 还原为代码块
- 清理 React 特有属性
- 保留 GitHub 源文件链接及行号范围
- 复杂嵌套表格

### 测试策略

- **单元测试**：导航树解析、HTML→Markdown 转换（覆盖各内容类型：表格、代码块、嵌套列表、折叠块、Mermaid、图片）、文件路径生成、URL 归一化、选择器 fallback、内部链接改写
- **集成测试**：本地 mock HTML 页面 + Playwright 完整流程，不依赖在线服务

### 合规与法律

- 工具启动时检查 DeepWiki 的 robots.txt，遵守 Disallow 规则
- 设置合理的 User-Agent 标识（包含工具名和版本）
- 请求间增加随机延迟，模拟正常浏览行为
- README 中包含免责声明，说明使用者需自行遵守 DeepWiki 的 Terms of Service
- 明确声明仅支持公开仓库
