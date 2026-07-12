# Builds multi-arch portable zips and NSIS installers into dist/<version>/.
# Usage:
#   .\build.ps1
#   .\build.ps1 -Arch x64,arm64
#   .\build.ps1 -SkipInstaller

[CmdletBinding()]
param(
    [string]$OutDir = "dist",
    [ValidateSet("x86", "x64", "arm64")]
    [string[]]$Arch = @("x86", "x64", "arm64"),
    [switch]$SkipInstaller
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Root = $PSScriptRoot
$ProductName = "Codex Minibar"
$Publisher = "Codex Minibar"
$AppExeName = "codex-minibar.exe"
# Mirrored by Tauri — SourceForge HTML landings are not a real zip.
$NsisVersion = "3.11"
$NsisUrl = "https://github.com/tauri-apps/binary-releases/releases/download/nsis-$NsisVersion/nsis-$NsisVersion.zip"

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

$TargetMap = [ordered]@{
    x86   = "i686-pc-windows-msvc"
    x64   = "x86_64-pc-windows-msvc"
    arm64 = "aarch64-pc-windows-msvc"
}

function Get-CargoVersion {
    $cargoToml = Join-Path $Root "Cargo.toml"
    $match = Select-String -LiteralPath $cargoToml -Pattern '^\s*version\s*=\s*"([^"]+)"' |
        Select-Object -First 1
    if (-not $match) {
        throw "Could not read package version from Cargo.toml"
    }
    return $match.Matches[0].Groups[1].Value
}

$RequiredRuntimeFiles = @(
    "codex-minibar.exe",
    "Microsoft.ui.xaml.dll",
    "Microsoft.WindowsAppRuntime.dll",
    "Microsoft.Web.WebView2.Core.dll",
    "resources.pri"
)

function Copy-RuntimeItem {
    param(
        [Parameter(Mandatory = $true)][string]$Source,
        [Parameter(Mandatory = $true)][string]$Destination,
        [switch]$Optional
    )

    if (-not (Test-Path -LiteralPath $Source)) {
        if ($Optional) {
            Write-Warning "Skipping missing optional runtime item: $Source"
            return
        }
        throw "Missing required runtime item: $Source"
    }

    $parent = Split-Path -Parent $Destination
    if ($parent -and -not (Test-Path -LiteralPath $parent)) {
        New-Item -ItemType Directory -Path $parent -Force | Out-Null
    }

    Copy-Item -LiteralPath $Source -Destination $Destination -Recurse -Force
}

function Clear-WasMsixExtractCache {
    # windows-reactor-setup extracts MSIX into a shared folder without an arch
    # suffix. Clear it before each target so cross-arch builds do not reuse the
    # wrong native DLLs.
    $candidates = @(
        (Join-Path $env:LOCALAPPDATA "windows-reactor-setup\temp\Microsoft.WindowsAppSDK.Runtime-2.1.3\.msix_extract")
    )
    foreach ($path in $candidates) {
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Recurse -Force
        }
    }
}

function Ensure-RustTarget {
    param([Parameter(Mandatory = $true)][string]$Triple)

    $installed = & rustup target list --installed
    if ($LASTEXITCODE -ne 0) {
        throw "rustup target list --installed failed with exit code $LASTEXITCODE"
    }
    if ($installed -notcontains $Triple) {
        Write-Host "==> rustup target add $Triple"
        & rustup target add $Triple
        if ($LASTEXITCODE -ne 0) {
            throw "rustup target add $Triple failed with exit code $LASTEXITCODE"
        }
    }
}

function New-PortablePackage {
    param(
        [Parameter(Mandatory = $true)][string]$ReleaseDir,
        [Parameter(Mandatory = $true)][string]$PackageDir
    )

    if (Test-Path -LiteralPath $PackageDir) {
        Remove-Item -LiteralPath $PackageDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $PackageDir -Force | Out-Null

    foreach ($name in $RuntimeFiles) {
        $optional = $name -notin $RequiredRuntimeFiles
        Copy-RuntimeItem `
            -Source (Join-Path $ReleaseDir $name) `
            -Destination (Join-Path $PackageDir $name) `
            -Optional:$optional
    }

    foreach ($name in $RuntimeDirs) {
        Copy-RuntimeItem `
            -Source (Join-Path $ReleaseDir $name) `
            -Destination (Join-Path $PackageDir $name) `
            -Optional
    }

    Copy-RuntimeItem `
        -Source (Join-Path $Root "assets") `
        -Destination (Join-Path $PackageDir "assets")

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
        Copy-RuntimeItem -Source $dir.FullName -Destination (Join-Path $PackageDir $dir.Name)
    }
}

