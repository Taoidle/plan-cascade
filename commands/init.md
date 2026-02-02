---
description: "Initialize Plan Cascade environment. Detects OS, installs uv if needed, and verifies Python execution. Run this once before using other commands."
---

# Plan Cascade Environment Initialization

This command sets up the cross-platform environment for Plan Cascade.

## Step 1: Detect Operating System

```bash
OS_TYPE="unknown"
case "$(uname -s)" in
    Linux*)     OS_TYPE="linux"; echo "Detected: Linux";;
    Darwin*)    OS_TYPE="macos"; echo "Detected: macOS";;
    CYGWIN*|MINGW*|MSYS*) OS_TYPE="windows"; echo "Detected: Windows (via Git Bash/MSYS2)";;
    *)          OS_TYPE="unknown"; echo "Warning: Unknown OS";;
esac
```

## Step 2: Check and Install uv

uv is a fast Python package manager that handles virtual environments automatically.

```bash
if command -v uv &> /dev/null; then
    echo "uv is already installed: $(uv --version)"
else
    echo "Installing uv..."
    case "$OS_TYPE" in
        windows)
            powershell -ExecutionPolicy Bypass -c "irm https://astral.sh/uv/install.ps1 | iex"
            ;;
        *)
            curl -LsSf https://astral.sh/uv/install.sh | sh
            ;;
    esac

    # Add to PATH for current session
    export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

    if command -v uv &> /dev/null; then
        echo "uv installed successfully: $(uv --version)"
    else
        echo "uv installation failed. Please install manually:"
        echo "  https://docs.astral.sh/uv/getting-started/installation/"
        exit 1
    fi
fi
```

## Step 3: Verify Python Execution

```bash
echo "Testing Python execution..."
if uv run python -c "import sys; print(f'Python {sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}')"; then
    echo "Python execution successful"
else
    echo "Python execution failed"
    exit 1
fi
```

## Step 4: Verify Plan Cascade Module

```bash
echo "Testing Plan Cascade module..."
if uv run python -c "from plan_cascade.state.path_resolver import PathResolver; print('Plan Cascade module loaded')"; then
    echo "Plan Cascade module accessible"
else
    echo "Plan Cascade module not found. Installing..."
    uv pip install -e .
fi
```

## Step 5: Display Environment Summary

```bash
echo ""
echo "================================"
echo "Environment Setup Complete"
echo "================================"
echo "OS: $OS_TYPE"
uv --version
uv run python -c "import sys; print(f'Python: {sys.version}')"
echo ""
echo "You can now use Plan Cascade commands:"
echo "  /plan-cascade:auto <task>"
echo "  /plan-cascade:mega-plan <project>"
echo "  /plan-cascade:hybrid-auto <feature>"
```
