# Run Cargo from a worktree while reusing the main checkout's warm target directory.
# Usage: .\cargo-worktree.ps1 check --locked
# From older worktrees: & C:\Dev\codex-minibar\cargo-worktree.ps1 check --locked
# Quote Cargo's '--' separator when invoking this PowerShell script, for example:
# .\cargo-worktree.ps1 clippy --locked '--' -D warnings
# Keep this a simple script: advanced binding would consume Clippy's -D as Debug.
param(
    [ValidateSet("check", "build", "test", "clippy", "fmt", "metadata", "tree", "fetch")]
    [string]$Command = "check"
)

$CargoArguments = @($args)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

# Resolve through Git, not the worktree's parent directory: T3 and Codex put
# worktrees in different locations. The main checkout already has a warm cache.
$commonDir = & git rev-parse --path-format=absolute --git-common-dir
if ($LASTEXITCODE -ne 0) { throw "Run this script inside a Codex Minibar worktree." }
$scriptCommonDir = & git -C $PSScriptRoot rev-parse --path-format=absolute --git-common-dir
if ($LASTEXITCODE -ne 0) { throw "Could not resolve this script's repository." }
if ([IO.Path]::GetFullPath($commonDir) -ne [IO.Path]::GetFullPath($scriptCommonDir)) {
    throw "The current directory belongs to a different repository."
}
$worktreeRoot = & git rev-parse --show-toplevel
if ($LASTEXITCODE -ne 0) { throw "Could not resolve the current worktree." }

$previousTarget = [Environment]::GetEnvironmentVariable("CARGO_TARGET_DIR", "Process")
$targetDir = if ([string]::IsNullOrWhiteSpace($previousTarget)) {
    Join-Path (Split-Path -Parent $commonDir) "target"
} else {
    # Resolve a caller's relative override before changing the working directory.
    $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($previousTarget)
}

$cargoExitCode = 1
Push-Location $worktreeRoot
try {
    $env:CARGO_TARGET_DIR = $targetDir
    Write-Host "Cargo worktree: $worktreeRoot"
    Write-Host "Cargo target:   $targetDir"
    & cargo $Command @CargoArguments
    $cargoExitCode = $LASTEXITCODE
} finally {
    [Environment]::SetEnvironmentVariable("CARGO_TARGET_DIR", $previousTarget, "Process")
    Pop-Location
}
exit $cargoExitCode
