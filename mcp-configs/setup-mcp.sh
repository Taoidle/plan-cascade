#!/bin/bash
# Plan Cascade MCP Setup Script
# Usage: ./setup-mcp.sh [tool]
# Tools: cursor, windsurf, cline, continue, zed, claude

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PLAN_CASCADE_PATH="$(dirname "$SCRIPT_DIR")"

echo "Plan Cascade MCP Setup"
echo "======================"
echo "Plan Cascade path: $PLAN_CASCADE_PATH"
echo ""

# Function to replace placeholder in config
replace_placeholder() {
    local file="$1"
    local dest="$2"
    sed "s|{{PLAN_CASCADE_PATH}}|$PLAN_CASCADE_PATH|g" "$file" > "$dest"
    echo "Created: $dest"
}

# Parse tool argument
TOOL="${1:-}"

case "$TOOL" in
    cursor)
        echo "Setting up Cursor..."
        mkdir -p "$PLAN_CASCADE_PATH/.cursor"
        replace_placeholder "$SCRIPT_DIR/cursor-mcp.json" "$PLAN_CASCADE_PATH/.cursor/mcp.json"
        echo "Done! Restart Cursor to apply changes."
        ;;

    windsurf)
        echo "Setting up Windsurf..."
        mkdir -p "$HOME/.codeium/windsurf"
        replace_placeholder "$SCRIPT_DIR/windsurf-mcp.json" "$HOME/.codeium/windsurf/mcp_config.json"
        echo "Done! Restart Windsurf to apply changes."
        ;;

    cline)
        echo "Setting up Cline..."
        echo "Add the following to your VS Code settings.json:"
        echo ""
        cat "$SCRIPT_DIR/cline-settings.json" | sed "s|{{PLAN_CASCADE_PATH}}|$PLAN_CASCADE_PATH|g"
        echo ""
        ;;

    continue)
        echo "Setting up Continue..."
        echo "Add the following to your ~/.continue/config.json:"
        echo ""
        cat "$SCRIPT_DIR/continue-config.json" | sed "s|{{PLAN_CASCADE_PATH}}|$PLAN_CASCADE_PATH|g"
        echo ""
        ;;

    zed)
        echo "Setting up Zed..."
        echo "Add the following to your ~/.config/zed/settings.json:"
        echo ""
        cat "$SCRIPT_DIR/zed-settings.json" | sed "s|{{PLAN_CASCADE_PATH}}|$PLAN_CASCADE_PATH|g"
        echo ""
        ;;

    claude)
        echo "Setting up Claude Code..."
        echo "Run the following command:"
        echo ""
        echo "  claude mcp add plan-cascade -- python -m mcp_server.server"
        echo ""
        echo "Or with explicit path:"
        echo ""
        echo "  cd $PLAN_CASCADE_PATH && claude mcp add plan-cascade -- python -m mcp_server.server"
        echo ""
        ;;

    test)
        echo "Testing MCP server..."
        echo "Running: python -m mcp_server.server --debug"
        echo ""
        cd "$PLAN_CASCADE_PATH"
        python -m mcp_server.server --debug
        ;;

    inspector)
        echo "Running MCP Inspector..."
        cd "$PLAN_CASCADE_PATH"
        npx @anthropic/mcp-inspector python -m mcp_server.server
        ;;

    *)
        echo "Usage: $0 [tool]"
        echo ""
        echo "Available tools:"
        echo "  cursor     - Setup for Cursor IDE"
        echo "  windsurf   - Setup for Windsurf"
        echo "  cline      - Show config for Cline (VS Code)"
        echo "  continue   - Show config for Continue"
        echo "  zed        - Show config for Zed"
        echo "  claude     - Show Claude Code command"
        echo "  test       - Test MCP server locally"
        echo "  inspector  - Run MCP Inspector"
        echo ""
        echo "Example:"
        echo "  $0 cursor"
        ;;
esac
