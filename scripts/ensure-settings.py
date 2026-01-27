#!/usr/bin/env python3
"""
Intelligent settings.local.json merge script.

This script ensures that required auto-approval patterns are present in
.claude/settings.local.json without overwriting existing user configurations.

Usage:
    python3 scripts/ensure-settings.py
"""

import json
import os
import sys
from pathlib import Path

# Required auto-approval patterns for Hybrid Ralph workflow
REQUIRED_PATTERNS = {
    "bash": [
        "git *",
        "cd *",
        "pwd",
        "ls *",
        "find *",
        "grep *",
        "cat *",
        "head *",
        "tail *",
        "echo *",
        "mkdir *",
        "rm -f .locks/*",
        "test *",
        "[ *",  # test command synonym
        "python3 *",
        "python *",
        "node *",
        "npm *",
        "cp *",
        "mv *",
        "sed *",
        "awk *",
        "wc *",
        "sort *",
        "uniq *",
    ],
    "cmd": [  # Windows PowerShell/CMD
        "git *",
        "cd *",
        "chdir",
        "dir",
        "type *",
        "copy *",
        "move *",
        "mkdir *",
        "del .locks\\*",
        "echo *",
        "python *",
        "node *",
        "npm *",
        "powershell *",
    ]
}

SETTINGS_DESCRIPTION = "Auto-approve safe development commands for Hybrid Ralph workflow. These commands are non-destructive and commonly used during automated PRD execution and worktree management."


def get_settings_path():
    """Get the path to settings.local.json."""
    # Check current directory first
    cwd = Path.cwd()
    settings_path = cwd / ".claude" / "settings.local.json"

    # If not in current directory, check script location
    if not settings_path.exists():
        script_dir = Path(__file__).parent.parent
        settings_path = script_dir / ".claude" / "settings.local.json"

    return settings_path


def load_existing_settings(settings_path):
    """Load existing settings if they exist."""
    if settings_path.exists():
        try:
            with open(settings_path, 'r', encoding='utf-8') as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError) as e:
            print(f"Warning: Could not load existing settings: {e}")
            return {}
    return {}


def merge_patterns(existing_patterns, required_patterns):
    """Merge required patterns with existing patterns, avoiding duplicates."""
    merged = list(existing_patterns) if existing_patterns else []

    for pattern in required_patterns:
        if pattern not in merged:
            merged.append(pattern)

    return merged


def ensure_settings():
    """Ensure required patterns are in settings.local.json."""
    settings_path = get_settings_path()
    print(f"Checking settings file: {settings_path}")

    # Load existing settings
    existing = load_existing_settings(settings_path)

    # Initialize alwaysApprove if not present
    if "alwaysApprove" not in existing:
        existing["alwaysApprove"] = {}

    # Merge patterns for each shell type
    updated = False
    for shell_type, patterns in REQUIRED_PATTERNS.items():
        existing_patterns = existing["alwaysApprove"].get(shell_type, [])
        merged_patterns = merge_patterns(existing_patterns, patterns)

        if len(merged_patterns) > len(existing_patterns):
            existing["alwaysApprove"][shell_type] = merged_patterns
            updated = True
            print(f"  Added {len(merged_patterns) - len(existing_patterns)} new pattern(s) for {shell_type}")
        else:
            print(f"  All required patterns already present for {shell_type}")

    # Add description if not present
    if "description" not in existing:
        existing["description"] = SETTINGS_DESCRIPTION
        updated = True
        print(f"  Added description")
    else:
        print(f"  Description already present")

    # Create directory if it doesn't exist
    settings_path.parent.mkdir(parents=True, exist_ok=True)

    # Write updated settings
    if updated or not settings_path.exists():
        with open(settings_path, 'w', encoding='utf-8') as f:
            json.dump(existing, f, indent=2, ensure_ascii=False)
        print(f"\n✓ Settings written to: {settings_path}")
        return True
    else:
        print(f"\n✓ Settings already up to date")
        return False


def main():
    """Main entry point."""
    print("=" * 60)
    print("Hybrid Ralph - Settings Configuration")
    print("=" * 60)
    print()

    if ensure_settings():
        print("\nSettings have been updated successfully.")
        print("\nThe following command patterns are now auto-approved:")
        print("\nBash:")
        for pattern in REQUIRED_PATTERNS["bash"]:
            print(f"  - {pattern}")
        print("\nPowerShell/CMD:")
        for pattern in REQUIRED_PATTERNS["cmd"]:
            print(f"  - {pattern}")
    else:
        print("\nNo changes needed - settings already configured.")

    return 0


if __name__ == "__main__":
    sys.exit(main())
