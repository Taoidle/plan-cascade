---
description: "Approve the mega-plan and start feature execution. Creates worktrees and generates PRDs for each feature. Usage: /plan-cascade:mega-approve [--auto-prd] [--agent <name>] [--prd-agent <name>] [--impl-agent <name>]"
---

# Approve Mega Plan and Start Execution

Approve the mega-plan and begin executing features in **batch-by-batch** order with **FULL AUTOMATION**.

## Multi-Agent Collaboration

Mega-plan execution supports multiple AI agents at two levels:

1. **PRD Generation**: Agent that generates PRDs for each feature worktree
2. **Story Execution**: Agent that executes stories within each feature

### Supported Agents

| Agent | Type | Best For |
|-------|------|----------|
| `claude-code` | task-tool | General purpose (default, always available) |
| `codex` | cli | PRD generation, bug fixes |
| `aider` | cli | Refactoring, code improvements |
| `amp-code` | cli | Alternative implementations |

### Command Parameters

```
--auto-prd           Fully automatic mode (no pauses)
--agent <name>       Global agent override (all tasks use this agent)
--prd-agent <name>   Agent for PRD generation phase
--impl-agent <name>  Agent for story implementation phase
--no-fallback        Disable automatic fallback to claude-code
```

## Tool Usage Policy (CRITICAL)

**To avoid command confirmation prompts during automatic execution:**

1. **Use Read tool for file reading** - NEVER use `cat`, `head`, `tail` via Bash
   - ✅ `Read("mega-plan.json")`
   - ❌ `Bash("cat mega-plan.json")`

2. **Use Glob tool for file finding** - NEVER use `find`, `ls` via Bash
   - ✅ `Glob(".worktree/*/progress.txt")`
   - ❌ `Bash("ls .worktree/*/progress.txt")`

3. **Use Grep tool for content search** - NEVER use `grep` via Bash
   - ✅ `Grep("[FEATURE_COMPLETE]", path=".worktree/feature-x/progress.txt")`
   - ❌ `Bash("grep '[FEATURE_COMPLETE]' .worktree/feature-x/progress.txt")`

4. **Only use Bash for actual system commands:**
   - Git operations: `git checkout`, `git merge`, `git worktree add`
   - Directory creation: `mkdir -p`
   - File writing (when Write tool cannot be used): `echo "..." >> file`

5. **For monitoring loops:** Use Read tool to poll file contents, not Bash cat/grep

**CRITICAL**: With `--auto-prd`, this command runs the ENTIRE mega-plan to completion automatically:
1. Creates worktrees for current batch
2. Generates PRDs for each feature (via Task agents)
3. Executes all stories in each feature (via Task agents)
4. Monitors for completion
5. Merges completed batch to target branch
6. Automatically starts next batch
7. Repeats until ALL batches complete

**WITHOUT `--auto-prd`**: Pauses after PRD generation for manual review.

## Step 1: Verify Mega Plan Exists

```bash
if [ ! -f "mega-plan.json" ]; then
    echo "No mega-plan.json found."
    echo "Use /plan-cascade:mega-plan <description> to create one first."
    exit 1
fi
```

## Step 2: Parse Arguments and State

Parse all command arguments:

```bash
# Mode flags
AUTO_PRD=false
NO_FALLBACK=false

# Agent parameters
GLOBAL_AGENT=""
PRD_AGENT=""
IMPL_AGENT=""

# Parse arguments
for arg in $ARGUMENTS; do
    case "$arg" in
        --auto-prd) AUTO_PRD=true ;;
        --no-fallback) NO_FALLBACK=true ;;
        --agent=*) GLOBAL_AGENT="${arg#*=}" ;;
        --prd-agent=*) PRD_AGENT="${arg#*=}" ;;
        --impl-agent=*) IMPL_AGENT="${arg#*=}" ;;
    esac
done

if [ "$AUTO_PRD" = true ]; then
    echo "============================================"
    echo "FULLY AUTOMATIC MODE ENABLED"
    echo "============================================"
    echo "Will execute ALL batches without stopping."
    echo "Only pauses on errors."
    echo "============================================"
fi

# Display agent configuration
echo ""
echo "Agent Configuration:"
echo "  Global Override: ${GLOBAL_AGENT:-"none (use defaults)"}"
echo "  PRD Generation: ${PRD_AGENT:-"claude-code"}"
echo "  Implementation: ${IMPL_AGENT:-"per-story resolution"}"
echo "  Fallback: ${NO_FALLBACK:+"disabled"}"
echo ""
```

