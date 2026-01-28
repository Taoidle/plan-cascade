# Plan Cascade MCP Setup Script (PowerShell)
# Usage: .\setup-mcp.ps1 [tool]
# Tools: cursor, windsurf, cline, continue, zed, claude

param(
    [Parameter(Position=0)]
    [string]$Tool = ""
)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$PlanCascadePath = Split-Path -Parent $ScriptDir

Write-Host "Plan Cascade MCP Setup"
Write-Host "======================"
Write-Host "Plan Cascade path: $PlanCascadePath"
Write-Host ""

function Replace-Placeholder {
    param(
        [string]$SourceFile,
        [string]$DestFile
    )
    $content = Get-Content $SourceFile -Raw
    $content = $content -replace '\{\{PLAN_CASCADE_PATH\}\}', $PlanCascadePath.Replace('\', '\\')
    Set-Content -Path $DestFile -Value $content -NoNewline
    Write-Host "Created: $DestFile"
}

switch ($Tool.ToLower()) {
    "cursor" {
        Write-Host "Setting up Cursor..."
        $cursorDir = Join-Path $PlanCascadePath ".cursor"
        if (-not (Test-Path $cursorDir)) {
            New-Item -ItemType Directory -Path $cursorDir | Out-Null
        }
        Replace-Placeholder "$ScriptDir\cursor-mcp.json" "$cursorDir\mcp.json"
        Write-Host "Done! Restart Cursor to apply changes."
    }

    "windsurf" {
        Write-Host "Setting up Windsurf..."
        $windsurfDir = Join-Path $env:USERPROFILE ".codeium\windsurf"
        if (-not (Test-Path $windsurfDir)) {
            New-Item -ItemType Directory -Path $windsurfDir -Force | Out-Null
        }
        Replace-Placeholder "$ScriptDir\windsurf-mcp.json" "$windsurfDir\mcp_config.json"
        Write-Host "Done! Restart Windsurf to apply changes."
    }

    "cline" {
        Write-Host "Setting up Cline..."
        Write-Host "Add the following to your VS Code settings.json:"
        Write-Host ""
        $content = Get-Content "$ScriptDir\cline-settings.json" -Raw
        $content = $content -replace '\{\{PLAN_CASCADE_PATH\}\}', $PlanCascadePath.Replace('\', '\\')
        Write-Host $content
        Write-Host ""
    }

    "continue" {
        Write-Host "Setting up Continue..."
        Write-Host "Add the following to your ~/.continue/config.json:"
        Write-Host ""
        $content = Get-Content "$ScriptDir\continue-config.json" -Raw
        $content = $content -replace '\{\{PLAN_CASCADE_PATH\}\}', $PlanCascadePath.Replace('\', '\\')
        Write-Host $content
        Write-Host ""
    }

    "zed" {
        Write-Host "Setting up Zed..."
        Write-Host "Add the following to your settings:"
        Write-Host ""
        $content = Get-Content "$ScriptDir\zed-settings.json" -Raw
        $content = $content -replace '\{\{PLAN_CASCADE_PATH\}\}', $PlanCascadePath.Replace('\', '\\')
        Write-Host $content
        Write-Host ""
    }

    "claude" {
        Write-Host "Setting up Claude Code..."
        Write-Host "Run the following command:"
        Write-Host ""
        Write-Host "  claude mcp add plan-cascade -- python -m mcp_server.server"
        Write-Host ""
        Write-Host "Or with explicit path:"
        Write-Host ""
        Write-Host "  cd $PlanCascadePath; claude mcp add plan-cascade -- python -m mcp_server.server"
        Write-Host ""
    }

    "test" {
        Write-Host "Testing MCP server..."
        Write-Host "Running: python -m mcp_server.server --debug"
        Write-Host ""
        Push-Location $PlanCascadePath
        python -m mcp_server.server --debug
        Pop-Location
    }

    "inspector" {
        Write-Host "Running MCP Inspector..."
        Push-Location $PlanCascadePath
        npx @anthropic/mcp-inspector python -m mcp_server.server
        Pop-Location
    }

    default {
        Write-Host "Usage: .\setup-mcp.ps1 [tool]"
        Write-Host ""
        Write-Host "Available tools:"
        Write-Host "  cursor     - Setup for Cursor IDE"
        Write-Host "  windsurf   - Setup for Windsurf"
        Write-Host "  cline      - Show config for Cline (VS Code)"
        Write-Host "  continue   - Show config for Continue"
        Write-Host "  zed        - Show config for Zed"
        Write-Host "  claude     - Show Claude Code command"
        Write-Host "  test       - Test MCP server locally"
        Write-Host "  inspector  - Run MCP Inspector"
        Write-Host ""
        Write-Host "Example:"
        Write-Host "  .\setup-mcp.ps1 cursor"
    }
}
