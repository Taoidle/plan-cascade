# Hybrid Ralph Worktree Completion Script (PowerShell)
# Verifies PRD completion and cleans up worktree

param(
    [string]$TargetBranch = ""
)

# Helper functions
function Write-ColorMsg {
    param(
        [string]$Color,
        [string]$Message
    )

    $colorMap = @{
        "Red" = "Red"
        "Green" = "Green"
        "Yellow" = "Yellow"
        "Blue" = "Cyan"
    }

    Write-Host $Message -ForegroundColor $colorMap[$Color]
}

function Error-Exit {
    param([string]$Message)
    Write-ColorMsg "Red" "Error: $Message"
    exit 1
}

# Check if in worktree mode
function Test-WorktreeMode {
    if (-not (Test-Path ".planning-config.json")) {
        Error-Exit "Not in worktree mode. .planning-config.json not found."
    }

    $config = Get-Content ".planning-config.json" -Raw | ConvertFrom-Json
    if ($config.mode -ne "hybrid") {
        Error-Exit "Not in hybrid worktree mode. Use /planning-with-files:complete instead."
    }

    return $config
}

# Read planning config
function Get-PlanningConfig {
    $config = Get-Content ".planning-config.json" -Raw | ConvertFrom-Json
    return @{
        TaskName = $config.task_name
        TaskBranch = $config.task_branch
        RootDir = $config.root_dir
    }
}

# Verify all stories complete
function Test-StoriesComplete {
    Write-ColorMsg "Blue" "Verifying PRD completion..."

    if (-not (Test-Path "prd.json")) {
        Error-Exit "prd.json not found. Cannot verify completion."
    }

    $prd = Get-Content "prd.json" -Raw | ConvertFrom-Json
    $storyIds = $prd.stories | ForEach-Object { $_.id }

    if (-not $storyIds -or $storyIds.Count -eq 0) {
        Write-ColorMsg "Yellow" "⚠ No stories found in PRD"
        return $true
    }

    $incomplete = @()

    foreach ($storyId in $storyIds) {
        if (Test-Path "progress.txt") {
            $progress = Get-Content "progress.txt" -Raw
            if ($progress -notmatch "\[COMPLETE\] $storyId") {
                $story = $prd.stories | Where-Object { $_.id -eq $storyId }
                $incomplete += "  - $storyId`: $($story.title)"
            }
        } else {
            $story = $prd.stories | Where-Object { $_.id -eq $storyId }
            $incomplete += "  - $storyId`: $($story.title)"
        }
    }

    if ($incomplete.Count -gt 0) {
        Write-ColorMsg "Red" "✗ Not all stories are complete:"
        $incomplete | ForEach-Object { Write-Host $_ }
        Error-Exit "Complete all stories before running /hybrid:complete"
    }

    Write-ColorMsg "Green" "✓ All $($storyIds.Count) stories complete!"
    return $true
}

# Show completion summary
function Show-CompletionSummary {
    param(
        [string]$TaskName,
        [string]$TaskBranch,
        [string]$TargetBranch
    )

    Write-ColorMsg "Blue" "═══════════════════════════════════════════════════"
    Write-ColorMsg "Blue" "  COMPLETION SUMMARY"
    Write-ColorMsg "Blue" "═══════════════════════════════════════════════════"
    Write-Host ""
    Write-ColorMsg "Yellow" "Task: $TaskName"
    Write-ColorMsg "Yellow" "Branch: $TaskBranch"
    Write-ColorMsg "Yellow" "Target: $TargetBranch"
    Write-Host ""

    # Show stories
    if (Test-Path "prd.json") {
        $prd = Get-Content "prd.json" -Raw | ConvertFrom-Json
        Write-ColorMsg "Yellow" "Stories: $($prd.stories.Count) total"

        $prd.stories | ForEach-Object {
            Write-Host "  ✓ $($_.id): $($_.title)"
        }
    }

    Write-Host ""
    Write-ColorMsg "Green" "All stories complete!"
    Write-Host ""
}

# Cleanup worktree files
function Remove-WorktreeFiles {
    Write-ColorMsg "Blue" "Cleaning up worktree files..."

    # Remove planning files
    if (Test-Path "prd.json") { Remove-Item "prd.json" -Force }
    if (Test-Path "findings.md") { Remove-Item "findings.md" -Force }
    if (Test-Path "progress.txt") { Remove-Item "progress.txt" -Force }
    if (Test-Path ".planning-config.json") { Remove-Item ".planning-config.json" -Force }

    # Remove agent outputs
    if (Test-Path ".agent-outputs") { Remove-Item ".agent-outputs" -Recurse -Force }

    Write-ColorMsg "Green" "✓ Worktree files cleaned up"
}