### 2.1: Load Agent Configuration

```
If agents.json exists at project root:
    Load agent configuration:
    - agents: Map of agent definitions
    - phase_defaults: PRD generation and implementation defaults
    - story_type_defaults: Agent selection by story type
Else:
    Use defaults: claude-code for all tasks
```

Read current state from `.mega-status.json`:
- `current_batch`: Which batch is currently executing (0 = not started)
- `completed_batches`: List of completed batch numbers
- `features`: Status of each feature

## Step 2.5: Check for Design Document (Optional)

Check if a global `design_doc.json` exists at project root:

```
If design_doc.json exists:
    Read and display design document summary:

    ────────────────────────────────────────────────────────────────
    📐 GLOBAL DESIGN DOCUMENT DETECTED
    ────────────────────────────────────────────────────────────────
    Title: <overview.title>

    Components: N defined
      • <component1> - <description>
      • <component2> - <description>

    Architectural Patterns: M patterns
      • <pattern1>
      • <pattern2>

    Key Decisions: P ADRs
      • ADR-001: <title>
      • ADR-002: <title>

    This design document will be:
    ✓ Copied to each feature worktree
    ✓ Used to guide PRD generation
    ✓ Injected into story execution context
    ────────────────────────────────────────────────────────────────

    HAS_DESIGN_DOC=true
Else:
    Note: No global design document found.
          Consider creating one with /plan-cascade:design-generate
          for better architectural guidance across features.

    HAS_DESIGN_DOC=false
```

## Step 3: Calculate Batches and Determine State

Calculate all batches from mega-plan.json based on dependencies:
- **Batch 1**: Features with no dependencies
- **Batch 2**: Features depending only on Batch 1 features
- **Batch N**: Features depending only on Batch 1..N-1 features

Determine current state:

### Case A: No batch started yet (current_batch = 0 or missing)
→ Start Batch 1

