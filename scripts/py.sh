#!/bin/bash
# Cross-platform Python wrapper for Plan Cascade
# Handles OS detection, uv installation, and Python execution

set -e

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)     echo "linux";;
        Darwin*)    echo "macos";;
        CYGWIN*|MINGW*|MSYS*) echo "windows";;
        *)          echo "unknown";;
    esac
}

# Install uv if not present
install_uv() {
    local os_type="$1"
    echo "Installing uv..." >&2

    if [ "$os_type" = "windows" ]; then
        powershell -ExecutionPolicy Bypass -c "irm https://astral.sh/uv/install.ps1 | iex"
    else
        curl -LsSf https://astral.sh/uv/install.sh | sh
    fi

    # Add to PATH for current session
    export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
}

# Main execution
OS_TYPE=$(detect_os)

# Check if uv is available
if ! command -v uv &> /dev/null; then
    install_uv "$OS_TYPE"
fi

# Execute Python with uv
exec uv run python "$@"
