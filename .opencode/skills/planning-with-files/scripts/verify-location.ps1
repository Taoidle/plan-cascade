# Verify location for worktree mode
# Checks if the current directory is correct for worktree mode operations
# Exit codes:
#   0 - Correct location (in worktree directory)
#   1 - Wrong location (in root but should be in worktree)
#   2 - Not in worktree mode (no .planning-config.json found)

$ErrorActionPreference = "Stop"

# Check if .planning-config.json exists in current directory
if (-not (Test-Path ".planning-config.json")) {
    # Not in worktree mode, no verification needed
    exit 2
}

# Read the config to get expected locations
try {
    $config = Get-Content ".planning-config.json" -Raw | ConvertFrom-Json
    $mode = $config.mode
    $rootDir = $config.root_dir
    $worktreeDir = $config.worktree_dir
} catch {
    # Invalid JSON, skip verification
    exit 2
}

# Only verify if mode is "worktree"
if ($mode -ne "worktree") {
    exit 2
}

# Get current directory
$currentDir = Get-Location | Select-Object -ExpandProperty Path

# Check if we're in the root directory when we should be in worktree
if ($rootDir -and $currentDir -eq $rootDir) {
    if ($worktreeDir) {
        Write-Host "ERROR: You are in the wrong directory!" -ForegroundColor Red
        Write-Host "Worktree mode requires working in the worktree directory."
        Write-Host ""
        Write-Host "Current location: $currentDir"
        Write-Host "Expected location: $worktreeDir"
        Write-Host ""
        Write-Host "Please navigate to the worktree:"
        Write-Host "  cd $worktreeDir" -ForegroundColor Cyan
        exit 1
    }
}

# Check if current directory matches the expected worktree directory
if ($worktreeDir) {
    # If WORKTREE_DIR is relative, make it absolute relative to root
    if (-not [System.IO.Path]::IsPathRooted($worktreeDir)) {
        if ($rootDir) {
            $expectedDir = Join-Path $rootDir $worktreeDir
        } else {
            $expectedDir = $worktreeDir
        }
    } else {
        $expectedDir = $worktreeDir
    }

    # Normalize paths
    $currentDirNormalized = (Get-Item $currentDir).FullName
    $expectedDirNormalized = (Get-Item $expectedDir).FullName

    if ($currentDirNormalized -ne $expectedDirNormalized) {
        Write-Host "ERROR: You are in the wrong directory!" -ForegroundColor Red
        Write-Host "Worktree mode requires working in the worktree directory."
        Write-Host ""
        Write-Host "Current location: $currentDirNormalized"
        Write-Host "Expected location: $expectedDirNormalized"
        Write-Host ""
        Write-Host "Please navigate to the worktree:"
        Write-Host "  cd $worktreeDir" -ForegroundColor Cyan
        exit 1
    }
}

# Location is correct
exit 0