### Case B: Current batch is in progress
→ Check if all features in current batch are complete
→ If not complete AND --auto-prd: Continue monitoring (don't exit)
→ If not complete AND no --auto-prd: Show status and exit
→ If complete: Merge current batch, then start next batch

### Case C: All batches complete
→ Run final cleanup and inform user

## Step 4: Main Execution Loop (AUTOMATIC)

**CRITICAL**: This is the main automation loop. With `--auto-prd`, this loop runs until ALL batches are complete.

```
TOTAL_BATCHES = <calculated from mega-plan.json>
CURRENT_BATCH = <from .mega-status.json or 1 if not started>

while CURRENT_BATCH <= TOTAL_BATCHES:

    # 4.1: Check if current batch features exist (worktrees created)
    if worktrees_not_created:
        create_worktrees_for_batch(CURRENT_BATCH)
        generate_prds_for_batch(CURRENT_BATCH)
        execute_stories_for_batch(CURRENT_BATCH)

    # 4.2: Monitor current batch until complete
    monitor_batch_until_complete(CURRENT_BATCH)

    # 4.3: Merge completed batch
    merge_batch_to_target(CURRENT_BATCH)

    # 4.4: Cleanup worktrees
    cleanup_batch_worktrees(CURRENT_BATCH)

    # 4.5: Move to next batch
    CURRENT_BATCH += 1

# All batches complete!
show_completion_status()
```

## Step 5: Create Worktrees for Batch

For each feature in the current batch:

### 5.1: Checkout Updated Target Branch

```bash
TARGET_BRANCH=$(jq -r '.target_branch' mega-plan.json)
git checkout "$TARGET_BRANCH"
git pull origin "$TARGET_BRANCH" 2>/dev/null || true
```

### 5.2: Create Worktree

```bash
FEATURE_NAME="<feature-name>"
BRANCH_NAME="mega-$FEATURE_NAME"
WORKTREE_PATH=".worktree/$FEATURE_NAME"

# Create worktree from current HEAD (includes all previously merged batches)
git worktree add -b "$BRANCH_NAME" "$WORKTREE_PATH"
```

### 5.3: Initialize Worktree Files

Create in each worktree:
- `.planning-config.json` with feature metadata
- `findings.md` initialized with feature info
- `progress.txt` for tracking
- Copy `mega-findings.md` from project root
- Copy `design_doc.json` from project root (if exists)

## Step 6: Generate PRDs for Batch (Task Agents)

**CRITICAL**: Launch Task agents IN PARALLEL for ALL features in the batch.

For EACH feature in the current batch, launch a Task agent with `run_in_background: true`:

### 6.1: PRD Generation Agent Prompt

```
You are generating a PRD for feature: {feature_id} - {feature_title}

Feature Description:
{feature_description}

Working Directory: {worktree_path}

Your task:
1. Change to the worktree directory: cd {worktree_path}
2. Read mega-findings.md for project context
3. **If design_doc.json exists, read it for architectural guidance:**
   - Identify relevant components for this feature
   - Note applicable architectural patterns
   - Reference relevant ADRs (architectural decisions)
   - Map stories to components/decisions in the PRD
4. Explore relevant code in the codebase
5. Generate a comprehensive prd.json with:
   - Clear goal matching the feature description
   - 3-7 user stories with proper dependencies
   - Each story has: id, title, description, priority, dependencies, acceptance_criteria, status="pending"
6. **If design_doc.json exists, also generate/update story_mappings** linking each story to relevant components, decisions, and interfaces
7. Save prd.json to {worktree_path}/prd.json
8. Update progress.txt: echo "[PRD_COMPLETE] {feature_id}" >> {worktree_path}/progress.txt

PRD JSON format:
{
  "metadata": {
    "created_at": "ISO-8601",
    "version": "1.0.0",
    "description": "{feature_description}",
    "mega_feature_id": "{feature_id}"
  },
  "goal": "Feature goal",
  "objectives": ["obj1", "obj2"],
  "stories": [
    {
      "id": "story-001",
      "title": "Story title",
      "description": "Detailed description",
      "priority": "high|medium|low",
      "dependencies": [],
      "status": "pending",
      "acceptance_criteria": ["criterion1"],
      "context_estimate": "small|medium|large"
    }
  ]
}

Work methodically. When done, the [PRD_COMPLETE] marker in progress.txt signals completion.
```

### 6.2: Resolve PRD Generation Agent

Determine which agent to use for PRD generation:

```
# Priority chain for PRD agent
If GLOBAL_AGENT specified:
    prd_agent = GLOBAL_AGENT
Elif PRD_AGENT specified:
    prd_agent = PRD_AGENT
Elif phase_defaults.planning.default_agent in agents.json:
    prd_agent = phase_defaults.planning.default_agent
Else:
    prd_agent = "claude-code"

# Verify agent availability
If prd_agent != "claude-code":
    If agents[prd_agent].type == "cli":
        If not is_command_available(agents[prd_agent].command):
            If NO_FALLBACK:
                ERROR: Agent {prd_agent} not available
            Else:
                echo "⚠️ {prd_agent} not available, falling back to claude-code"
                prd_agent = "claude-code"

echo "PRD Generation Agent: {prd_agent}"
```

### 6.3: Launch PRD Agents in Parallel

Launch ALL PRD generation agents simultaneously:

```
For each feature in batch:
    If prd_agent == "claude-code":
        # Use Task tool (built-in)
        task_id = Task(
            prompt=<PRD generation prompt above>,
            subagent_type="general-purpose",
            run_in_background=true,
            description="Generate PRD for {feature_id}"
        )
    Else:
        # Use CLI agent
        agent_config = agents[prd_agent]
        command = build_cli_command(agent_config, prompt)
        task_id = Bash(
            command=command,
            run_in_background=true,
            timeout=agent_config.timeout
        )

    Store task_id in prd_tasks[feature_id]
    echo "[PRD_AGENT] {feature_id} -> {prd_agent}" >> mega-findings.md
```

### 6.3: Wait for All PRD Agents to Complete

Use TaskOutput to wait for each agent:

```
For each feature_id, task_id in prd_tasks:
    TaskOutput(task_id=task_id, block=true, timeout=600000)
```

OR monitor progress.txt files:

```
while not all_prds_complete:
    for each feature in batch:
        check if {worktree_path}/progress.txt contains "[PRD_COMPLETE] {feature_id}"
    sleep 10 seconds
```

### 6.4: Validate PRDs

After all PRD agents complete:
- Read each prd.json
- Validate structure (has stories, valid dependencies)
- If validation fails, show error and pause

## Step 7: Execute Stories for Batch (Task Agents)

**CRITICAL**: After PRDs are generated, execute stories for ALL features in the batch.

### 7.1: For Each Feature, Launch Story Execution

For EACH feature in the batch, launch a Task agent to execute its stories:

```
You are executing all stories for feature: {feature_id} - {feature_title}

Working Directory: {worktree_path}

EXECUTION RULES:
1. Read prd.json from {worktree_path}/prd.json
2. **If design_doc.json exists, read it for architectural context:**
   - Check story_mappings to find relevant components for each story
   - Follow the architectural patterns defined in the document
   - Adhere to the architectural decisions (ADRs)
   - Reference the relevant APIs and data models
3. Calculate story batches based on dependencies
4. Execute stories in batch order (parallel within batch, sequential across batches)
5. For each story:
   a. **Get design context for this story from design_doc.json (if exists)**
   b. Implement according to acceptance criteria
   c. **Follow architectural patterns and decisions from design context**
   d. Test your implementation
   e. Mark complete: Update story status to "complete" in prd.json
   f. Log to progress.txt: echo "[STORY_COMPLETE] {story_id}" >> progress.txt
6. When ALL stories are complete:
   echo "[FEATURE_COMPLETE] {feature_id}" >> progress.txt

IMPORTANT:
- Execute bash/powershell commands directly
- Do NOT wait for user confirmation between stories
- Update findings.md with important discoveries
- If a story fails, mark it [STORY_FAILED] and continue to next independent story
- Only stop on blocking errors

Story execution loop:
  STORY_BATCH = 1
  while stories_remaining:
      for each story in current_batch (no pending dependencies):
          implement_story()
          test_story()
          mark_complete_in_prd()
          log_to_progress()
      STORY_BATCH += 1

When completely done, [FEATURE_COMPLETE] marker signals this feature is ready for merge.
```

### 7.2: Resolve Story Execution Agent

For mega-plan, the feature execution agent handles all stories within a feature.
Agent resolution follows the same priority chain:

```
# Priority chain for implementation agent
If GLOBAL_AGENT specified:
    impl_agent = GLOBAL_AGENT
Elif IMPL_AGENT specified:
    impl_agent = IMPL_AGENT
Elif phase_defaults.implementation.default_agent in agents.json:
    impl_agent = phase_defaults.implementation.default_agent
Else:
    impl_agent = "claude-code"

# Verify agent availability
If impl_agent != "claude-code":
    If agents[impl_agent].type == "cli":
        If not is_command_available(agents[impl_agent].command):
            If NO_FALLBACK:
                ERROR: Agent {impl_agent} not available
            Else:
                echo "⚠️ {impl_agent} not available, falling back to claude-code"
                impl_agent = "claude-code"

echo "Story Execution Agent: {impl_agent}"
```

### 7.3: Launch Feature Execution Agents in Parallel

```
For each feature in batch:
    If impl_agent == "claude-code":
        # Use Task tool (built-in)
        task_id = Task(
            prompt=<Story execution prompt above>,
            subagent_type="general-purpose",
            run_in_background=true,
            description="Execute stories for {feature_id}"
        )
    Else:
        # Use CLI agent
        agent_config = agents[impl_agent]
        command = build_cli_command(agent_config, prompt)
        task_id = Bash(
            command=command,
            run_in_background=true,
            timeout=agent_config.timeout
        )

    Store task_id in execution_tasks[feature_id]
    echo "[IMPL_AGENT] {feature_id} -> {impl_agent}" >> mega-findings.md
```

**Note on Story-Level Agent Selection**:
Within each feature, the execution agent may optionally use different agents per story based on:
- `story.agent` field in prd.json
- Story type inference (bugfix, refactor, etc.)
- Story type defaults from agents.json

This is handled within the feature execution prompt if the agent supports it.

## Step 8: Wait for Batch Completion (Using TaskOutput)

**CRITICAL**: Use TaskOutput to wait for agents instead of polling. This avoids Bash confirmation prompts.

### 8.1: Wait for All Feature Agents

For each feature agent launched in Step 7, wait using TaskOutput:

```
For each feature_id, task_id in execution_tasks:
    echo "Waiting for {feature_id}..."

    result = TaskOutput(
        task_id=task_id,
        block=true,
        timeout=1800000  # 30 minutes per feature
    )

    echo "✓ {feature_id} agent completed"
```

**IMPORTANT**:
- Call TaskOutput for ALL feature agents (can be parallel or sequential)
- TaskOutput blocks until the agent finishes - NO polling needed
- NO sleep commands - NO Bash confirmation prompts

### 8.2: Verify Completion After Agents Finish

After all TaskOutput calls return, verify the results by reading progress files:

```
For each feature in current_batch:
    # Use Read tool (not Bash) to check progress
    progress_content = Read("{worktree_path}/progress.txt")

    # Parse in your response (not with grep)
    if "[FEATURE_COMPLETE] {feature_id}" in progress_content:
        feature_status = "complete"
    elif "[FEATURE_FAILED] {feature_id}" in progress_content:
        feature_status = "failed"
    else:
        feature_status = "incomplete"

    # Count markers by parsing the content yourself
    story_complete_count = count occurrences of "[STORY_COMPLETE]"
    story_failed_count = count occurrences of "[STORY_FAILED]"
```

### 8.3: Handle Results

```
if all features complete:
    echo "✓ BATCH {N} COMPLETE"
    proceed to Step 9 (merge)

elif some features failed:
    echo "⚠️ BATCH COMPLETE WITH ERRORS"
    list failed features
    if AUTO_PRD:
        continue (skip failed features during merge)
    else:
        pause for manual review

elif some features incomplete (agent finished but no FEATURE_COMPLETE marker):
    echo "⚠️ AGENT FINISHED BUT FEATURE INCOMPLETE"
    # This means the agent hit an issue - check progress.txt for details
    Use Read tool to show last entries from progress.txt
```

### Why TaskOutput Instead of Polling?

| Method | Confirmation Prompts | Efficiency |
|--------|---------------------|------------|
| `sleep + Bash cat/grep` | YES - every iteration | Poor (wastes cycles) |
| `TaskOutput(block=true)` | NO | Good (proper wait) |

TaskOutput is the native way to wait for background agents in Claude Code.

## Step 9: Merge Completed Batch

When all features in current batch are complete:

### 9.1: Display Merge Start

```
============================================================
BATCH {N} COMPLETED - MERGING TO TARGET BRANCH
============================================================

Merging completed features in dependency order...
```

### 9.2: Checkout Target Branch

```bash
TARGET_BRANCH=$(jq -r '.target_branch' mega-plan.json)
git checkout "$TARGET_BRANCH"
git pull origin "$TARGET_BRANCH" 2>/dev/null || true
```

### 9.3: Merge Each Feature

For each successfully completed feature in the batch:

```bash
FEATURE_NAME="<name>"
WORKTREE_PATH=".worktree/$FEATURE_NAME"
BRANCH_NAME="mega-$FEATURE_NAME"

# Commit any uncommitted changes in worktree (code only, exclude planning files)
cd "$WORKTREE_PATH"
git add -A
git reset HEAD -- prd.json findings.md progress.txt .planning-config.json .agent-status.json mega-findings.md 2>/dev/null || true
git commit -m "feat: complete $FEATURE_NAME" || true
cd -

# Merge to target branch
git merge "$BRANCH_NAME" --no-ff -m "Merge feature: <title>

Mega-plan feature: <feature-id>
Batch: <batch-number>

Co-Authored-By: Claude <noreply@anthropic.com>"

echo "[OK] Merged {feature_id}: {title}"
```

### 9.4: Cleanup Worktrees

```bash
git worktree remove ".worktree/$FEATURE_NAME" --force
git branch -d "mega-$FEATURE_NAME"
```

### 9.5: Update Status

Update `.mega-status.json`:
- Mark features as "merged"
- Add batch to `completed_batches`
- Increment `current_batch`

## Step 10: Continue to Next Batch (AUTOMATIC)

**CRITICAL**: With `--auto-prd`, automatically continue to the next batch.

```
if CURRENT_BATCH < TOTAL_BATCHES:
    echo ""
    echo "============================================"
    echo "AUTO-CONTINUING TO BATCH {CURRENT_BATCH + 1}"
    echo "============================================"

    # Go back to Step 5 (create worktrees for next batch)
    CURRENT_BATCH += 1
    continue main loop
```

## Step 11: All Batches Complete

When all batches are done:

```
============================================================
ALL BATCHES COMPLETE - MEGA PLAN FINISHED
============================================================

Total batches completed: {TOTAL_BATCHES}
Total features merged: {count}

Summary:
  Batch 1: {features} - MERGED
  Batch 2: {features} - MERGED
  ...

All code has been merged to {target_branch}.

Final cleanup (removes planning files):
  /plan-cascade:mega-complete

============================================================
```

## Error Handling

### Merge Conflict

```
============================================================
MERGE CONFLICT
============================================================

Conflict while merging {feature_id}: {title}

Conflicting files:
  - {file1}
  - {file2}

To resolve:
  1. Resolve conflicts in the listed files
  2. git add <resolved-files>
  3. git commit
  4. Re-run /plan-cascade:mega-approve --auto-prd

Or abort: git merge --abort
============================================================
```

Pause execution on merge conflicts (even with --auto-prd).

### Feature Execution Failed

```
============================================================
FEATURE EXECUTION FAILED
============================================================

Feature {feature_id}: {title} failed during execution.

Error details in:
  - {worktree_path}/progress.txt
  - {worktree_path}/findings.md

Failed stories:
  - {story_id}: {reason}

Options:
  1. Fix the issue in {worktree_path}
  2. Re-run /plan-cascade:mega-approve --auto-prd to retry
  3. Skip feature: Mark as failed in .mega-status.json

============================================================
```

### PRD Generation Failed

```
============================================================
PRD GENERATION FAILED
============================================================

Could not generate PRD for {feature_id}: {title}

Worktree: {worktree_path}

To fix:
  1. Manually create prd.json in {worktree_path}
  2. Or edit the feature description in mega-plan.json
  3. Re-run /plan-cascade:mega-approve --auto-prd

============================================================
```

## Execution Flow Summary (AUTOMATIC MODE)

```
/plan-cascade:mega-approve --auto-prd
    │
    ├─→ Read mega-plan.json, calculate batches
    │
    ├─→ BATCH 1 ─────────────────────────────────────────────
    │   ├─→ Create worktrees for Batch 1 features
    │   ├─→ Launch PRD generation agents (parallel)
    │   ├─→ Wait for all PRDs complete
    │   ├─→ Launch story execution agents (parallel)
    │   ├─→ Monitor until all features complete
    │   ├─→ Merge Batch 1 to target_branch
    │   └─→ Cleanup Batch 1 worktrees
    │
    ├─→ BATCH 2 ─────────────────────────────────────────────
    │   ├─→ Create worktrees (from UPDATED target_branch)
    │   ├─→ Launch PRD generation agents (parallel)
    │   ├─→ Wait for all PRDs complete
    │   ├─→ Launch story execution agents (parallel)
    │   ├─→ Monitor until all features complete
    │   ├─→ Merge Batch 2 to target_branch
    │   └─→ Cleanup Batch 2 worktrees
    │
    ├─→ ... continue for all batches ...
    │
    └─→ ALL COMPLETE
        └─→ Show final status, suggest /plan-cascade:mega-complete
```

## Important Notes

### Parallelism Strategy

- **Within a batch**: All features execute in PARALLEL (independent worktrees)
- **Across batches**: SEQUENTIAL (Batch N+1 depends on Batch N code)
- **Within a feature**: Stories execute in dependency order (parallel where possible)

### Progress Markers

Agents use these markers in progress.txt:
- `[PRD_COMPLETE] {feature_id}` - PRD generation done
- `[STORY_COMPLETE] {story_id}` - Individual story done
- `[STORY_FAILED] {story_id}` - Story failed
- `[FEATURE_COMPLETE] {feature_id}` - All stories done, ready for merge
- `[FEATURE_FAILED] {feature_id}` - Feature cannot complete

### Timeout Behavior

- **PRD generation**: 10 minute timeout per feature
- **Story execution**: NO TIMEOUT - stories may take varying time
- **Monitoring loop**: NO TIMEOUT - keeps polling until complete

### Recovery

If interrupted:
1. `.mega-status.json` tracks current state
2. **Recommended**: Use `/plan-cascade:mega-resume --auto-prd` to intelligently resume
   - Auto-detects actual state from files
   - Skips already-completed work
   - Compatible with old and new executions
3. Or re-run `/plan-cascade:mega-approve --auto-prd` to continue from batch level
