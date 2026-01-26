# Hybrid Ralph Worktree Initialization Script (PowerShell)
# Combines Git worktree creation with Hybrid Ralph PRD generation

param(
    [Parameter(Mandatory=$true)]
    [string]$TaskName,

    [string]$TargetBranch = "",

    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$TaskDescriptionParts
)

# Get script directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path

# Build task description from remaining parts
$TaskDescription = $TaskDescriptionParts -join " "

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

# Check if we're in a git repository
function Test-GitRepo {
    $result = git rev-parse --git-dir 2>$null
    return -not [string]::IsNullOrEmpty($result)
}

# Detect default branch
function Get-DefaultBranch {
    $branch = git symbolic-ref refs/remotes/origin/HEAD 2>$null
    if ($branch) {
        return $branch -replace "refs/remotes/origin/", ""
    }

    # Fallback to checking main or master
    $mainExists = git show-ref --verify --quiet refs/heads/main 2>$null
    if ($mainExists) {
        return "main"
    }

    $masterExists = git show-ref --verify --quiet refs/heads/master 2>$null
    if ($masterExists) {
        return "master"
    }

    return "main"  # Default
}

# Create worktree
function New-Worktree {
    param(
        [string]$Name,
        [string]$Target
    )

    # Generate timestamp for branch name
    $timestamp = Get-Date -Format "yyyyMMdd-HHmm"
    $taskBranch = "task-${timestamp}"

    # Worktree path
    $worktreePath = ".worktree/${Name}"

    # Check if worktree already exists
    if (Test-Path $worktreePath) {
        Error-Exit "Worktree already exists at ${worktreePath}"
    }

    Write-ColorMsg "Blue" "Creating Git worktree..."
    Write-ColorMsg "Yellow" "  Task name: ${Name}"
    Write-ColorMsg "Yellow" "  Task branch: ${taskBranch}"
    Write-ColorMsg "Yellow" "  Target branch: ${Target}"
    Write-ColorMsg "Yellow" "  Worktree path: ${worktreePath}"

    # Create worktree
    $result = git worktree add -b $taskBranch $worktreePath $Target 2>&1

    if ($LASTEXITCODE -eq 0) {
        Write-ColorMsg "Green" "✓ Worktree created successfully!"
        return $worktreePath
    } else {
        Error-Exit "Failed to create worktree: $result"
    }
}

# Initialize hybrid ralph in worktree
function Initialize-HybridRalph {
    param(
        [string]$WorktreePath,
        [string]$TaskDescription
    )

    Write-ColorMsg "Blue" "Initializing Hybrid Ralph in worktree..."

    # Get current branch name
    Push-Location $WorktreePath
    $currentBranch = git branch --show-current
    Pop-Location

    # Create planning config
    $config = @{
        mode = "hybrid"
        task_name = (Split-Path -Leaf $WorktreePath)
        created_at = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
        task_branch = $currentBranch
        root_dir = (Resolve-Path (Join-Path $WorktreePath "../..")).Path
    } | ConvertTo-Json -Depth 10

    $config | Out-File -FilePath (Join-Path $WorktreePath ".planning-config.json") -Encoding utf8

    Write-ColorMsg "Green" "✓ Planning config created"

    # Create initial PRD template if description provided
    if (-not [string]::IsNullOrWhiteSpace($TaskDescription)) {
        Write-ColorMsg "Blue" "Generating PRD from description..."

        $prdScript = Join-Path $ScriptDir "prd-generate.py"
        if (Test-Path $prdScript) {
            $pythonCmd = Get-Command python -ErrorAction SilentlyContinue
            if ($pythonCmd) {
                & python $prdScript $TaskDescription | Out-File -FilePath (Join-Path $WorktreePath "prd.json") -Encoding utf8
                Write-ColorMsg "Green" "✓ PRD template created"
            }
        }
    }

    # Create empty files
    $null = New-Item -Path (Join-Path $WorktreePath "findings.md") -ItemType File -Force
    $null = New-Item -Path (Join-Path $WorktreePath "progress.txt") -ItemType File -Force

    Write-ColorMsg "Green" "✓ Hybrid Ralph initialized!"
}

# Main execution
Write-ColorMsg "Blue" "═══════════════════════════════════════════════════"
Write-ColorMsg "Blue" "  Hybrid Ralph + Worktree Setup"
Write-ColorMsg "Blue" "═══════════════════════════════════════════════════"
Write-Host ""

# Validate task name
if ([string]::IsNullOrWhiteSpace($TaskName)) {
    Error-Exit "Task name is required"
}

# Check git repository
if (-not (Test-GitRepo)) {
    Error-Exit "Not a git repository. Please run: git init"
}

# Detect target branch if not provided
if ([string]::IsNullOrWhiteSpace($TargetBranch)) {
    $TargetBranch = Get-DefaultBranch
    Write-ColorMsg "Yellow" "Auto-detected target branch: ${TargetBranch}"
}

# Verify target branch exists
$targetExists = git show-ref --verify --quiet "refs/heads/${TargetBranch}" 2>$null
if (-not $targetExists) {
    $targetExists = git show-ref --verify --quiet "refs/remotes/origin/${TargetBranch}" 2>$null
}

if (-not $targetExists) {
    Error-Exit "Target branch '${TargetBranch}' not found"
}

# Create worktree
$worktreePath = New-Worktree -Name $TaskName -Target $TargetBranch

Write-Host ""

# Initialize hybrid ralph
Initialize-HybridRalph -WorktreePath $worktreePath -TaskDescription $TaskDescription

Write-Host ""
Write-ColorMsg "Green" "═══════════════════════════════════════════════════"
Write-ColorMsg "Green" "  Setup Complete!"
Write-ColorMsg "Green" "═══════════════════════════════════════════════════"
Write-Host ""
Write-ColorMsg "Yellow" "Next steps:"
Write-Host "  1. cd $worktreePath"
Write-Host "  2. Review/edit prd.json if needed"
Write-Host "  3. Run: /approve"
Write-Host ""
Write-ColorMsg "Yellow" "Or run hybrid commands from the worktree:"
Write-Host "  cd $worktreePath"
Write-Host "  /hybrid:auto '$TaskDescription'"
Write-Host "  /approve"
Write-Host ""
Write-ColorMsg "Yellow" "When complete:"
Write-Host "  /hybrid:complete $TargetBranch"
Write-Host ""
