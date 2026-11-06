# Builds target\dist\OBCR-Setup-<version>-windows-x64.exe: one self-extracting
# file holding OBCR Setup (Qt 6), its Qt runtime and the release game.
#
#   powershell -ExecutionPolicy Bypass -File installer\build-windows.ps1
#
# Needs Rust, CMake and a Qt 6 MinGW kit (the defaults match the Qt online
# installer's layout; override with -Qt, -MinGW and -Ninja).
# SPDX-License-Identifier: GPL-3.0-or-later
param(
    [string]$Qt = "C:\Qt\6.11.1\mingw_64",
    [string]$MinGW = "C:\Qt\Tools\mingw1310_64",
    [string]$Ninja = "C:\Qt\Tools\Ninja"
)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path "$PSScriptRoot\..").Path
Set-Location $root

function Invoke-Checked([string]$what, [scriptblock]$block) {
    Write-Host "== $what"
    & $block
    if ($LASTEXITCODE -ne 0) { throw "$what failed (exit code $LASTEXITCODE)" }
}

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$env:PATH = "$MinGW\bin;$Ninja;$Qt\bin;$env:PATH"
$build = "target\installer\windows"
$deploy = "target\installer\windows-deploy"
$dist = "target\dist"
$out = "$dist\OBCR-Setup-$version-windows-x64.exe"

Invoke-Checked "cargo build" { cargo build --release -p obcr -p obcr-setup }
Invoke-Checked "configure Setup" {
    cmake -S installer -B $build -G Ninja "-DCMAKE_BUILD_TYPE=Release" "-DCMAKE_PREFIX_PATH=$Qt" `
        "-DOBCR_BINARY=$root\target\release\obcr.exe" "-DOBCR_VERSION=$version"
}
Invoke-Checked "build Setup" { cmake --build $build }

Write-Host "== deploy the Qt runtime"
if (Test-Path $deploy) { Remove-Item -Recurse -Force $deploy }
New-Item -ItemType Directory -Force $deploy, $dist | Out-Null
Copy-Item "$build\OBCR-Setup.exe" $deploy
Invoke-Checked "strip" { & "$MinGW\bin\strip.exe" "$deploy\OBCR-Setup.exe" }
Invoke-Checked "windeployqt" {
    windeployqt --release --no-translations --no-system-d3d-compiler --no-system-dxc-compiler `
        --no-opengl-sw --no-network `
        --skip-plugin-types "generic,networkinformation,tls,iconengines,imageformats" `
        "$deploy\OBCR-Setup.exe"
}
# The uninstaller copy needs exactly these files.
$files = Get-ChildItem $deploy -Recurse -File | ForEach-Object {
    $_.FullName.Substring((Resolve-Path $deploy).Path.Length + 1).Replace('\', '/')
} | Where-Object { $_ -ne "OBCR-Setup.exe" }
$list = @("# Runtime files of OBCR Setup, copied with the uninstaller.") + $files + @("setup-runtime.txt")
Set-Content -Path "$deploy\setup-runtime.txt" -Value $list -Encoding utf8

Invoke-Checked "pack" { cargo xtask pack-installer target\release\obcr-setup.exe $deploy $out }
$size = [math]::Round((Get-Item $out).Length / 1MB, 1)
Write-Host "== done: $out ($size MB)"
