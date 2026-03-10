param(
  [switch]$SkipSystemPackages,
  [switch]$SkipVendorSync,
  [switch]$SkipPnpmInstall,
  [switch]$SkipVerify
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$DesktopDir = Split-Path -Parent $ScriptDir
$RepoRoot = Split-Path -Parent $DesktopDir
$VendorMeta = Join-Path $DesktopDir 'src-tauri\vendor-patches\openai-api-rs\UPSTREAM.md'
$VendorDir = Join-Path $DesktopDir 'src-tauri\vendor\openai-api-rs'
$PatchDir = Join-Path $DesktopDir 'src-tauri\vendor-patches\openai-api-rs'
$NodeMajorMin = 20
$PnpmVersion = '10'

function Write-Step {
  param([string]$Message)
  Write-Host ""
  Write-Host "[setup] $Message" -ForegroundColor Cyan
}

function Refresh-Path {
  $machine = [System.Environment]::GetEnvironmentVariable('Path', 'Machine')
  $user = [System.Environment]::GetEnvironmentVariable('Path', 'User')
  $env:Path = "$machine;$user"
}

function Require-Command {
  param([string]$Name)
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Required command not found: $Name"
  }
}

function Ensure-Winget {
  Require-Command 'winget'
}

function Install-WingetPackage {
  param(
    [Parameter(Mandatory = $true)][string]$Id,
    [string]$Override
  )

  $args = @(
    'install',
    '--id', $Id,
    '-e',
    '--source', 'winget',
    '--accept-package-agreements',
    '--accept-source-agreements'
  )

  if ($Override) {
    $args += @('--override', $Override)
  }

  & winget @args
}

function Get-NodeMajorVersion {
  if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    return $null
  }

  $version = (& node -p "process.versions.node.split('.')[0]").Trim()
  if ([string]::IsNullOrWhiteSpace($version)) {
    return $null
  }

  return [int]$version
}

function Ensure-SystemPackages {
  if ($SkipSystemPackages) {
    Write-Step 'Skipping Windows system package installation'
    return
  }

  Ensure-Winget

  if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Write-Step 'Installing Git for Windows'
    Install-WingetPackage -Id 'Git.Git'
    Refresh-Path
  }

  $nodeMajor = Get-NodeMajorVersion
  if ($null -eq $nodeMajor -or $nodeMajor -lt $NodeMajorMin) {
    Write-Step 'Installing Node.js LTS'
    Install-WingetPackage -Id 'OpenJS.NodeJS.LTS'
    Refresh-Path
  }

  if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Write-Step 'Installing rustup'
    Install-WingetPackage -Id 'Rustlang.Rustup'
    Refresh-Path
  }

  if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    Write-Step 'Installing Visual Studio C++ Build Tools'
    Install-WingetPackage -Id 'Microsoft.VisualStudio.2022.BuildTools' -Override '--wait --quiet --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    Refresh-Path
  }

  $webView2Key = 'HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
  if (-not (Test-Path $webView2Key)) {
    Write-Step 'Installing WebView2 Runtime'
    Install-WingetPackage -Id 'Microsoft.EdgeWebView2Runtime'
    Refresh-Path
  }
}

function Ensure-Rust {
  Refresh-Path
  if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    throw 'rustup is not available after installation.'
  }

  Write-Step 'Configuring Rust stable toolchain'
  & rustup toolchain install stable --profile minimal | Out-Host
  & rustup default stable | Out-Host
  & rustup component add rustfmt clippy | Out-Null

  if (Test-Path (Join-Path $env:USERPROFILE '.cargo\bin')) {
    $env:Path = "$($env:USERPROFILE)\.cargo\bin;$env:Path"
  }
}

function Ensure-CorepackPnpm {
  Require-Command 'node'
  Require-Command 'corepack'

  Write-Step "Enabling corepack and pnpm@$PnpmVersion"
  & corepack enable
  & corepack prepare "pnpm@$PnpmVersion" --activate
  Refresh-Path
  Require-Command 'pnpm'
}

function Read-MetaValue {
  param([string]$Key)
  $pattern = '^{0}: ' -f [regex]::Escape($Key)
  $replacePattern = '^{0}:\s*' -f [regex]::Escape($Key)
  $line = Get-Content $VendorMeta | Where-Object { $_ -match $pattern } | Select-Object -First 1
  if (-not $line) {
    throw "Metadata key not found in ${VendorMeta}: ${Key}"
  }

  return ($line -replace $replacePattern, '').Trim()
}

function Copy-DirectoryMirror {
  param(
    [Parameter(Mandatory = $true)][string]$Source,
    [Parameter(Mandatory = $true)][string]$Destination
  )

  if (Test-Path $Destination) {
    Remove-Item $Destination -Recurse -Force
  }
  New-Item -ItemType Directory -Path $Destination | Out-Null

  Get-ChildItem -LiteralPath $Source -Force | Where-Object { $_.Name -ne '.git' } | ForEach-Object {
    Copy-Item -LiteralPath $_.FullName -Destination $Destination -Recurse -Force
  }
}

function Sync-VendorPatchQueue {
  if ($SkipVendorSync) {
    Write-Step 'Skipping vendored openai-api-rs sync'
    return
  }

  Require-Command 'git'

  $upstreamRepo = Read-MetaValue -Key 'upstream_repo'
  $upstreamRef = Read-MetaValue -Key 'upstream_ref'
  $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("openai-api-rs-sync-" + [guid]::NewGuid().ToString('N'))
  $upstreamDir = Join-Path $tempRoot 'upstream'

  try {
    New-Item -ItemType Directory -Path $tempRoot | Out-Null

    Write-Step "Cloning $upstreamRepo"
    & git clone --filter=blob:none $upstreamRepo $upstreamDir | Out-Host
    & git -C $upstreamDir checkout --quiet $upstreamRef

    $patches = Get-ChildItem -Path $PatchDir -Filter '*.patch' | Sort-Object Name
    foreach ($patch in $patches) {
      Write-Step "Applying patch $($patch.Name)"
      & git -C $upstreamDir apply --whitespace=nowarn $patch.FullName
    }

    Write-Step 'Syncing vendored openai-api-rs'
    Copy-DirectoryMirror -Source $upstreamDir -Destination $VendorDir
  }
  finally {
    if (Test-Path $tempRoot) {
      Remove-Item $tempRoot -Recurse -Force
    }
  }
}

function Install-FrontendDeps {
  if ($SkipPnpmInstall) {
    Write-Step 'Skipping pnpm install'
    return
  }

  Write-Step 'Installing desktop frontend dependencies'
  Push-Location $DesktopDir
  try {
    & pnpm install --frozen-lockfile
  }
  finally {
    Pop-Location
  }
}

function Verify-DesktopEnv {
  if ($SkipVerify) {
    Write-Step 'Skipping verification'
    return
  }

  Write-Step 'Running cargo check'
  & cargo check --manifest-path (Join-Path $DesktopDir 'src-tauri\Cargo.toml') --lib

  Write-Step 'Running TypeScript check'
  & pnpm -C $DesktopDir exec tsc --noEmit
}

function Main {
  Ensure-SystemPackages
  Ensure-Rust
  Ensure-CorepackPnpm
  Sync-VendorPatchQueue
  Install-FrontendDeps
  Verify-DesktopEnv

  Write-Host ""
  Write-Host "[setup] Desktop development environment is ready." -ForegroundColor Green
  Write-Host ""
  Write-Host "Next steps:"
  Write-Host "  cd $DesktopDir"
  Write-Host "  pnpm tauri:dev"
}

Main
