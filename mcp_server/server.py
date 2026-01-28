#!/usr/bin/env python3
"""
Plan Cascade MCP Server

Main entry point for the MCP server using FastMCP framework.
Provides tools and resources for Plan Cascade's three-layer architecture:
- Layer 1: Mega Plan (Project Level)
- Layer 2: Hybrid Ralph (Feature Level / PRD)
- Layer 3: Stories (Task Level)
"""

import argparse
import asyncio
import logging
import sys
from pathlib import Path

# Add parent directory to path for importing skills modules
PLUGIN_ROOT = Path(__file__).parent.parent
if str(PLUGIN_ROOT) not in sys.path:
    sys.path.insert(0, str(PLUGIN_ROOT))

try:
    from mcp.server.fastmcp import FastMCP
except ImportError:
    print("Error: MCP package not installed. Please run: pip install 'mcp[cli]'")
    sys.exit(1)

from mcp_server.tools import register_prd_tools, register_mega_tools, register_execution_tools
from mcp_server.resources import register_resources

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger("plan-cascade-mcp")

# Create FastMCP server instance
mcp = FastMCP(
    name="plan-cascade",
    version="3.2.0",
    description="Plan Cascade - Three-layer parallel development framework for AI coding tools"
)


def get_project_root() -> Path:
    """
    Get the project root directory.

    Uses CWD as the project root for MCP server operations.
    This allows the server to work with any project directory.
    """
    return Path.cwd()


# ============================================================
# Server Initialization
# ============================================================

def initialize_server():
    """Initialize all tools and resources on the MCP server."""
    project_root = get_project_root()
    logger.info(f"Initializing Plan Cascade MCP server with project root: {project_root}")

    # Register all tool groups
    register_prd_tools(mcp, project_root)
    register_mega_tools(mcp, project_root)
    register_execution_tools(mcp, project_root)

    # Register resources
    register_resources(mcp, project_root)

    logger.info("Plan Cascade MCP server initialized successfully")


# ============================================================
# MCP Prompts (Templates)
# ============================================================

@mcp.prompt()
def start_feature_development(description: str) -> str:
    """
    Template for starting a new feature development with Hybrid Ralph.

    Args:
        description: Feature description
    """
    return f"""# Start Feature Development

## Task Description
{description}

## Recommended Workflow

1. **Generate PRD**: Use `prd_generate` to create a PRD from the description
2. **Review Stories**: Review the generated stories and adjust if needed
3. **Add Stories**: Use `prd_add_story` to add any missing stories
4. **Validate PRD**: Use `prd_validate` to ensure the PRD is valid
5. **Get Execution Batches**: Use `prd_get_batches` to see the parallel execution plan
6. **Execute Stories**: Work through each batch, updating status as you go

## Available Tools
- `prd_generate` - Generate PRD from description
- `prd_add_story` - Add a new story to PRD
- `prd_validate` - Validate PRD structure
- `prd_get_batches` - Get execution batches
- `prd_update_story_status` - Update story status
- `get_story_context` - Get context for a specific story
- `append_findings` - Record findings during development
- `mark_story_complete` - Mark a story as complete
"""


@mcp.prompt()
def start_mega_project(description: str) -> str:
    """
    Template for starting a mega project with Plan Cascade.

    Args:
        description: Project description
    """
    return f"""# Start Mega Project

## Project Description
{description}

## Recommended Workflow

1. **Generate Mega Plan**: Use `mega_generate` to create a project plan
2. **Add Features**: Use `mega_add_feature` to add features
3. **Validate Plan**: Use `mega_validate` to ensure the plan is valid
4. **Get Feature Batches**: Use `mega_get_batches` to see parallel execution order
5. **Execute Features**: Work through each feature batch
6. **Complete & Merge**: Use `mega_get_merge_plan` when all features are done

## Three-Layer Architecture

```
Layer 1: Mega Plan (Project Level)
  - mega_generate, mega_add_feature, mega_validate
  - mega_get_batches, mega_update_feature_status, mega_get_merge_plan

Layer 2: PRD (Feature Level)
  - prd_generate, prd_add_story, prd_validate
  - prd_get_batches, prd_update_story_status

Layer 3: Execution (Task Level)
  - get_story_context, get_execution_status
  - append_findings, mark_story_complete, get_progress
```
"""


# ============================================================
# Main Entry Point
# ============================================================

def main():
    """Main entry point for the MCP server."""
    parser = argparse.ArgumentParser(
        description="Plan Cascade MCP Server",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Run with stdio transport (default, for IDE integration)
  python -m mcp_server.server

  # Run with SSE transport
  python -m mcp_server.server --transport sse --port 8080

  # Test with MCP Inspector
  npx @anthropic/mcp-inspector python -m mcp_server.server
        """
    )
    parser.add_argument(
        "--transport",
        choices=["stdio", "sse"],
        default="stdio",
        help="Transport protocol (default: stdio)"
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8080,
        help="Port for SSE transport (default: 8080)"
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="Enable debug logging"
    )

    args = parser.parse_args()

    if args.debug:
        logging.getLogger().setLevel(logging.DEBUG)
        logger.setLevel(logging.DEBUG)

    # Initialize server
    initialize_server()

    # Run server with specified transport
    logger.info(f"Starting Plan Cascade MCP server with {args.transport} transport")

    if args.transport == "stdio":
        mcp.run(transport="stdio")
    else:
        mcp.run(transport="sse", port=args.port)


if __name__ == "__main__":
    main()