function New-ZipFromDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$SourceDir,
        [Parameter(Mandatory = $true)][string]$ZipPath
    )

    if (Test-Path -LiteralPath $ZipPath) {
        Remove-Item -LiteralPath $ZipPath -Force
    }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $SourceDir,
        $ZipPath,
        [System.IO.Compression.CompressionLevel]::Optimal,
        $true
    )
}

function Find-MakeNsis {
    $cmd = Get-Command makensis -ErrorAction SilentlyContinue
    if ($cmd) {
        return $cmd.Source
    }

    $candidates = @(
        (Join-Path $Root ".tools\nsis-$NsisVersion\makensis.exe"),
        "${env:ProgramFiles(x86)}\NSIS\makensis.exe",
        "${env:ProgramFiles}\NSIS\makensis.exe"
    )
    foreach ($path in $candidates) {
        if ($path -and (Test-Path -LiteralPath $path)) {
            return $path
        }
    }
    return $null
}

function Test-ZipFile {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return $false
    }
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    return ($bytes.Length -ge 4 -and $bytes[0] -eq 0x50 -and $bytes[1] -eq 0x4B)
}

function Ensure-MakeNsis {
    $existing = Find-MakeNsis
    if ($existing) {
        return $existing
    }

    $toolsDir = Join-Path $Root ".tools"
    $zipPath = Join-Path $toolsDir "nsis-$NsisVersion.zip"
    $extractDir = Join-Path $toolsDir "nsis-$NsisVersion"

    New-Item -ItemType Directory -Path $toolsDir -Force | Out-Null
    if (-not (Test-ZipFile -Path $zipPath)) {
        Write-Host "==> downloading NSIS $NsisVersion"
        if (Test-Path -LiteralPath $zipPath) {
            Remove-Item -LiteralPath $zipPath -Force
        }
        & curl.exe -fsSL -o $zipPath $NsisUrl
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to download NSIS from $NsisUrl (exit $LASTEXITCODE)"
        }
        if (-not (Test-ZipFile -Path $zipPath)) {
            throw "Downloaded NSIS archive is not a valid zip: $zipPath"
        }
    }

    if (Test-Path -LiteralPath $extractDir) {
        Remove-Item -LiteralPath $extractDir -Recurse -Force
    }
    Expand-Archive -LiteralPath $zipPath -DestinationPath $toolsDir -Force

    $makensis = Join-Path $extractDir "makensis.exe"
    if (-not (Test-Path -LiteralPath $makensis)) {
        throw "NSIS extract succeeded but makensis.exe was not found at $makensis"
    }
    return $makensis
}

function New-NsisInstaller {
    param(
        [Parameter(Mandatory = $true)][string]$MakeNsis,
        [Parameter(Mandatory = $true)][string]$ArchName,
        [Parameter(Mandatory = $true)][string]$Version,
        [Parameter(Mandatory = $true)][string]$PackageDir,
        [Parameter(Mandatory = $true)][string]$OutFile,
        [Parameter(Mandatory = $true)][string]$WorkDir
    )

    $templatePath = Join-Path $Root "packaging\installer.nsi"
    if (-not (Test-Path -LiteralPath $templatePath)) {
        throw "Missing NSIS template: $templatePath"
    }

    $iconFile = Join-Path $Root "assets\app-icon.ico"
    if ($ArchName -eq "x86") {
        $installDir = "`$PROGRAMFILES\$ProductName"
        $regView = ""
        $regViewOnInit = ""
    }
    else {
        $installDir = "`$PROGRAMFILES64\$ProductName"
        $regView = "SetRegView 64"
        $regViewOnInit = @"
Function .onInit
  SetRegView 64
FunctionEnd
"@
    }

    $nsiPath = Join-Path $WorkDir "installer-$ArchName.nsi"
    $rendered = Get-Content -LiteralPath $templatePath -Raw -Encoding UTF8
    $replacements = [ordered]@{
        "{{PRODUCT_NAME}}"  = $ProductName
        "{{VERSION}}"       = $Version
        "{{PUBLISHER}}"     = $Publisher
        "{{ARCH}}"          = $ArchName
        "{{OUT_FILE}}"      = ($OutFile -replace '\\', '/')
        "{{SOURCE_DIR}}"    = ($PackageDir -replace '\\', '/')
        "{{ICON_FILE}}"     = ($iconFile -replace '\\', '/')
        "{{INSTALL_DIR}}"   = $installDir
        "{{INIT_REG_VIEW}}" = $regViewOnInit
        "{{SET_REG_VIEW}}"  = $regView
    }
    foreach ($key in $replacements.Keys) {
        $rendered = $rendered.Replace($key, [string]$replacements[$key])
    }

    # NSIS expects ANSI/UTF-8 without BOM for reliable parsing of paths.
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($nsiPath, $rendered, $utf8NoBom)

    Write-Host "==> makensis ($ArchName)"
    & $MakeNsis /V2 $nsiPath
    if ($LASTEXITCODE -ne 0) {
        throw "makensis failed for $ArchName with exit code $LASTEXITCODE"
    }
    if (-not (Test-Path -LiteralPath $OutFile)) {
        throw "Installer was not created: $OutFile"
    }
}

