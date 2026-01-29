# Plan Cascade - CLI Guide

**版本**: 4.0.0
**最后更新**: 2026-01-29

本文档详细介绍 Plan Cascade 独立 CLI 工具的使用方法。

---

## 安装

```bash
# 从 PyPI 安装
pip install plan-cascade

# 安装带 LLM 支持
pip install plan-cascade[llm]

# 安装全部依赖
pip install plan-cascade[all]
```

---

## 快速开始

```bash
# 配置向导（首次使用）
plan-cascade config --setup

# 简单模式 - 一键执行
plan-cascade run "实现用户登录功能"

# 专家模式 - 更多控制
plan-cascade run "实现用户登录功能" --expert

# 交互式聊天模式
plan-cascade chat
```

---

## 双模式设计

### 简单模式（默认）

面向新手用户和快速任务，AI 自动判断策略并执行。

```bash
plan-cascade run "添加一个退出按钮"
# → AI 判断：小任务 → 直接执行

plan-cascade run "实现用户登录功能"
# → AI 判断：中等功能 → 生成 PRD → 自动执行

plan-cascade run "构建电商平台：用户、商品、订单"
# → AI 判断：大型项目 → Mega Plan → 多 PRD 级联
```

### 专家模式

面向资深用户，提供精细控制。

```bash
plan-cascade run "实现用户登录" --expert
```

专家模式支持：
- 查看和编辑 PRD
- 选择执行策略
- 指定每个 Story 的 Agent
- 调整依赖关系
- 配置质量门控

---

## 命令参考

### run - 执行任务

```bash
plan-cascade run <description> [options]

Options:
  -e, --expert           专家模式
  -b, --backend <name>   后端选择 (claude-code|claude-api|openai|deepseek|ollama)
  --model <name>         指定模型
  --project <path>       项目路径
```

示例：

```bash
# 简单模式
plan-cascade run "添加搜索功能"

# 专家模式
plan-cascade run "重构用户模块" --expert

# 使用 OpenAI
plan-cascade run "实现评论功能" --backend openai --model gpt-4o
```

### config - 配置管理

```bash
plan-cascade config [options]

Options:
  --show     显示当前配置
  --setup    运行配置向导
```

示例：

```bash
# 查看配置
plan-cascade config --show

# 配置向导
plan-cascade config --setup
```

### chat - 交互式 REPL

```bash
plan-cascade chat [options]

Options:
  -p, --project <path>   项目路径
  -b, --backend <name>   后端选择
```

REPL 特殊命令：

| 命令 | 说明 |
|------|------|
| `/exit`, `/quit` | 退出 |
| `/clear` | 清空上下文 |
| `/status` | 查看会话状态 |
| `/mode [simple\|expert]` | 切换模式 |
| `/history` | 查看对话历史 |
| `/config` | 配置管理 |
| `/help` | 帮助 |

示例：

```bash
plan-cascade chat

> 分析一下项目结构
(AI 分析并响应)

> 基于上面的分析，实现用户登录功能
(意图识别：TASK)
(策略分析)
(执行任务)

> /status
Session: abc123
Mode: simple
Project: /path/to/project

> /mode expert
Mode changed to: expert

> /exit
```

### status - 查看状态

```bash
plan-cascade status

# 输出示例：
任务: 实现用户登录
进度: 3/5
  ✓ 设计数据库 Schema
  ✓ 实现 API 路由
  ✓ OAuth 登录
  ⟳ 手机验证码登录 (执行中)
  ○ 集成测试
```

### version - 版本信息

```bash
plan-cascade version
```

---

## LLM 后端配置

### 支持的后端

| 后端 | 需要 API Key | 说明 |
|------|-------------|------|
| `claude-code` | 否 | 通过 Claude Code CLI（默认） |
| `claude-max` | 否 | 通过 Claude Code 获取 LLM |
| `claude-api` | 是 | 直接调用 Anthropic API |
| `openai` | 是 | OpenAI GPT-4o 等 |
| `deepseek` | 是 | DeepSeek Chat/Coder |
| `ollama` | 否 | 本地模型 |

### 配置示例

```bash
# 使用配置向导
plan-cascade config --setup

# 选择后端:
#   1. Claude Code (推荐，无需 API Key)
#   2. Claude API
#   3. OpenAI
#   4. DeepSeek
#   5. Ollama (本地)
```

### 环境变量

