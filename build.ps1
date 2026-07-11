# Builds a portable release package with only runtime files.
# Usage: .\build.ps1 [-OutDir dist]

[CmdletBinding()]
param(
    [string]$OutDir = "dist"
)

$ErrorActionPreference = "Stop"

$Root = $PSScriptRoot
$ReleaseDir = Join-Path $Root "target\release"
$PackageDir = Join-Path $Root $OutDir

# Windows App SDK / WinUI runtime files deployed by windows-reactor-setup::as_self_contained()
# plus WebView2 Core DLL and the app binary.
$RuntimeFiles = @(
    "codex-minibar.exe",
    "CoreMessagingXP.dll",
    "dcompi.dll",
    "dwmcorei.dll",
    "DwmSceneI.dll",
    "DWriteCore.dll",
    "marshal.dll",
    "Microsoft.DirectManipulation.dll",
    "Microsoft.Graphics.Imaging.dll",
    "Microsoft.InputStateManager.dll",
    "Microsoft.Internal.FrameworkUdk.dll",
    "Microsoft.UI.Composition.OSSupport.dll",
    "Microsoft.UI.dll",
    "Microsoft.UI.Input.dll",
    "Microsoft.UI.pri",
    "Microsoft.UI.Windowing.Core.dll",
    "Microsoft.UI.Windowing.dll",
    "Microsoft.UI.Xaml.Controls.dll",
    "Microsoft.UI.Xaml.Controls.pri",
    "Microsoft.ui.xaml.dll",
    "Microsoft.UI.Xaml.Internal.dll",
    "Microsoft.UI.Xaml.Phone.dll",
    "Microsoft.ui.xaml.resources.19h1.dll",
    "Microsoft.ui.xaml.resources.common.dll",
    "Microsoft.Web.WebView2.Core.dll",
    "Microsoft.Windows.ApplicationModel.Resources.dll",
    "Microsoft.WindowsAppRuntime.dll",
    "Microsoft.WindowsAppRuntime.pri",
    "MRM.dll",
    "resources.pri",
    "SessionHandleIPCProxyStub.dll",
    "WinUIEdit.dll",
    "wuceffectsi.dll"
)

$RuntimeDirs = @(
    "Microsoft.UI.Xaml"
)

function Copy-RuntimeItem {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    if (-not (Test-Path -LiteralPath $Source)) {
        throw "Missing required runtime item: $Source"
    }

    $parent = Split-Path -Parent $Destination
    if ($parent -and -not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }

    Copy-Item -LiteralPath $Source -Destination $Destination -Recurse -Force
}

Write-Host "==> cargo build --release"
Push-Location $Root
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build --release failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

Write-Host "==> packaging into $PackageDir"
if (Test-Path -LiteralPath $PackageDir) {
    Remove-Item -LiteralPath $PackageDir -Recurse -Force
}
New-Item -ItemType Directory -Path $PackageDir -Force | Out-Null

foreach ($name in $RuntimeFiles) {
    $src = Join-Path $ReleaseDir $name
    $dst = Join-Path $PackageDir $name
    Copy-RuntimeItem -Source $src -Destination $dst
}

foreach ($name in $RuntimeDirs) {
    $src = Join-Path $ReleaseDir $name
    $dst = Join-Path $PackageDir $name
    Copy-RuntimeItem -Source $src -Destination $dst
}

# Custom title-bar images are resolved through ms-appx:///assets in the
# unpackaged WinUI host, so keep them beside the portable executable.
Copy-RuntimeItem -Source (Join-Path $Root "assets") -Destination (Join-Path $PackageDir "assets")

# Locale folders (af-ZA, en-us, ru-RU, ca-Es-VALENCIA, ...)
$skipDirs = @("deps", "build", "incremental", "examples", ".fingerprint", "Microsoft.UI.Xaml")
$localeDirs = Get-ChildItem -LiteralPath $ReleaseDir -Directory | Where-Object {
    $name = $_.Name
    $name -notin $skipDirs -and (
        (Test-Path -LiteralPath (Join-Path $_.FullName "Microsoft.ui.xaml.dll.mui")) -or
        (Test-Path -LiteralPath (Join-Path $_.FullName "Microsoft.UI.Xaml.Phone.dll.mui")) -or
        ($name -match '^[a-z]{2}(-[A-Za-z0-9]+)+$')
    )
}

foreach ($dir in $localeDirs) {
    $dst = Join-Path $PackageDir $dir.Name
    Copy-RuntimeItem -Source $dir.FullName -Destination $dst
}

$bytes = (Get-ChildItem -LiteralPath $PackageDir -Recurse -File | Measure-Object -Property Length -Sum).Sum
$sizeMb = [math]::Round($bytes / 1MB, 2)
$fileCount = (Get-ChildItem -LiteralPath $PackageDir -Recurse -File).Count

Write-Host ""
Write-Host "Done."
Write-Host "  Output : $PackageDir"
Write-Host "  Files  : $fileCount"
Write-Host "  Size   : $sizeMb MB"
Write-Host "  Launch : $(Join-Path $PackageDir 'codex-minibar.exe')"
