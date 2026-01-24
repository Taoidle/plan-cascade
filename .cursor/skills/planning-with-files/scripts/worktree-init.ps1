# Worktree Init Script for PowerShell
# Creates an isolated Git worktree for parallel multi-task development
# Usage: .\worktree-init.ps1 [[-TaskName] <string>] [[-TargetBranch] <string>]

param(
    [string]$TaskName = "task-$(Get-Date -Format 'yyyy-MM-dd-HHmm')",
    [string]$TargetBranch = ""
)

# Color output
function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) {
        Write-Output $args
    }
    $host.UI.RawUI.ForegroundColor = $fc
}

Write-ColorOutput Cyan "=== Planning with Files - Git Worktree Mode ==="
Write-Output ""
Write-ColorOutput Blue "Multi-Task Parallel Development"
Write-Output "Each task gets its own isolated worktree directory."
Write-Output "You can run multiple tasks simultaneously without conflicts."
Write-Output ""

# Default values
$WorktreeDir = ".worktree/$TaskName"
$TaskBranch = $TaskName

# Save current branch
$originalBranch = & git branch --show-current
if ($LASTEXITCODE -ne 0) {
    Write-ColorOutput Red "ERROR: Not a git repository"
    exit 1
}

# Step 1: Detect default branch
if ([string]::IsNullOrEmpty($TargetBranch)) {
    $targetRef = & git symbolic-ref refs/remotes/origin/HEAD 2>$null
    if ($targetRef) {
        $TargetBranch = $targetRef -replace 'refs/remotes/origin/', ''
    } else {
        # Check if main exists
        $null = & git show-ref --verify --quiet refs/heads/main 2>$null
        if ($LASTEXITCODE -eq 0) {
            $TargetBranch = "main"
        } else {
            $TargetBranch = "master"
        }
    }
}

Write-ColorOutput Yellow "Configuration:"
Write-Output "  Task Name:      $TaskName"
Write-Output "  Task Branch:    $TaskBranch"
Write-Output "  Target Branch:  $TargetBranch"
Write-Output "  Worktree Path:  $WorktreeDir"
Write-Output "  Original Branch: $originalBranch"
Write-Output ""

# Step 2: Check if worktree already exists
if (Test-Path $WorktreeDir) {
    Write-ColorOutput Yellow "Worktree already exists: $WorktreeDir"
    Write-Output ""
    Write-Output "This task is already in progress."
    Write-Output ""
    $choice = Read-Host "Open existing worktree? [Y/n]"
    if ($choice -ne "n" -and $choice -ne "N") {
        Write-Output ""
        Write-ColorOutput Green "=== Opening Existing Worktree ==="
        Write-Output ""
        Write-Output "To work on this task, navigate to:"
        Write-ColorOutput Cyan "  cd $WorktreeDir"
        Write-Output ""
        Write-Output "Planning files are already in that directory."
        exit 0
    }
    Write-Output "Cancelled."
    exit 0
}

# Step 3: Check if branch already exists
$branchExists = & git show-ref --verify --quiet refs/heads/$TaskBranch 2>$null
if ($LASTEXITCODE -eq 0) {
    Write-ColorOutput Yellow "Branch $TaskBranch already exists"
    Write-Output ""
    Write-Output "This branch is checked out in another worktree."
    Write-Output "Use that worktree or delete the branch first."
    exit 1
}

# Step 4: Create Git Worktree
Write-ColorOutput Green "Creating Git worktree..."
& git worktree add -b $TaskBranch $WorktreeDir $TargetBranch
if ($LASTEXITCODE -ne 0) {
    Write-ColorOutput Red "ERROR: Failed to create worktree"
    exit 1
}
Write-ColorOutput Green "Created worktree: $WorktreeDir"

# Step 5: Create planning files in the worktree
Write-Output ""
Write-ColorOutput Green "Creating planning files in worktree..."

$ConfigFile = "$WorktreeDir/.planning-config.json"
$rootDir = Get-Location
$createdAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")

