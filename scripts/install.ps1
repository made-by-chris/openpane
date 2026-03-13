param(
  [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"

$repo = "made-by-chris/openpane"

if ($Version -eq "latest") {
  $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest"
  if (-not $release.tag_name) {
    throw "Unable to determine the latest openpane release."
  }
  $Version = $release.tag_name.TrimStart("v")
}

$asset = "openpane-x86_64-pc-windows-msvc.zip"
$archiveUrl = "https://github.com/$repo/releases/download/v$Version/$asset"
$installRoot = Join-Path $env:LOCALAPPDATA "openpane"
$versionDir = Join-Path $installRoot $Version
$binDir = Join-Path $env:USERPROFILE ".openpane\bin"
$tempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("openpane-" + [guid]::NewGuid().ToString("N"))
$zipPath = Join-Path $tempDir "openpane.zip"

New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
New-Item -ItemType Directory -Force -Path $installRoot | Out-Null
New-Item -ItemType Directory -Force -Path $binDir | Out-Null

try {
  Write-Host "Downloading openpane v$Version..."
  Invoke-WebRequest -Uri $archiveUrl -OutFile $zipPath

  if (Test-Path $versionDir) {
    Remove-Item -Recurse -Force $versionDir
  }

  New-Item -ItemType Directory -Force -Path $versionDir | Out-Null
  Expand-Archive -LiteralPath $zipPath -DestinationPath $versionDir -Force

  foreach ($name in @("openpane", "grid", "codegrid")) {
    $cmdPath = Join-Path $binDir "$name.cmd"
    @(
      "@echo off",
      "`"$versionDir\openpane.exe`" %*"
    ) | Set-Content -Path $cmdPath -Encoding ASCII
  }
}
finally {
  if (Test-Path $tempDir) {
    Remove-Item -Recurse -Force $tempDir
  }
}

Write-Host ""
Write-Host "Installed openpane to $versionDir"
Write-Host "Command shims created in $binDir"

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not $userPath) {
  $userPath = ""
}

if (($userPath -split ';') -contains $binDir) {
  Write-Host "You can run: openpane 2 2 claude"
} else {
  Write-Host "Add this directory to your user PATH, then open a new terminal:"
  Write-Host "  $binDir"
}
