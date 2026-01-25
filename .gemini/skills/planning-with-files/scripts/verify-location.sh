#!/bin/bash
# Verify location for worktree mode
# Checks if the current directory is correct for worktree mode operations
# Exit codes:
#   0 - Correct location (in worktree directory)
#   1 - Wrong location (in root but should be in worktree)
#   2 - Not in worktree mode (no .planning-config.json found)

set -e

# Check if .planning-config.json exists in current directory
if [ ! -f ".planning-config.json" ]; then
    # Not in worktree mode, no verification needed
    exit 2
fi

# Read the config to get expected locations
MODE=$(jq -r '.mode' .planning-config.json 2>/dev/null || echo "")
ROOT_DIR=$(jq -r '.root_dir' .planning-config.json 2>/dev/null || echo "")
WORKTREE_DIR=$(jq -r '.worktree_dir' .planning-config.json 2>/dev/null || echo "")

# Only verify if mode is "worktree"
if [ "$MODE" != "worktree" ]; then
    exit 2
fi

# Get current directory
CURRENT_DIR=$(pwd)

# Check if we're in the root directory when we should be in worktree
# This happens if the worktree path is a subdirectory of root
if [ -n "$ROOT_DIR" ] && [ "$CURRENT_DIR" = "$ROOT_DIR" ]; then
    if [ -n "$WORKTREE_DIR" ]; then
        echo "ERROR: You are in the wrong directory!"
        echo "Worktree mode requires working in the worktree directory."
        echo ""
        echo "Current location: $CURRENT_DIR"
        echo "Expected location: $WORKTREE_DIR"
        echo ""
        echo "Please navigate to the worktree:"
        echo "  cd $WORKTREE_DIR"
        exit 1
    fi
fi

# Check if current directory matches the expected worktree directory
# Convert to absolute paths for comparison
if [ -n "$WORKTREE_DIR" ]; then
    # If WORKTREE_DIR is relative, make it absolute relative to root
    if [[ ! "$WORKTREE_DIR" = /* ]]; then
        if [ -n "$ROOT_DIR" ]; then
            EXPECTED_DIR="$ROOT_DIR/$WORKTREE_DIR"
        else
            EXPECTED_DIR="$WORKTREE_DIR"
        fi
    else
        EXPECTED_DIR="$WORKTREE_DIR"
    fi

    # Normalize paths (handle .. and .)
    CURRENT_DIR_NORMALIZED=$(cd "$CURRENT_DIR" && pwd)
    EXPECTED_DIR_NORMALIZED=$(cd "$EXPECTED_DIR" 2>/dev/null && pwd || echo "$EXPECTED_DIR")

    if [ "$CURRENT_DIR_NORMALIZED" != "$EXPECTED_DIR_NORMALIZED" ]; then
        echo "ERROR: You are in the wrong directory!"
        echo "Worktree mode requires working in the worktree directory."
        echo ""
        echo "Current location: $CURRENT_DIR_NORMALIZED"
        echo "Expected location: $EXPECTED_DIR_NORMALIZED"
        echo ""
        echo "Please navigate to the worktree:"
        echo "  cd $WORKTREE_DIR"
        exit 1
    fi
fi

# Location is correct
exit 0
