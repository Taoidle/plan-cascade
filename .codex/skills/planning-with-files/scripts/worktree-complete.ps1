# Worktree Complete Script for PowerShell
# Merges the worktree branch and cleans up the worktree directory
# Usage: .\worktree-complete.ps1 [[-TargetBranch] <string>]
# Run this FROM INSIDE the worktree directory

param(
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

Write-ColorOutput Cyan "=== Planning with Files - Complete Worktree Task ==="
Write-Output ""

# Step 1: Verify we're in a worktree
$ConfigFile = ".planning-config.json"

if (-not (Test-Path $ConfigFile)) {
    Write-ColorOutput Red "ERROR: .planning-config.json not found"
    Write-Output ""
    Write-Output "Are you in a worktree directory?"
    Write-Output "This command must be run from inside the worktree."
    exit 1
}

# Step 2: Read configuration
$config = Get-Content $ConfigFile | ConvertFrom-Json

$mode = $config.mode
$taskName = $config.task_name
$taskBranch = $config.task_branch
$targetBranchConfig = $config.target_branch
$worktreeDir = $config.worktree_dir
$rootDir = $config.root_dir
$originalBranch = $config.original_branch

if ($mode -ne "worktree") {
    Write-ColorOutput Red "ERROR: Not in worktree mode (current mode: $mode)"
    exit 1
}

# Use override target if provided
$targetFinal = if ([string]::IsNullOrEmpty($TargetBranch)) { $targetBranchConfig } else { $TargetBranch }

Write-ColorOutput Blue "Current Worktree:"
Write-Output "  Task Name:      $taskName"
Write-Output "  Task Branch:    $taskBranch"
Write-Output "  Target Branch:  $targetBranchConfig"
if (-not [string]::IsNullOrEmpty($TargetBranch)) {
    Write-Output "  Override Target: $targetFinal"
}
Write-Output "  Worktree Dir:   $worktreeDir"
Write-Output "  Root Directory: $rootDir"
Write-Output "  Original Branch: $originalBranch"
Write-Output ""

# Step 3: Verify task completion
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$checkScript = Join-Path $scriptDir "check-complete.sh"

if (Test-Path $checkScript) {
    Write-ColorOutput Blue "Checking task completion..."
    $result = & bash $checkScript 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Output ""
        Write-ColorOutput Yellow "WARNING: Not all phases are marked complete"
        Write-Output ""
        $confirm = Read-Host "Continue anyway? [y/N]"
        if ($confirm -ne "y" -and $confirm -ne "Y") {
            Write-Output "Cancelled."
            exit 1
        }
    }
}

# Step 4: Check for uncommitted changes
Write-ColorOutput Blue "Checking for uncommitted changes..."
$status = & git status --porcelain 2>$null
if ($status) {
    Write-ColorOutput Yellow "There are uncommitted changes:"
    & git status --short
    Write-Output ""
    Write-Output "Options:"
    Write-Output "  1) Commit changes now"
    Write-Output "  2) Stash changes"
    Write-Output "  3) Cancel and handle manually"
    Write-Output ""
    $choice = Read-Host "Choose [1/2/3]"
    switch ($choice) {
        "1" {
            $msg = Read-Host "Enter commit message"
            & git add -A
            & git commit -m ($msg ?? "Complete task phase")
        }
        "2" {
            & git stash push -m "Worktree complete stash"
        }
        "3" {
            Write-Output "Cancelled."
            exit 0
        }
        default {
            Write-ColorOutput Red "Invalid choice"
            exit 1
        }
    }
}

# Step 5: Show summary
Write-Output ""
Write-ColorOutput Cyan "=== Worktree Completion Summary ==="
Write-Output ""
Write-Output "This will:"
Write-Output "  1. Delete planning files from worktree"
Write-Output "  2. Navigate to root directory"
Write-Output "  3. Merge $taskBranch into $targetFinal"
Write-Output "  4. Delete this worktree"
Write-Output "  5. Delete the task branch"
Write-Output ""
$confirm = Read-Host "Proceed? [Y/n]"
if ($confirm -eq "n" -or $confirm -eq "N") {
    Write-Output "Cancelled."
    exit 0
}

