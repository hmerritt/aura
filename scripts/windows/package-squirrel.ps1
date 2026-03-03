param(
    [Parameter(Mandatory = $true)]
    [string]$Version,
    [string]$BinaryPath = "target/release/aura.exe",
    [string]$OutputDir = "dist/squirrel",
    [string]$NuGetExe = "nuget",
    [string]$SquirrelExe = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
}

function Resolve-NuGetCommand {
    param([string]$CommandName)
    $nuget = Get-Command $CommandName -ErrorAction SilentlyContinue
    if (-not $nuget) {
        throw "NuGet command '$CommandName' was not found. Install NuGet.CommandLine first."
    }
    return $nuget.Source
}

function Resolve-SquirrelExecutable {
    param(
        [string]$ProvidedPath,
        [string]$NuGetPath,
        [string]$ToolsDir
    )

    if ($ProvidedPath) {
        if (-not (Test-Path -LiteralPath $ProvidedPath)) {
            throw "Provided Squirrel executable does not exist: $ProvidedPath"
        }
        return (Resolve-Path -LiteralPath $ProvidedPath).Path
    }

    $fromPath = Get-Command "Squirrel.exe" -ErrorAction SilentlyContinue
    if ($fromPath) {
        return $fromPath.Source
    }

    New-Item -ItemType Directory -Path $ToolsDir -Force | Out-Null
    
    # Suppress the standard output to prevent pipeline pollution
    & $NuGetPath install Squirrel.Windows -OutputDirectory $ToolsDir -ExcludeVersion -NonInteractive | Out-Null
    
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to install Squirrel.Windows via NuGet."
    }

    $candidate = Join-Path $ToolsDir "Squirrel.Windows\tools\Squirrel.exe"
    if (-not (Test-Path -LiteralPath $candidate)) {
        throw "Unable to locate Squirrel.exe after installing Squirrel.Windows package."
    }

    return $candidate
}

$repoRoot = Resolve-RepoRoot
$binaryFullPath = Join-Path $repoRoot $BinaryPath
$outputFullPath = Join-Path $repoRoot $OutputDir
$workRoot = Join-Path $repoRoot "dist\squirrel-work"
$inputDir = Join-Path $workRoot "input"
$pkgDir = Join-Path $workRoot "pkg"
$toolsDir = Join-Path $workRoot "tools"
$nuspecPath = Join-Path $repoRoot "packaging\windows\squirrel\aura.nuspec"
$packageIconSourcePath = Join-Path $repoRoot "assets\tray.png"

if (-not (Test-Path -LiteralPath $binaryFullPath)) {
    throw "Binary does not exist: $binaryFullPath"
}

if (-not (Test-Path -LiteralPath $nuspecPath)) {
    throw "Nuspec does not exist: $nuspecPath"
}
if (-not (Test-Path -LiteralPath $packageIconSourcePath)) {
    throw "Package icon source does not exist: $packageIconSourcePath"
}

# Execute cleanup prior to resolving and downloading executables
if (Test-Path -LiteralPath $workRoot) {
    Remove-Item -LiteralPath $workRoot -Recurse -Force
}
if (Test-Path -LiteralPath $outputFullPath) {
    Remove-Item -LiteralPath $outputFullPath -Recurse -Force
}

# Recreate required directories
New-Item -ItemType Directory -Path $inputDir -Force | Out-Null
New-Item -ItemType Directory -Path $pkgDir -Force | Out-Null
New-Item -ItemType Directory -Path $outputFullPath -Force | Out-Null

$nugetPath = Resolve-NuGetCommand -CommandName $NuGetExe
$squirrelPath = Resolve-SquirrelExecutable -ProvidedPath $SquirrelExe -NuGetPath $nugetPath -ToolsDir $toolsDir

Copy-Item -LiteralPath $binaryFullPath -Destination (Join-Path $inputDir "aura.exe") -Force
Copy-Item -LiteralPath $packageIconSourcePath -Destination (Join-Path $inputDir "tray.png") -Force

& $nugetPath pack $nuspecPath -Version $Version -BasePath $inputDir -OutputDirectory $pkgDir -NoPackageAnalysis -NonInteractive
if ($LASTEXITCODE -ne 0) {
    throw "NuGet pack failed."
}

$nupkgPath = Join-Path $pkgDir ("aura.{0}.nupkg" -f $Version)
if (-not (Test-Path -LiteralPath $nupkgPath)) {
    $candidatePackage = Get-ChildItem -LiteralPath $pkgDir -Filter "*.nupkg" | Select-Object -First 1
    if ($candidatePackage) {
        $nupkgPath = $candidatePackage.FullName
    }
    else {
        $nupkgPath = ""
    }
}
if (-not $nupkgPath) {
    throw "No NuGet package was generated."
}

& $squirrelPath --releasify $nupkgPath --releaseDir $outputFullPath --no-msi
if ($LASTEXITCODE -ne 0) {
    throw "Squirrel releasify failed."
}

$setupPath = Join-Path $outputFullPath "Setup.exe"

# Polling loop to mitigate file system / Antivirus locking race conditions
$maxRetries = 10
$retryCount = 0
$setupExists = $false

while (-not $setupExists -and $retryCount -lt $maxRetries) {
    if (Test-Path -LiteralPath $setupPath) {
        $setupExists = $true
    }
    else {
        Start-Sleep -Milliseconds 500
        $retryCount++
    }
}

if (-not $setupExists) {
    throw "Squirrel setup executable was not found. It may not have generated, or it remains locked by an external process."
}

$versionedSetup = Join-Path $outputFullPath ("aura-{0}-installer.exe" -f $Version)
Copy-Item -LiteralPath $setupPath -Destination $versionedSetup -Force

Write-Host "Squirrel packaging complete."
Write-Host "Output directory: $outputFullPath"