# Create config in worktree
$configContent = @"
{
  "mode": "worktree",
  "task_name": "$TaskName",
  "task_branch": "$TaskBranch",
  "target_branch": "$TargetBranch",
  "worktree_dir": "$WorktreeDir",
  "original_branch": "$originalBranch",
  "root_dir": "$rootDir",
  "created_at": "$createdAt",
  "planning_files": [
    "task_plan.md",
    "findings.md",
    "progress.md"
  ]
}
"@
Set-Content -Path $ConfigFile -Value $configContent

# Create task_plan.md
$taskPlanContent = @"
# Task Plan: $TaskName

## Goal
[One sentence describing the end state]

## Current Phase
Phase 1

## Phases

### Phase 1: Requirements & Discovery
- [ ] Understand user intent
- [ ] Identify constraints and requirements
- [ ] Document findings in findings.md
- **Status:** in_progress

### Phase 2: Planning & Structure
- [ ] Define technical approach
- [ ] Create project structure if needed
- [ ] Document decisions with rationale
- **Status:** pending

### Phase 3: Implementation
- [ ] Execute the plan step by step
- [ ] Write code to files before executing
- [ ] Test incrementally
- **Status:** pending

### Phase 4: Testing & Verification
- [ ] Verify all requirements met
- [ ] Document test results in progress.md
- [ ] Fix any issues found
- **Status:** pending

### Phase 5: Delivery
- [ ] Review all output files
- [ ] Ensure deliverables are complete
- [ ] Complete task with: \`/planning-with-files:complete\`
- **Status:** pending

## Decisions Made
| Decision | Rationale |
|----------|-----------|

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|

## Worktree Info
- **Task Name:** $TaskName
- **Branch:** $TaskBranch
- **Target:** $TargetBranch
- **Worktree:** $WorktreeDir
- **Complete with:** \`/planning-with-files:complete\`
"@
Set-Content -Path "$WorktreeDir/task_plan.md" -Value $taskPlanContent

# Create findings.md
$findingsContent = @"
# Findings & Decisions

## Requirements
-

## Research Findings
-

## Technical Decisions
| Decision | Rationale |
|----------|-----------|

## Issues Encountered
| Issue | Resolution |
|-------|------------|

## Resources
-
"@
Set-Content -Path "$WorktreeDir/findings.md" -Value $findingsContent

# Create progress.md
$date = Get-Date -Format 'yyyy-MM-dd'
$progressContent = @"
# Progress Log

## Session: $date

### Current Status
- **Phase:** 1 - Requirements & Discovery
- **Started:** $date
- **Branch:** $TaskBranch
- **Task Name:** $TaskName

### Actions Taken
-

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|

### Errors
| Error | Resolution |
|-------|------------|
"@
Set-Content -Path "$WorktreeDir/progress.md" -Value $progressContent

Write-ColorOutput Green "Planning files created"

# Step 6: List all active worktrees
Write-Output ""
Write-ColorOutput Cyan "=== Active Worktrees ==="
& git worktree list

# Step 7: Final instructions
Write-Output ""
Write-ColorOutput Green "=== Worktree Session Created ==="
Write-Output ""
Write-ColorOutput Yellow "IMPORTANT: Navigate to the worktree to work on this task"
Write-Output ""
Write-ColorOutput Cyan "cd $WorktreeDir"
Write-Output ""
Write-Output "Once in the worktree directory:"
Write-Output "  1. Edit task_plan.md to define your task phases"
Write-Output "  2. Work on your task in this isolated environment"
Write-Output "  3. Use /planning-with-files:complete when done"
Write-Output ""
Write-ColorOutput Blue "Multi-Task Usage:"
Write-Output "You can create multiple worktrees for parallel tasks:"
Write-Output "  /planning-with-files:worktree task-auth-fix"
Write-Output "  /planning-with-files:worktree task-refactor"
Write-Output "  /planning-with-files:worktree task-docs"
Write-Output ""
Write-Output "Each task works in its own directory without conflicts."
Write-Output ""
Write-ColorOutput Blue "To return to the main project:"
Write-Output "  cd $rootDir"
Write-Output ""