```bash
# Claude API
export ANTHROPIC_API_KEY=sk-ant-...

# OpenAI
export OPENAI_API_KEY=sk-...

# DeepSeek
export DEEPSEEK_API_KEY=sk-...

# Ollama
export OLLAMA_BASE_URL=http://localhost:11434
```

---

## AI 自动策略判断

简单模式下，AI 根据需求自动选择最佳执行策略：

| 输入 | AI 判断 | 执行策略 |
|------|---------|----------|
| "添加一个退出按钮" | 小任务 | 直接执行（无 PRD） |
| "实现用户登录功能" | 中等功能 | Hybrid Auto（自动生成 PRD） |
| "开发博客系统，包含用户、文章、评论" | 大型项目 | Mega Plan（多 PRD 级联） |
| "重构支付模块，不要影响现有功能" | 需要隔离 | Hybrid Worktree |

判断维度：
1. **任务规模**：单一任务 / 多功能 / 完整项目
2. **复杂度**：是否需要分解为多个 Stories
3. **风险程度**：是否需要隔离开发
4. **依赖关系**：是否有跨模块依赖

---

## 专家模式详解

### 工作流

```
1. 输入需求描述
       ↓
2. 生成 PRD
       ↓
3. 交互式菜单
   ├── view    - 查看 PRD
   ├── edit    - 编辑 PRD
   ├── agent   - 指定 Agent
   ├── run     - 执行
   ├── save    - 保存草稿
   └── quit    - 退出
       ↓
4. 执行并监控
```

### 交互示例

```bash
$ plan-cascade run "实现用户登录" --expert

✓ 已生成 PRD (5 个 Stories)

? 请选择操作:
  > view   - 查看 PRD
    edit   - 编辑 PRD
    agent  - 指定 Agent
    run    - 开始执行
    save   - 保存草稿
    quit   - 退出
```

### PRD 编辑

```bash
? 选择要编辑的内容:
  > 修改 Story
    添加 Story
    删除 Story
    调整依赖
    修改优先级
    返回
```

### Agent 分配

```bash
? 为 Story 分配 Agent:
  Story 1: 设计数据库 Schema
  > claude-code (推荐)
    aider
    codex

  Story 2: 实现 OAuth 登录
  > aider
    claude-code
    codex
```

---

## 配置文件

配置文件位于 `~/.plan-cascade/config.yaml`：

```yaml
# 后端配置
backend: claude-code  # claude-code | claude-api | openai | deepseek | ollama
provider: claude      # claude | openai | deepseek | ollama
model: ""            # 留空使用默认

# 执行 Agent
agents:
  - name: claude-code
    enabled: true
    command: claude
    is_default: true
  - name: aider
    enabled: true
    command: aider --model gpt-4o
  - name: codex
    enabled: false
    command: codex

# Agent 选择策略
agent_selection: prefer_default  # smart | prefer_default | manual
default_agent: claude-code

# 质量门控
quality_gates:
  typecheck: true
  test: true
  lint: true
  custom: false
  custom_script: ""
  max_retries: 3

# 执行配置
max_parallel_stories: 3
max_iterations: 50
timeout_seconds: 300

# UI 配置
default_mode: simple  # simple | expert
theme: system        # light | dark | system
```

---

## 故障排除

### API Key 未配置

```
Error: Claude API key is required
```

解决：

```bash
plan-cascade config --setup
# 或设置环境变量
export ANTHROPIC_API_KEY=sk-ant-...
```

### 后端不可用

```
Error: Backend 'ollama' not available
```

解决：确保 Ollama 已启动并运行在正确端口。

### 模型不支持

```
Error: Model 'gpt-5' not found
```

解决：检查模型名称是否正确，使用 `--model` 指定有效模型。

---

## 与 Claude Code Plugin 的区别

| 特性 | CLI | Plugin |
|------|-----|--------|
| 安装方式 | pip install | claude plugins install |
| 使用方式 | 命令行 | /slash 命令 |
| 后端支持 | 多 LLM | Claude Code |
| 工具执行 | 内置 ReAct | Claude Code |
| 离线使用 | 支持（Ollama） | 不支持 |

CLI 适合：
- 需要使用其他 LLM（OpenAI、DeepSeek 等）
- 需要离线使用（Ollama）
- 偏好命令行操作
- 自动化脚本集成

Plugin 适合：
- Claude Code 深度用户
- 需要完整 Claude Code 功能
- 偏好 /slash 命令交互