# Step 6: Delete planning files from worktree
Write-Output ""
Write-ColorOutput Blue "Deleting planning files..."
$planningFiles = @("task_plan.md", "findings.md", "progress.md")
foreach ($file in $planningFiles) {
    if (Test-Path $file) {
        Remove-Item $file
        Write-Output "  Deleted: $file"
    }
}
if (Test-Path $ConfigFile) {
    Remove-Item $ConfigFile
    Write-Output "  Deleted: $ConfigFile"
}

# Step 7: Navigate to root directory
Write-Output ""
Write-ColorOutput Blue "Navigating to root directory..."
Set-Location $rootDir
Write-Output "  Now in: $(Get-Location)"

# Step 8: Switch to target branch
Write-Output ""
Write-ColorOutput Blue "Switching to target branch: $targetFinal"

# Fetch if remote exists
$null = & git ls-remote --exit-code origin $targetFinal 2>$null
if ($LASTEXITCODE -eq 0) {
    & git fetch origin $targetFinal 2>$null
}

$checkoutResult = & git checkout $targetFinal 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Output "  Checked out: $targetFinal"
} else {
    # Try to create from origin
    $checkoutResult = & git checkout -b $targetFinal "origin/$targetFinal" 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Output "  Created and checked out: $targetFinal from origin"
    } else {
        Write-ColorOutput Yellow "Warning: Could not checkout $targetFinal"
        Write-Output "  Staying on: $(& git branch --show-current)"
    }
}

# Step 9: Merge task branch
Write-Output ""
Write-ColorOutput Blue "Merging $taskBranch into $targetFinal..."

$mergeResult = & git merge --no-ff -m "Merge task branch: $taskName" $taskBranch 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-ColorOutput Green "Merge successful!"
} else {
    Write-Output ""
    Write-ColorOutput Red "=== MERGE CONFLICT DETECTED ==="
    Write-Output ""
    Write-Output "Merge conflicts need to be resolved manually."
    Write-Output ""
    Write-Output "After resolving conflicts:"
    Write-Output "  1. Run: git add ."
    Write-Output "  2. Run: git commit"
    Write-Output "  3. Run: git worktree remove $worktreeDir"
    Write-Output "  4. Run: git branch -d $taskBranch"
    Write-Output ""
    Write-Output "Or abort with: git merge --abort"
    exit 1
}

# Step 10: Remove worktree
Write-Output ""
Write-ColorOutput Blue "Removing worktree: $worktreeDir"
$removeResult = & git worktree remove $worktreeDir 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-ColorOutput Green "Worktree removed"
} else {
    # Fallback to manual removal
    Remove-Item -Recurse -Force $worktreeDir -ErrorAction SilentlyContinue
    Write-ColorOutput Yellow "Worktree directory removed (manually)"
}

# Step 11: Delete task branch
Write-Output ""
Write-ColorOutput Blue "Deleting task branch: $taskBranch"
$deleteResult = & git branch -d $taskBranch 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-ColorOutput Green "Task branch deleted"
} else {
    Write-ColorOutput Yellow "Warning: Could not delete branch $taskBranch"
    Write-Output "  You may need to delete it manually with: git branch -D $taskBranch"
}

# Step 12: Summary
Write-Output ""
Write-ColorOutput Green "=== Task Completed Successfully ==="
Write-Output ""
Write-Output "Task: $taskName"
Write-Output "Branch: $taskBranch merged into $targetFinal"
Write-Output ""
Write-Output "Planning files have been deleted."
Write-Output "Worktree has been removed."
Write-Output ""
Write-Output "Current branch: $(& git branch --show-current)"
Write-Output "Current directory: $(Get-Location)"
Write-Output ""
Write-ColorOutput Cyan "=== Active Worktrees ==="
& git worktree list
Write-Output ""
Write-ColorOutput Yellow "Next:"
Write-Output "  - Push the merge if needed: git push"
Write-Output "  - Continue with your next task"
Write-Output ""
