"""
Plan Cascade CLI Module

Contains the command-line interface:
- main: CLI entry point with typer
- Commands: run, config, status
- Simple mode: One-click execution
- Expert mode: Interactive PRD editing
"""

from .main import app, main

__all__ = [
    "app",
    "main",
]
