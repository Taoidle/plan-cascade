# Worktree Complete Script for PowerShell
# Usage: .\worktree-complete.ps1 [[-TargetBranch] <string>]

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

Write-ColorOutput Green "=== Planning with Files - Worktree Complete ==="
Write-Output ""

# Step 1: Read configuration
$ConfigFile = ".planning-config.json"

if (-not (Test-Path $ConfigFile)) {
    Write-ColorOutput Red "ERROR: No planning configuration found"
    Write-Output "Are you in a worktree session?"
    exit 1
}

$config = Get-Content $ConfigFile | ConvertFrom-Json

$mode = $config.mode
$taskBranch = $config.task_branch
$targetBranchConfig = $config.target_branch
$worktreeDir = $config.worktree_dir

if ($mode -ne "worktree") {
    Write-ColorOutput Red "ERROR: Not in worktree mode (current mode: $mode)"
    exit 1
}

# Use override target if provided
$targetFinal = if ([string]::IsNullOrEmpty($TargetBranch)) { $targetBranchConfig } else { $TargetBranch }

Write-ColorOutput Cyan "Configuration:"
Write-Output "  Task Branch:    $taskBranch"
Write-Output "  Target Branch:  $targetBranchConfig"
if (-not [string]::IsNullOrEmpty($TargetBranch)) {
    Write-Output "  Override:       $targetFinal"
}
Write-Output ""

# Step 2: Verify completion
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$checkScript = Join-Path $scriptDir "check-complete.sh"

if (Test-Path $checkScript) {
    Write-ColorOutput Cyan "Checking task completion..."
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

# Step 3: Check for uncommitted changes
Write-ColorOutput Cyan "Checking for uncommitted changes..."
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

# Step 4: Show summary
Write-Output ""
Write-ColorOutput Cyan "=== Worktree Completion Summary ==="
Write-Output ""
Write-Output "Files to Delete:"
Write-Output "  - task_plan.md"
Write-Output "  - findings.md"
Write-Output "  - progress.md"
Write-Output "  - .planning-config.json"
Write-Output ""
Write-Output "Actions:"
Write-Output "  1. Delete planning files and config"
Write-Output "  2. Switch to target branch ($targetFinal)"
Write-Output "  3. Merge task branch ($taskBranch) into target"
Write-Output "  4. Delete task branch"
Write-Output "  5. Clean up worktree (if applicable)"
Write-Output ""
$confirm = Read-Host "Proceed? [Y/n]"
if ($confirm -eq "n" -or $confirm -eq "N") {
    Write-Output "Cancelled."
    exit 0
}

# Step 5: Delete planning files
Write-Output ""
Write-ColorOutput Cyan "Deleting planning files..."
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

# Step 6: Switch to target branch
Write-Output ""
Write-ColorOutput Cyan "Switching to target branch: $targetFinal"

# Fetch if remote exists
$null = & git ls-remote --exit-code origin "$targetFinal" 2>$null
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
        Write-ColorOutput Yellow "Warning: Could not checkout $targetFinal, staying on current branch"
    }
}

# Step 7: Merge task branch
Write-Output ""
Write-ColorOutput Cyan "Merging $taskBranch into $targetFinal..."

$mergeResult = & git merge --no-ff -m "Merge task branch: $taskBranch" $taskBranch 2>&1
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
    Write-Output "  3. Run: git branch -d $taskBranch"
    Write-Output ""
    Write-Output "Or abort with: git merge --abort"
    exit 1
}

# Step 8: Delete task branch
Write-Output ""
Write-ColorOutput Cyan "Cleaning up task branch: $taskBranch"
$deleteResult = & git branch -d $taskBranch 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Output "  Deleted branch: $taskBranch"
} else {
    Write-ColorOutput Yellow "Warning: Could not delete branch $taskBranch"
}

# Step 9: Cleanup worktree if exists
$worktreePath = ".worktree/$taskBranch"
if (Test-Path $worktreePath) {
    Write-Output ""
    Write-ColorOutput Cyan "Cleaning up worktree: $worktreePath"
    try {
        & git worktree remove $worktreePath 2>$null
        if ($LASTEXITCODE -eq 0) {
            Write-Output "  Removed worktree: $worktreePath"
        } else {
            Remove-Item -Recurse -Force $worktreePath 2>$null
        }
    } catch {
        Write-ColorOutput Yellow "Warning: Could not remove $worktreePath"
    }
}

# Step 10: Summary
Write-Output ""
Write-ColorOutput Green "=== Task Completed Successfully ==="
Write-Output ""
Write-Output "Task branch $taskBranch has been merged into $targetFinal."
Write-Output ""
Write-Output "Planning files have been deleted."
Write-Output ""
$currentBranch = & git branch --show-current
Write-Output "Current branch: $currentBranch"
Write-Output ""
Write-ColorOutput Yellow "Next:"
Write-Output "  - Push the merge if needed: git push"
Write-Output "  - Continue with your next task"