# --- main --------------------------------------------------------------------

$Version = Get-CargoVersion
$VersionDir = Join-Path $Root (Join-Path $OutDir $Version)
$StageRoot = Join-Path $Root (Join-Path $OutDir ".staging\$Version")
$WorkDir = Join-Path $StageRoot "_work"

Write-Host "==> release $Version"
Write-Host "    arches : $($Arch -join ', ')"
Write-Host "    output : $VersionDir"

if (Test-Path -LiteralPath $VersionDir) {
    Remove-Item -LiteralPath $VersionDir -Recurse -Force
}
New-Item -ItemType Directory -Path $VersionDir -Force | Out-Null

if (Test-Path -LiteralPath $StageRoot) {
    Remove-Item -LiteralPath $StageRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null

$makeNsis = $null
if (-not $SkipInstaller) {
    $makeNsis = Ensure-MakeNsis
    Write-Host "    nsis   : $makeNsis"
}

$artifacts = @()

Push-Location $Root
try {
    foreach ($archName in $Arch) {
        $triple = $TargetMap[$archName]
        if (-not $triple) {
            throw "Unknown architecture: $archName"
        }

        Write-Host ""
        Write-Host "======== $archName ($triple) ========"
        Ensure-RustTarget -Triple $triple
        Clear-WasMsixExtractCache

        Write-Host "==> cargo build --release --target $triple"
        & cargo build --release --target $triple --locked
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed for $triple with exit code $LASTEXITCODE"
        }

        $releaseDir = Join-Path $Root "target\$triple\release"
        $packageName = "codex-minibar-$Version-$archName"
        $packageDir = Join-Path $StageRoot $packageName

        Write-Host "==> packaging portable ($archName)"
        New-PortablePackage -ReleaseDir $releaseDir -PackageDir $packageDir

        if (-not (Test-Path -LiteralPath (Join-Path $packageDir $AppExeName))) {
            throw "Packaged binary missing: $(Join-Path $packageDir $AppExeName)"
        }

        $zipPath = Join-Path $VersionDir "$packageName-portable.zip"
        Write-Host "==> zip $([IO.Path]::GetFileName($zipPath))"
        New-ZipFromDirectory -SourceDir $packageDir -ZipPath $zipPath
        $artifacts += $zipPath

        if (-not $SkipInstaller) {
            $setupPath = Join-Path $VersionDir "$packageName-setup.exe"
            New-NsisInstaller `
                -MakeNsis $makeNsis `
                -ArchName $archName `
                -Version $Version `
                -PackageDir $packageDir `
                -OutFile $setupPath `
                -WorkDir $WorkDir
            $artifacts += $setupPath
        }
    }
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "Cleaning staging..."
Remove-Item -LiteralPath $StageRoot -Recurse -Force -ErrorAction SilentlyContinue

# Leave the shared WAS extract cache empty so the next host `cargo run` does
# not pick up the last cross-arch runtime DLLs.
Clear-WasMsixExtractCache

Write-Host ""
Write-Host "Done."
Write-Host "  Version : $Version"
Write-Host "  Output  : $VersionDir"
foreach ($path in $artifacts) {
    $item = Get-Item -LiteralPath $path
    $sizeMb = [math]::Round($item.Length / 1MB, 2)
    Write-Host ("  - {0} ({1} MB)" -f $item.Name, $sizeMb)
}
