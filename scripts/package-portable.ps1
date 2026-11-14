<#
  Builds the Shuusshy portable ZIP from the release executable.

  Run after `npm run tauri build` (which builds src-tauri\target\release\shuusshy.exe):
      powershell -ExecutionPolicy Bypass -File scripts\package-portable.ps1

  The ZIP contains shuusshy.exe and the `shuusshy.portable` marker. With the marker next to
  the executable, Shuusshy keeps its settings and WebView2 cache in a `data` folder beside it
  and writes nothing to %APPDATA% / %LOCALAPPDATA%.
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$conf = Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$version = $conf.version
$exe = Join-Path $root "src-tauri\target\release\shuusshy.exe"
if (-not (Test-Path $exe)) { throw "Release build not found: $exe (run 'npm run tauri build' first)" }

$out = Join-Path $root "src-tauri\target\release\bundle\portable"
$stage = Join-Path $out "Shuusshy"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null

Copy-Item $exe (Join-Path $stage "shuusshy.exe")
Copy-Item (Join-Path $root "THIRD_PARTY_LICENSES.md") $stage

Set-Content -Encoding utf8 (Join-Path $stage "shuusshy.portable") @"
This file switches Shuusshy to portable mode: settings are kept in the "data" folder next to
shuusshy.exe. Delete this file to use the normal per-user settings location instead.
"@

Set-Content -Encoding utf8 (Join-Path $stage "README-PORTABLE.txt") @"
Shuusshy $version - portable edition
Less Noise. More Voice.

1. Extract this folder anywhere you can write to (for example a USB drive or Documents).
   Do not run it from inside the ZIP, and avoid C:\Program Files (not writable).
2. Run shuusshy.exe. Settings are stored in the "data" folder next to it.
3. Shuusshy needs the free VB-CABLE virtual audio driver (installed separately, once):
   https://vb-audio.com/Cable/  -  Settings > Audio Output shows the steps.
4. Requires Windows 10 or 11 (64-bit) with the Microsoft Edge WebView2 Runtime, which
   Windows 10/11 normally include.

To remove: exit Shuusshy from its tray icon, turn off "Start with Windows" first if you
enabled it, then delete the folder. Nothing else is installed.
"@

$zip = Join-Path $out "Shuusshy_${version}_x64_portable.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path $stage -DestinationPath $zip
$hash = (Get-FileHash -Algorithm SHA256 $zip).Hash.ToLower()
Set-Content -Encoding ascii "$zip.sha256" "$hash  $(Split-Path -Leaf $zip)"
Write-Host "Portable ZIP: $zip"
Write-Host "SHA-256:      $hash"
