# Worktree Init Script for PowerShell
# Usage: .\worktree-init.ps1 [[-BranchName] <string>] [[-TargetBranch] <string>]

param(
    [string]$BranchName = "task-$(Get-Date -Format 'yyyy-MM-dd')",
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

Write-ColorOutput Green "=== Planning with Files - Worktree Init ==="
Write-Output ""

# Default values
$WorktreeDir = ".worktree/$BranchName"
$ConfigFile = ".planning-config.json"

# Step 1: Verify git repository
$null = & git rev-parse --git-dir 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-ColorOutput Red "ERROR: Not a git repository"
    exit 1
}

# Step 2: Detect default branch
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
Write-Output "  Task Branch:    $BranchName"
Write-Output "  Target Branch:  $TargetBranch"
Write-Output "  Worktree Dir:   $WorktreeDir"
Write-Output ""

# Step 3: Check for existing config
if (Test-Path $ConfigFile) {
    Write-ColorOutput Yellow "WARNING: .planning-config.json already exists"
    Write-Output ""
    Write-Output "Existing configuration:"
    Get-Content $ConfigFile
    Write-Output ""
    Write-Output "Do you want to:"
    Write-Output "  1) Continue with existing session"
    Write-Output "  2) Start a new session (will overwrite config)"
    Write-Output ""
    $choice = Read-Host "Choose [1/2]"
    if ($choice -eq "1") {
        Write-Output "Continuing with existing session..."
        exit 0
    }
}

# Step 4: Create task branch
Write-ColorOutput Green "Creating task branch..."

$branchExists = & git show-ref --verify --quiet refs/heads/$BranchName 2>$null
if ($LASTEXITCODE -eq 0) {
    Write-Output "Branch $BranchName already exists. Checking it out..."
    & git checkout $BranchName
} else {
    & git checkout $TargetBranch 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-ColorOutput Red "ERROR: Cannot checkout target branch $TargetBranch"
        exit 1
    }
    & git checkout -b $BranchName
    Write-ColorOutput Green "Created new branch: $BranchName (from $TargetBranch)"
}

# Step 5: Create planning config
$createdAt = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
$configContent = @"
{
  "mode": "worktree",
  "task_branch": "$BranchName",
  "target_branch": "$TargetBranch",
  "worktree_dir": "$WorktreeDir",
  "created_at": "$createdAt",
  "planning_files": [
    "task_plan.md",
    "findings.md",
    "progress.md"
  ]
}
"@
Set-Content -Path $ConfigFile -Value $configContent
Write-ColorOutput Green "Created $ConfigFile"

# Step 6: Create planning files
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$initScript = Join-Path $scriptDir "init-session.sh"

if (Test-Path $initScript) {
    & bash $initScript
} else {
    # Create files manually
    $taskPlanContent = @"
# Task Plan: [Task Description]

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
- [ ] Complete task with worktree-complete command
- **Status:** pending

## Decisions Made
| Decision | Rationale |
|----------|-----------|

## Errors Encountered
| Error | Attempt | Resolution |
|-------|---------|------------|

## Worktree Info
- **Branch:** $BranchName
- **Target:** $TargetBranch
- **Complete with:** \`/planning-with-files:complete\`
"@
    Set-Content -Path "task_plan.md" -Value $taskPlanContent

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
    Set-Content -Path "findings.md" -Value $findingsContent

    $date = Get-Date -Format 'yyyy-MM-dd'
    $progressContent = @"
# Progress Log

## Session: $date

### Current Status
- **Phase:** 1 - Requirements & Discovery
- **Started:** $date
- **Branch:** $BranchName

### Actions Taken
-

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|

### Errors
| Error | Resolution |
|-------|------------|
"@
    Set-Content -Path "progress.md" -Value $progressContent

    Write-ColorOutput Green "Created planning files"
}

# Step 7: Summary
Write-Output ""
Write-ColorOutput Green "=== Worktree Session Created ==="
Write-Output ""
Write-Output "Branch:       $BranchName"
Write-Output "Target:       $TargetBranch"
Write-Output "Config File:  $ConfigFile"
Write-Output ""
Write-Output "Planning Files:"
Write-Output "  - task_plan.md"
Write-Output "  - findings.md"
Write-Output "  - progress.md"
Write-Output ""
Write-ColorOutput Yellow "Next Steps:"
Write-Output "  1. Edit task_plan.md to define your task phases"
Write-Output "  2. Work on your task in this isolated branch"
Write-Output "  3. Use /planning-with-files:complete when done"
Write-Output ""