# Navigate to root and merge
function Invoke-MergeToTarget {
    param(
        [string]$TaskBranch,
        [string]$RootDir,
        [string]$TargetBranch
    )

    Write-ColorMsg "Blue" "Navigating to root directory..."

    if ($RootDir -and (Test-Path $RootDir)) {
        Set-Location $RootDir
    } else {
        # Try to navigate up from .worktree/
        if ($PWD -match "\.worktree\\") {
            Set-Location "..\..\"
        }
    }

    Write-ColorMsg "Green" "✓ Now in project root"

    # Detect target branch if not provided
    if ([string]::IsNullOrWhiteSpace($TargetBranch)) {
        $mainExists = git show-ref --verify --quiet refs/heads/main 2>$null
        if ($mainExists) {
            $TargetBranch = "main"
        } else {
            $masterExists = git show-ref --verify --quiet refs/heads/master 2>$null
            if ($masterExists) {
                $TargetBranch = "master"
            } else {
                # Get current branch as fallback
                $TargetBranch = git branch --show-current
            }
        }
        Write-ColorMsg "Yellow" "Auto-detected target branch: $TargetBranch"
    }

    Write-ColorMsg "Blue" "Merging $TaskBranch to $TargetBranch..."

    # Checkout target branch
    $checkoutResult = git checkout $TargetBranch 2>&1
    if ($LASTEXITCODE -ne 0) {
        Error-Exit "Cannot checkout $TargetBranch"
    }

    # Merge task branch
    $mergeResult = git merge --no-ff $TaskBranch -m "Merge $TaskName (task branch)" 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-ColorMsg "Green" "✓ Merge successful!"
    } else {
        Write-ColorMsg "Red" "✗ Merge failed or has conflicts"
        Write-ColorMsg "Yellow" "Please resolve conflicts and complete manually"
        Write-ColorMsg "Yellow" "After resolving, run: git worktree remove .worktree\$TaskName && git branch -D $TaskBranch"
        exit 1
    }

    # Remove worktree
    Write-ColorMsg "Blue" "Removing worktree..."
    $worktreeRemoveResult = git worktree remove ".worktree\$TaskName" 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-ColorMsg "Yellow" "⚠ Could not remove worktree (may need manual cleanup)"
    }

    # Delete task branch
    Write-ColorMsg "Blue" "Deleting task branch..."
    $branchDeleteResult = git branch -D $TaskBranch 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-ColorMsg "Yellow" "⚠ Could not delete task branch"
    }

    Write-ColorMsg "Green" "✓ Cleanup complete!"

    return $TargetBranch
}

# Main execution
Write-ColorMsg "Blue" "═══════════════════════════════════════════════════"
Write-ColorMsg "Blue" "  Hybrid Ralph + Worktree Completion"
Write-ColorMsg "Blue" "═══════════════════════════════════════════════════"
Write-Host ""

# Phase 1: Verification
$config = Test-WorktreeMode
$planningConfig = Get-PlanningConfig
Test-StoriesComplete

# Phase 2: Show summary
Show-CompletionSummary `
    -TaskName $planningConfig.TaskName `
    -TaskBranch $planningConfig.TaskBranch `
    -TargetBranch $TargetBranch

# Phase 3: Prompt
Write-ColorMsg "Yellow" "Ready to complete and merge."
Write-ColorMsg "Yellow" "Press Enter to continue or Ctrl+C to cancel..."
$null = Read-Host

# Phase 4: Cleanup
Remove-WorktreeFiles

# Phase 5: Navigate and merge
$finalTarget = Invoke-MergeToTarget `
    -TaskBranch $planningConfig.TaskBranch `
    -RootDir $planningConfig.RootDir `
    -TargetBranch $TargetBranch

Write-Host ""
Write-ColorMsg "Green" "═══════════════════════════════════════════════════"
Write-ColorMsg "Green" "  Task Complete!"
Write-ColorMsg "Green" "═══════════════════════════════════════════════════"
Write-Host ""
Write-ColorMsg "Green" "✓ All stories complete"
Write-ColorMsg "Green" "✓ Changes merged to $finalTarget"
Write-ColorMsg "Green" "✓ Worktree removed"
Write-ColorMsg "Green" "✓ Task branch deleted"
Write-Host ""
Write-ColorMsg "Yellow" "You can now:"
Write-Host "  - Start a new worktree task with /hybrid:worktree"
Write-Host "  - Continue working in the current directory"
Write-Host "  - Push changes with: git push origin $finalTarget"
Write-Host ""
