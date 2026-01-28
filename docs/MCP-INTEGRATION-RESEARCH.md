# MCP 集成调研报告：Documentation、Code Search、Slack

> 调研日期: 2026-01-28
> 目标: 提升 AI 代码质量、降低幻觉、实现通知

---

## 目录

- [调研结论](#调研结论)
- [1. Documentation MCP](#1-documentation-mcp)
- [2. Codebase Search MCP](#2-codebase-search-mcp)
- [3. Slack MCP](#3-slack-mcp)
- [集成方案](#集成方案)
- [配置模板](#配置模板)

---

## 调研结论

### Claude Code 官方插件 (推荐)

Claude Code 官方插件目录 (`claude-plugins-official`) 已提供以下相关插件：

| 插件 | 安装命令 | 状态 |
|------|----------|------|
| **context7** | `claude plugin install context7@claude-plugins-official` | ✅ 已安装 |
| **slack** | `claude plugin install slack@claude-plugins-official` | ✅ 已安装 (需 OAuth) |

### 推荐方案

| 用途 | 推荐项目 | 理由 |
|------|----------|------|
| **文档获取** | **Context7 官方插件** | Claude Code 官方集成，一键安装 |
| **代码搜索** | Claude Context (Zilliz) | 大厂维护，混合搜索，40% token 节省 |
| **Slack 通知** | **Slack 官方插件** | Claude Code 官方集成，OAuth 认证 |
| **私有文档** | Docs MCP Server | 补充本地/私有文档 |

### 关键发现

1. **Context7 和 Slack 都有 Claude Code 官方插件** - 无需手动配置 MCP
2. **Slack 官方插件使用 Slack 官方 MCP** - `https://mcp.slack.com/sse`，需 OAuth 授权
3. **@modelcontextprotocol/server-slack 已弃用** - 使用官方插件替代
4. **Claude Context (代码搜索) 暂无官方插件** - 需手动配置 MCP

---

## 1. Documentation MCP

### 1.1 Context7 (Upstash)

**官方仓库**: [github.com/upstash/context7](https://github.com/upstash/context7)

#### 核心价值

- 解决 LLM 训练数据过时问题
- 版本感知：Next.js 15、React 19 等最新 API
- 5000+ 库覆盖，持续更新

#### 工具列表

| 工具 | 功能 | 参数 |
|------|------|------|
| `resolve-library-id` | 库名 → Context7 ID | `libraryName`, `query` |
| `query-docs` | 获取文档片段 | `libraryId`, `query`, `tokens` (默认 5000) |

#### 安装方式

**方式一：Claude Code CLI (推荐)**
```bash
claude mcp add context7 -- npx -y @upstash/context7-mcp@latest
```

**方式二：带 API Key (更高配额)**
```bash
claude mcp add context7 -- npx -y @upstash/context7-mcp@latest --api-key YOUR_API_KEY
```

**方式三：远程 MCP**
```json
{
  "mcpServers": {
    "context7": {
      "type": "remote",
      "url": "https://mcp.context7.com/mcp",
      "headers": {
        "CONTEXT7_API_KEY": "your-api-key"
      }
    }
  }
}
```

#### API Key 获取

免费注册: [context7.com/dashboard](https://context7.com/dashboard)

#### 使用示例

```
用户: 如何在 Next.js 15 中使用 Server Actions？use context7

Claude:
1. 调用 resolve-library-id("next.js", "server actions")
   → 返回 "/vercel/next.js/v15.0.0"
2. 调用 query-docs("/vercel/next.js/v15.0.0", "server actions")
   → 返回最新文档和代码示例
```

---

### 1.2 Docs MCP Server (开源替代)

**官方仓库**: [github.com/arabold/docs-mcp-server](https://github.com/arabold/docs-mcp-server)

#### 核心价值

- Context7 的开源替代方案
- 完全本地运行，代码不离开网络
- 支持私有文档、本地文件

#### 与 Context7 对比

| 特性 | Context7 | Docs MCP Server |
|------|----------|-----------------|
| 托管方式 | 云服务 | 自托管 |
| 文档源 | 预索引 5000+ 库 | 按需抓取 |
| 私有文档 | 不支持 | 支持 |
| 本地文件 | 不支持 | 支持 |
| 成本 | 免费tier + 付费 | 完全免费 |
| 隐私 | 查询发送到云端 | 完全本地 |

#### 支持的文档源

- 网站 (HTML)
- GitHub 仓库
- npm 包
- PyPI 包
- 本地文件夹
- ZIP 压缩包
- PDF、Word、Excel、PPT

#### 安装方式

**方式一：NPX 快速启动**
```bash
npx @arabold/docs-mcp-server@latest
```

**方式二：Docker 部署**
```bash
docker run --rm \
  -v docs-mcp-data:/data \
  -v docs-mcp-config:/config \
  -p 6280:6280 \
  ghcr.io/arabold/docs-mcp-server:latest
```

#### MCP 配置

```json
{
  "mcpServers": {
    "docs-mcp-server": {
      "type": "sse",
      "url": "http://localhost:6280/sse"
    }
  }
}
```

#### Web 界面

启动后访问 `http://localhost:6280` 添加和管理文档源。

---

### 1.3 推荐组合策略

```
┌─────────────────────────────────────────────────────────────┐
│  Documentation MCP 双层策略                                  │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  第一层: Context7 (主流库)                                   │
│  ├─ React, Vue, Next.js, Nuxt                              │
│  ├─ Express, Fastify, NestJS                               │
│  ├─ Prisma, Drizzle, TypeORM                               │
│  └─ 5000+ 预索引库                                          │
│                                                             │
│  第二层: Docs MCP Server (补充)                              │
│  ├─ 私有 API 文档                                           │
│  ├─ 内部 Wiki/Confluence                                    │
│  ├─ 本地 Markdown 文档                                      │
│  └─ 最新版本未被 Context7 收录的库                           │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Codebase Search MCP

### 2.1 Claude Context (Zilliz) - 推荐

**官方仓库**: [github.com/zilliztech/claude-context](https://github.com/zilliztech/claude-context)

#### 核心价值

- 语义代码搜索：用自然语言查找代码
- 混合搜索：BM25 关键词 + 向量语义
- 40% token 节省：精准上下文检索
- 持久化索引：跨会话保留

#### 工具列表

| 工具 | 功能 | 参数 |
|------|------|------|
| `search_code` | 语义代码搜索 | `path` (绝对路径), `query`, `limit`, `extensionFilter` |
| `index_codebase` | 索引代码库 | `path`, `force`, `splitter`, `ignorePatterns` |
| `get_indexing_status` | 检查索引进度 | `path` |
| `clear_index` | 清除索引 | `path` |

#### 前置要求

| 要求 | 说明 |
|------|------|
| Node.js | 20.0.0 - 23.x (不兼容 24.0.0) |
| 向量数据库 | Zilliz Cloud (免费tier可用) |
| Embedding | OpenAI API Key 或其他 provider |

#### 安装步骤

**步骤 1: 获取 Zilliz Cloud API Key**

1. 注册 [cloud.zilliz.com](https://cloud.zilliz.com)
2. 创建免费 Cluster
3. 复制 Public Endpoint 和 API Key

**步骤 2: 获取 OpenAI API Key**

1. 访问 [platform.openai.com](https://platform.openai.com)
2. 创建 API Key (sk-xxx 格式)

**步骤 3: 安装 MCP**

```bash
claude mcp add claude-context \
  -e OPENAI_API_KEY=sk-your-openai-api-key \
  -e MILVUS_ADDRESS=your-zilliz-public-endpoint \
  -e MILVUS_TOKEN=your-zilliz-api-key \
  -- npx @zilliz/claude-context-mcp@latest
```

#### 环境变量

| 变量 | 必需 | 说明 |
|------|------|------|
| `OPENAI_API_KEY` | 是 | OpenAI API Key |
| `MILVUS_ADDRESS` | 是 | Zilliz Cloud Public Endpoint |
| `MILVUS_TOKEN` | 是 | Zilliz Cloud API Key |
| `EMBEDDING_MODEL` | 否 | 模型名 (默认 text-embedding-3-small) |

#### 全局配置 (推荐)

创建 `~/.context/.env`:
```bash
OPENAI_API_KEY=sk-your-key
MILVUS_ADDRESS=https://xxx.zillizcloud.com
MILVUS_TOKEN=your-token
EMBEDDING_MODEL=text-embedding-3-large
```

#### 使用流程

```
1. 首次使用项目
   用户: 索引这个代码库
   Claude: 调用 index_codebase("/path/to/project")

2. 检查索引状态
   用户: 索引完成了吗？
   Claude: 调用 get_indexing_status("/path/to/project")
   → 返回 "Indexing complete: 1234 files, 5678 chunks"

3. 语义搜索
   用户: 找到处理用户认证的代码
   Claude: 调用 search_code("/path/to/project", "user authentication handling")
   → 返回相关代码片段和文件路径

4. 后续会话 (索引持久化)
   用户: 找到数据库连接配置
   Claude: 直接调用 search_code (无需重新索引)
```

#### 支持的 Embedding Providers

| Provider | 模型 | 说明 |
|----------|------|------|
| OpenAI | text-embedding-3-small | 默认，性价比高 |
| OpenAI | text-embedding-3-large | 更高精度 |
| VoyageAI | voyage-code-3 | 代码专用模型 |
| Ollama | nomic-embed-text | 本地运行，免费 |

#### 本地 Embedding (Ollama)

如果不想使用 OpenAI API:

```bash
# 1. 安装 Ollama
curl -fsSL https://ollama.com/install.sh | sh

# 2. 拉取 embedding 模型
ollama pull nomic-embed-text

# 3. 配置 Claude Context
claude mcp add claude-context \
  -e OLLAMA_HOST=http://localhost:11434 \
  -e EMBEDDING_MODEL=nomic-embed-text \
  -e MILVUS_ADDRESS=your-zilliz-endpoint \
  -e MILVUS_TOKEN=your-token \
  -- npx @zilliz/claude-context-mcp@latest
```

---

### 2.2 其他备选方案

#### mcp-vector-search (完全本地)

**仓库**: [github.com/bobmatnyc/mcp-vector-search](https://github.com/bobmatnyc/mcp-vector-search)

| 特性 | 说明 |
|------|------|
| 向量数据库 | ChromaDB (本地) |
| Embedding | 本地模型 |
| 隐私 | 完全本地，无需云服务 |
| 语言支持 | 8 种编程语言 |

适用场景：对隐私要求极高，不愿使用云服务。

#### DeepContext

**来源**: Wildcard AI

| 特性 | 说明 |
|------|------|
| 搜索方式 | 混合搜索 (Vector + BM25) |
| 特点 | 符号感知 (Symbol-aware) |
| 语言 | TypeScript, Python |

适用场景：需要符号级别的代码理解。

---

## 3. Slack MCP

### 3.1 现状分析

| 项目 | 状态 | 说明 |
|------|------|------|
| **Claude Code 官方 Slack 插件** | **推荐** | 一键安装，OAuth 认证，Slack 官方 MCP |
| @modelcontextprotocol/server-slack | **已弃用** | npm 包标记为 deprecated |
| korotovsky/slack-mcp-server | 活跃 | 9000+ 用户，功能全，适合高级定制 |

### 3.2 Claude Code 官方 Slack 插件 - 首选

**安装命令**:
```bash
claude plugin install slack@claude-plugins-official
```

**特点**:
- 使用 Slack 官方 MCP 服务器: `https://mcp.slack.com/sse`
- OAuth 认证：首次使用时会引导授权
- 功能: 搜索消息、访问频道、阅读线程
- 无需手动配置 Token

**验证安装**:
```bash
claude mcp list
# 输出: plugin:slack:slack: https://mcp.slack.com/sse (SSE) - ✓ Connected
```

**首次使用**:
1. 运行任何 Slack 相关命令
2. Claude Code 会提示进行 OAuth 授权
3. 在浏览器中登录 Slack 并授权
4. 授权完成后自动连接

---

### 3.3 korotovsky/slack-mcp-server - 备选 (高级定制)

**官方仓库**: [github.com/korotovsky/slack-mcp-server](https://github.com/korotovsky/slack-mcp-server)

#### 核心特性

| 特性 | 说明 |
|------|------|
| 认证模式 | Stealth (无需权限) / OAuth / Bot Token |
| 传输协议 | Stdio, SSE, HTTP |
| 消息功能 | 频道、私信、群组、线程 |
| 智能历史 | 按日期范围或消息数量获取 |
| 安全设计 | 发消息默认禁用，需显式开启 |

#### 工具列表

| 工具 | 功能 | 说明 |
|------|------|------|
| `conversations_history` | 获取频道消息 | 支持游标分页 |
| `conversations_replies` | 获取线程回复 | 支持游标分页 |
| `conversations_add_message` | 发送消息 | 默认禁用 |
| `conversations_search_messages` | 搜索消息 | 支持日期、用户、内容过滤 |
| `channels_list` | 列出频道 | 支持类型筛选和排序 |

#### 资源 (Resources)

| 资源 URI | 格式 | 内容 |
|----------|------|------|
| `slack://<workspace>/channels` | CSV | 频道目录 (ID, 名称, 话题, 成员数) |
| `slack://<workspace>/users` | CSV | 用户目录 (ID, 用户名, 真名) |

#### 认证方式

**方式一：Stealth 模式 (推荐测试)**

从浏览器获取 cookies，无需创建 Slack App:

1. 打开 Slack Web 版
2. 打开开发者工具 → Application → Cookies
3. 复制 `xoxc-xxx` 和 `xoxd-xxx` 值

**方式二：Bot Token (推荐生产)**

1. 创建 Slack App: [api.slack.com/apps](https://api.slack.com/apps)
2. 添加 Bot Token Scopes:
   - `channels:history`
   - `channels:read`
   - `chat:write` (如需发消息)
   - `users:read`
3. 安装到工作区
4. 复制 Bot Token (`xoxb-xxx`)

**方式三：User OAuth Token**

1. 创建 Slack App
2. 添加 User Token Scopes
3. OAuth 授权
4. 复制 User Token (`xoxp-xxx`)

#### 环境变量

| 变量 | 说明 | 示例 |
|------|------|------|
| `SLACK_MCP_XOXC_TOKEN` | Stealth 模式 cookie | `xoxc-xxx` |
| `SLACK_MCP_XOXD_TOKEN` | Stealth 模式 cookie | `xoxd-xxx` |
| `SLACK_MCP_XOXB_TOKEN` | Bot Token | `xoxb-xxx` |
| `SLACK_MCP_XOXP_TOKEN` | User OAuth Token | `xoxp-xxx` |
| `SLACK_MCP_ADD_MESSAGE_TOOL` | 启用发消息 | `true` 或频道白名单 `C123,C456` |
| `SLACK_MCP_USERS_CACHE` | 用户缓存路径 | `/path/to/users.json` |
| `SLACK_MCP_CHANNELS_CACHE` | 频道缓存路径 | `/path/to/channels.json` |
| `SLACK_MCP_PORT` | 服务端口 | `13080` |

#### 安装方式

**方式一：NPX (Stdio)**

```bash
claude mcp add slack-mcp \
  -e SLACK_MCP_XOXB_TOKEN=xoxb-your-bot-token \
  -e SLACK_MCP_ADD_MESSAGE_TOOL=true \
  -- npx -y @anthropic/slack-mcp-server@latest
```

> 注意：包名可能需要确认，建议使用下方 Docker 方式

**方式二：Docker (推荐)**

```bash
docker run -d \
  --name slack-mcp \
  -p 13080:13080 \
  -e SLACK_MCP_XOXB_TOKEN=xoxb-your-bot-token \
  -e SLACK_MCP_ADD_MESSAGE_TOOL=true \
  -e SLACK_MCP_USERS_CACHE=/data/users.json \
  -e SLACK_MCP_CHANNELS_CACHE=/data/channels.json \
  -v slack-mcp-data:/data \
  ghcr.io/korotovsky/slack-mcp-server:latest
```

**MCP 配置 (SSE)**

```json
{
  "mcpServers": {
    "slack": {
      "type": "sse",
      "url": "http://localhost:13080/sse"
    }
  }
}
```

#### 与 Plan Cascade 集成

**通知时机设计**

```python
# 在 orchestrator.py 中集成

class SlackNotifier:
    def __init__(self, mcp_client):
        self.mcp = mcp_client
        self.channel = os.getenv("SLACK_NOTIFY_CHANNEL", "#dev-progress")

    def notify_batch_start(self, batch_num: int, stories: List[str]):
        """批次开始通知"""
        self.mcp.call("slack", "conversations_add_message", {
            "channel": self.channel,
            "text": f":rocket: **Batch {batch_num}** 开始执行\n" +
                    f"Stories: {', '.join(stories)}"
        })

    def notify_story_complete(self, story_id: str, title: str):
        """Story 完成通知"""
        self.mcp.call("slack", "conversations_add_message", {
            "channel": self.channel,
            "text": f":white_check_mark: Story `{story_id}` 完成: {title}"
        })

    def notify_story_failed(self, story_id: str, error: str):
        """Story 失败通知"""
        self.mcp.call("slack", "conversations_add_message", {
            "channel": self.channel,
            "text": f":x: Story `{story_id}` 失败\n```{error}```"
        })

    def notify_batch_complete(self, batch_num: int, total: int, completed: int):
        """批次完成通知"""
        self.mcp.call("slack", "conversations_add_message", {
            "channel": self.channel,
            "text": f":tada: **Batch {batch_num}** 完成 ({completed}/{total})"
        })

    def notify_feature_complete(self, feature_name: str):
        """Feature 完成通知"""
        self.mcp.call("slack", "conversations_add_message", {
            "channel": self.channel,
            "text": f":trophy: **Feature: {feature_name}** 开发完成！"
        })
```

---

## 集成方案

### 完整架构

```
┌─────────────────────────────────────────────────────────────────┐
│  Plan Cascade + MCP 增强架构                                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Context7   │  │ Claude       │  │   Slack      │          │
│  │   (文档)     │  │ Context     │  │   (通知)     │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                 │                   │
│         ▼                 ▼                 ▼                   │
│  ┌─────────────────────────────────────────────────────┐       │
│  │                    MCP Layer                         │       │
│  │  resolve-library-id  search_code    conversations_*  │       │
│  │  query-docs          index_codebase                  │       │
│  └─────────────────────────────────────────────────────┘       │
│                              │                                  │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────┐       │
│  │                   Plan Cascade                       │       │
│  │  ┌─────────────────────────────────────────────┐    │       │
│  │  │  context_filter.py                          │    │       │
│  │  │  ├─ 调用 Context7 获取库文档                 │    │       │
│  │  │  └─ 调用 Claude Context 搜索相关代码        │    │       │
│  │  └─────────────────────────────────────────────┘    │       │
│  │  ┌─────────────────────────────────────────────┐    │       │
│  │  │  orchestrator.py                            │    │       │
│  │  │  ├─ Story 完成 → Slack 通知                 │    │       │
│  │  │  └─ 批次完成 → Slack 汇总                   │    │       │
│  │  └─────────────────────────────────────────────┘    │       │
│  └─────────────────────────────────────────────────────┘       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 集成优先级

| 阶段 | MCP 服务 | 集成点 | 预期效果 |
|------|----------|--------|----------|
| **P0** | Context7 | context_filter.py | 减少 API 幻觉 |
| **P1** | Claude Context | context_filter.py | 语义代码搜索，精准上下文 |
| **P2** | Slack | orchestrator.py | 实时进度通知 |
| **P3** | Docs MCP Server | context_filter.py | 私有文档补充 |

---

## 配置模板

### 完整 .mcp.json

```json
{
  "mcpServers": {
    "context7": {
      "command": "npx",
      "args": ["-y", "@upstash/context7-mcp@latest"],
      "env": {
        "CONTEXT7_API_KEY": "${CONTEXT7_API_KEY}"
      }
    },
    "claude-context": {
      "command": "npx",
      "args": ["@zilliz/claude-context-mcp@latest"],
      "env": {
        "OPENAI_API_KEY": "${OPENAI_API_KEY}",
        "MILVUS_ADDRESS": "${MILVUS_ADDRESS}",
        "MILVUS_TOKEN": "${MILVUS_TOKEN}",
        "EMBEDDING_MODEL": "text-embedding-3-small"
      }
    },
    "slack": {
      "type": "sse",
      "url": "http://localhost:13080/sse"
    },
    "docs": {
      "type": "sse",
      "url": "http://localhost:6280/sse"
    }
  }
}
```

### 环境变量模板 (.env)

```bash
# Context7
CONTEXT7_API_KEY=your-context7-api-key

# Claude Context (Zilliz)
OPENAI_API_KEY=sk-your-openai-key
MILVUS_ADDRESS=https://xxx.api.zillizcloud.com
MILVUS_TOKEN=your-zilliz-token

# Slack
SLACK_MCP_XOXB_TOKEN=xoxb-your-bot-token
SLACK_MCP_ADD_MESSAGE_TOOL=true
SLACK_NOTIFY_CHANNEL=#dev-progress
```

### Docker Compose (本地服务)

```yaml
version: '3.8'

services:
  slack-mcp:
    image: ghcr.io/korotovsky/slack-mcp-server:latest
    ports:
      - "13080:13080"
    environment:
      - SLACK_MCP_XOXB_TOKEN=${SLACK_MCP_XOXB_TOKEN}
      - SLACK_MCP_ADD_MESSAGE_TOOL=true
      - SLACK_MCP_USERS_CACHE=/data/users.json
      - SLACK_MCP_CHANNELS_CACHE=/data/channels.json
    volumes:
      - slack-data:/data

  docs-mcp:
    image: ghcr.io/arabold/docs-mcp-server:latest
    ports:
      - "6280:6280"
    volumes:
      - docs-data:/data
      - docs-config:/config

volumes:
  slack-data:
  docs-data:
  docs-config:
```

---

## 下一步行动

1. **验证 Context7 现有配置** - 确认 Claude Code 中 Context7 插件状态
2. **注册 Zilliz Cloud** - 获取免费 API Key
3. **创建 Slack App** - 获取 Bot Token
4. **部署本地服务** - 使用 Docker Compose
5. **修改 Plan Cascade** - 集成 MCP 调用

---

## 参考链接

- [Context7 GitHub](https://github.com/upstash/context7)
- [Claude Context GitHub](https://github.com/zilliztech/claude-context)
- [Slack MCP Server GitHub](https://github.com/korotovsky/slack-mcp-server)
- [Docs MCP Server GitHub](https://github.com/arabold/docs-mcp-server)
- [MCP 官方规范](https://modelcontextprotocol.io/specification/2025-11-25)
- [MCP 官方服务器列表](https://github.com/modelcontextprotocol/servers)
