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

from mcp_server.tools import register_prd_tools, register_mega_tools, register_execution_tools, register_worktree_tools, register_design_tools, register_spec_tools, register_dashboard_tools
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
    version="3.3.0",
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
    register_worktree_tools(mcp, project_root)
    register_design_tools(mcp, project_root)
    register_spec_tools(mcp, project_root)
    register_dashboard_tools(mcp, project_root)

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


@mcp.prompt()
def start_worktree_development(task_name: str, target_branch: str, description: str) -> str:
    """
    Template for starting feature development in an isolated Git worktree.

    Args:
        task_name: Name of the task/feature being developed
        target_branch: Git branch name for the worktree
        description: Description of the feature to implement
    """
    return f"""# Start Worktree Development

## Task
**Name**: {task_name}
**Branch**: {target_branch}

## Description
{description}

## Recommended Workflow

1. **Create Worktree**: Use `worktree_create` to create an isolated Git worktree on branch `{target_branch}`
2. **Generate PRD**: Use `prd_generate` to create a PRD from the description inside the worktree
3. **Generate Design Doc**: Use `design_generate` to create the feature-level design document
4. **Review Stories**: Review the generated stories and adjust if needed
5. **Execute Stories**: Work through each story batch in the isolated worktree
6. **Verify**: Run quality gates (tests, lint, type-check) in the worktree
7. **List Worktrees**: Use `worktree_list` to check all active worktrees
8. **Complete**: Use `worktree_complete` to merge the worktree back and clean up

## Available Tools
- `worktree_create` - Create an isolated Git worktree for `{task_name}`
- `worktree_list` - List all active worktrees and their statuses
- `worktree_complete` - Merge worktree changes back and clean up
- `prd_generate` - Generate PRD from the feature description
- `design_generate` - Generate a feature-level design document
- `prd_get_batches` - Get parallel execution batches
- `get_story_context` - Get context for a specific story
- `mark_story_complete` - Mark a story as complete
- `append_findings` - Record findings during development

## Tips
- The worktree provides full isolation so you can develop without affecting the main branch
- Run quality gates inside the worktree before completing
- Use `worktree_list` to check worktree status at any time
"""


@mcp.prompt()
def start_design_review(scope: str) -> str:
    """
    Template for reviewing design documents.

    Args:
        scope: Review scope - either "feature" or "project"
    """
    scope_label = scope.capitalize()
    if scope == "project":
        scope_details = """- Cross-feature component consistency
- Global architecture alignment
- Project-wide ADR compliance
- Feature mapping completeness"""
    else:
        scope_details = """- Story-component mapping coverage
- Feature-specific API design
- Feature ADR compliance (ADR-F### prefix)
- Integration points with other features"""

    return f"""# Start Design Review

## Scope
**Level**: {scope_label}-level design review

## Review Checklist

### 1. Retrieve Design Document
Use `design_get` to load the current {scope}-level design document.

### 2. Structural Review
- Verify all required sections are present
- Check component definitions are complete
- Validate API contracts and interfaces
{scope_details}

### 3. Run Automated Review
Use `design_review` to run the automated design review checks. This will report:
- Missing sections or incomplete fields
- Story/feature mapping coverage percentage
- Consistency issues between design doc and PRD

### 4. Address Issues
For each issue found:
- Update the design document to fix gaps
- Use `design_generate` to regenerate sections if needed
- Re-run `design_review` to verify fixes

### 5. Final Validation
- Confirm all review items are resolved
- Verify coverage percentages meet thresholds
- Document any accepted deviations as ADRs

## Available Tools
- `design_get` - Retrieve the current design document
- `design_review` - Run automated review and get coverage report
- `design_generate` - Generate or regenerate design document sections

## Review Focus Areas ({scope_label})
{scope_details}
"""


@mcp.prompt()
def start_spec_interview(description: str, flow: str = "standard") -> str:
    """
    Template for conducting a spec interview.

    Args:
        description: Description of the feature or project to specify
        flow: Interview flow level - "quick", "standard", or "full" (default: "standard")
    """
    return f"""# Start Spec Interview

## Feature Description
{description}

## Interview Configuration
**Flow**: {flow}

## Recommended Workflow

1. **Start Interview**: Use `spec_start` with the description and flow="{flow}" to begin the interview
2. **Answer Questions**: Review the returned questions and prepare answers
3. **Submit Answers**: Use `spec_submit_answers` to submit answers for the current question batch
4. **Check Progress**: Use `spec_get_status` to see how many questions remain
5. **Continue or Resume**: If interrupted, use `spec_resume` to pick up where you left off
6. **Compile Spec**: On the final `spec_submit_answers` call, set compile=True to generate the spec document
7. **Clean Up**: Use `spec_cleanup` when the interview is complete

## Flow Levels
- **quick**: Fewer questions, faster turnaround (good for small features)
- **standard**: Balanced coverage (default)
- **full**: Comprehensive interview with first-principles questions

## Available Tools
- `spec_start` - Begin a new spec interview session
- `spec_submit_answers` - Submit answers to interview questions
- `spec_get_status` - Check interview progress and completion percentage
- `spec_resume` - Resume an interrupted interview session
- `spec_cleanup` - Clean up interview state and optionally remove output files

## Tips
- Answer questions as thoroughly as possible for better spec quality
- Use the {flow} flow for this interview
- You can submit answers in batches - no need to answer everything at once
- The spec can be compiled into a PRD for immediate execution
"""


@mcp.prompt()
def resume_interrupted_task() -> str:
    """
    Template for resuming a previously interrupted task.

    No required parameters - this prompt guides recovery from any interruption.
    """
    return """# Resume Interrupted Task

## Recovery Workflow

1. **Check Dashboard**: Use `dashboard` to get an overview of all active plans, features, and stories
2. **Get Execution Status**: Use `get_execution_status` to see the current state of any in-progress execution
3. **Recover Session**: Use `session_recover` to restore context from the last session and identify where work stopped
4. **Review Progress**: Check which stories/features are complete, in-progress, or blocked
5. **Resume Execution**: Continue from the last incomplete story or feature batch

## Available Tools
- `dashboard` - Overview of all active plans and their statuses
- `session_recover` - Restore context from a previous session
- `get_execution_status` - Get detailed execution state for the current plan

## What Gets Recovered
- **PRD State**: Story statuses, batch progress, completion percentages
- **Mega Plan State**: Feature statuses, cross-feature dependencies
- **Execution Context**: Last active story, retry counts, quality gate results
- **Findings**: All recorded findings from the interrupted session

## Common Recovery Scenarios

### Interrupted During Story Execution
1. Use `dashboard` to identify the active PRD
2. Use `get_execution_status` to find the last in-progress story
3. Use `session_recover` to restore the execution context
4. Continue from the incomplete story

### Interrupted During Mega Plan
1. Use `dashboard` to see the mega plan status
2. Check which features are complete and which are pending
3. Use `session_recover` to restore cross-feature context
4. Continue with the next pending feature batch

### Lost Session Context
1. Use `dashboard` for a fresh overview
2. Use `session_recover` to regenerate the execution context file
3. Review the recovered context before proceeding

## Tips
- Always start with `dashboard` to understand the current state
- The `session_recover` tool regenerates context files automatically
- Check for any failed quality gates that need re-running
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
