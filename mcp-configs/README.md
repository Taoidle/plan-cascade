# MCP Configuration Examples

This directory contains MCP (Model Context Protocol) configuration examples for various AI coding tools.

## Supported Tools

| Tool | Config File | Transport |
|------|-------------|-----------|
| [Cursor](#cursor) | `.cursor/mcp.json` | stdio |
| [Claude Code](#claude-code) | CLI command | stdio |
| [Windsurf](#windsurf) | `~/.codeium/windsurf/mcp_config.json` | stdio |
| [Cline](#cline) | VS Code settings | stdio |
| [Continue](#continue) | `~/.continue/config.json` | stdio |
| [Zed](#zed) | `~/.config/zed/settings.json` | stdio |
| [SSE Server](#sse-server) | HTTP endpoint | sse |

## Quick Setup

### 1. Install Dependencies

```bash
pip install 'mcp[cli]'
```

### 2. Copy Configuration

Copy the appropriate config file to your tool's configuration location.

### 3. Update Paths

Replace placeholder paths in the config:
- `{{PLAN_CASCADE_PATH}}` - Path to plan-cascade directory
- `{{PYTHON_PATH}}` - Path to Python executable (optional)

---

## Cursor

Copy `cursor-mcp.json` to `.cursor/mcp.json` in your project root.

```bash
cp mcp-configs/cursor-mcp.json .cursor/mcp.json
```

Or for global configuration:
```bash
# macOS/Linux
cp mcp-configs/cursor-mcp.json ~/.cursor/mcp.json

# Windows
copy mcp-configs\cursor-mcp.json %USERPROFILE%\.cursor\mcp.json
```

---

## Claude Code

Use the CLI to add the MCP server:

```bash
# Add server (run from plan-cascade directory)
claude mcp add plan-cascade -- python -m mcp_server.server

# Or with explicit path
claude mcp add plan-cascade -- python -m mcp_server.server --cwd /path/to/plan-cascade

# Verify
claude mcp list
```

---

## Windsurf

Copy `windsurf-mcp.json` to Windsurf's MCP config location:

```bash
# macOS/Linux
mkdir -p ~/.codeium/windsurf
cp mcp-configs/windsurf-mcp.json ~/.codeium/windsurf/mcp_config.json

# Windows
mkdir %USERPROFILE%\.codeium\windsurf
copy mcp-configs\windsurf-mcp.json %USERPROFILE%\.codeium\windsurf\mcp_config.json
```

---

## Cline

Add to VS Code settings (`settings.json`):

```json
{
  "cline.mcpServers": {
    "plan-cascade": {
      "command": "python",
      "args": ["-m", "mcp_server.server"],
      "cwd": "/path/to/plan-cascade"
    }
  }
}
```

Or copy content from `cline-settings.json`.

---

## Continue

Add MCP server to Continue's config:

```bash
# macOS/Linux
cat mcp-configs/continue-config.json >> ~/.continue/config.json

# Or manually edit ~/.continue/config.json
```

---

## Zed

Add to Zed settings:

```bash
# macOS/Linux
# Edit ~/.config/zed/settings.json and add the mcp section from zed-settings.json
```

---

## SSE Server

For tools that support HTTP/SSE transport:

```bash
# Start SSE server
python -m mcp_server.server --transport sse --port 8080

# Connect using HTTP endpoint
# URL: http://localhost:8080
```

---

## Platform-Specific Notes

### Windows

Use backslashes in paths and ensure Python is in PATH:

```json
{
  "command": "python",
  "args": ["-m", "mcp_server.server"],
  "cwd": "C:\\path\\to\\plan-cascade"
}
```

Or use full Python path:

```json
{
  "command": "C:\\Users\\YourName\\AppData\\Local\\Programs\\Python\\Python310\\python.exe",
  "args": ["-m", "mcp_server.server"],
  "cwd": "C:\\path\\to\\plan-cascade"
}
```

### macOS/Linux

Use forward slashes:

```json
{
  "command": "python3",
  "args": ["-m", "mcp_server.server"],
  "cwd": "/path/to/plan-cascade"
}
```

Or use virtual environment:

```json
{
  "command": "/path/to/plan-cascade/.venv/bin/python",
  "args": ["-m", "mcp_server.server"],
  "cwd": "/path/to/plan-cascade"
}
```

---

## Environment Variables

You can set environment variables in the config:

```json
{
  "command": "python",
  "args": ["-m", "mcp_server.server"],
  "cwd": "/path/to/plan-cascade",
  "env": {
    "PYTHONPATH": "/path/to/plan-cascade",
    "LOG_LEVEL": "DEBUG"
  }
}
```

---

## Testing

Use MCP Inspector to test the server:

```bash
cd /path/to/plan-cascade
npx @anthropic/mcp-inspector python -m mcp_server.server
```

---

## Troubleshooting

### Server Not Found

Ensure Python and mcp are installed:

```bash
pip show mcp
# If not installed:
pip install 'mcp[cli]'
```

### Permission Denied

On macOS/Linux, ensure scripts are executable:

```bash
chmod +x /path/to/plan-cascade/mcp_server/*.py
```

### Path Issues

Use absolute paths if relative paths don't work:

```bash
# Find Python path
which python3  # macOS/Linux
where python   # Windows
```

### Debug Mode

Run with debug logging to see detailed output:

```bash
python -m mcp_server.server --debug
```
