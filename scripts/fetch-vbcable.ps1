<#
  Fetches the official VB-CABLE package (VB-Audio Software) for the Windows installer.

      powershell -ExecutionPolicy Bypass -File scripts\fetch-vbcable.ps1

  The package is shipped AS IS (unmodified), as VB-Audio's terms require
  (https://vb-audio.com/Services/licensing.htm). It is not committed to the repository: this
  script downloads it from VB-Audio's own server, refuses it unless the SHA-256 matches the
  pinned value and VBCABLE_Setup_x64.exe carries a valid Authenticode signature from
  VB-Audio's author, and unpacks it to src-tauri\vbcable (bundled by tauri.windows.conf.json).

  VB-CABLE is a donationware, all participations are welcome: www.vb-cable.com
  To move to a newer VB-CABLE release, update $Url and $Sha256 after checking the new package.
#>
$ErrorActionPreference = "Stop"
$Url = "https://download.vb-audio.com/Download_CABLE/VBCABLE_Driver_Pack46.zip"
$Sha256 = "BC44E8974191A5A2427D6C979E2DAB08DBB09925F3F435DFA2F0BA3E45B6EB90"
$Signer = "BUREL VINCENT"

$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "src-tauri\vbcable"
$setup = Join-Path $dest "VBCABLE_Setup_x64.exe"

if (Test-Path $setup) {
    Write-Host "VB-CABLE package already present: $dest"
    exit 0
}

$tmp = Join-Path ([IO.Path]::GetTempPath()) ("vbcable-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force $tmp | Out-Null
try {
    $zip = Join-Path $tmp "VBCABLE_Driver_Pack.zip"
    Write-Host "Downloading $Url"
    Invoke-WebRequest -Uri $Url -OutFile $zip -UseBasicParsing
    $hash = (Get-FileHash -Algorithm SHA256 $zip).Hash
    if ($hash -ne $Sha256) {
        throw "VB-CABLE package checksum mismatch: expected $Sha256, got $hash. Not using it."
    }
    $unpacked = Join-Path $tmp "x"
    Expand-Archive -Path $zip -DestinationPath $unpacked
    $sig = Get-AuthenticodeSignature (Join-Path $unpacked "VBCABLE_Setup_x64.exe")
    if ($sig.Status -ne "Valid" -or $sig.SignerCertificate.Subject -notmatch $Signer) {
        throw "VBCABLE_Setup_x64.exe signature is not valid or not from VB-Audio ($($sig.Status))."
    }
    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    Move-Item $unpacked $dest
    Write-Host "VB-CABLE package verified (SHA-256 $hash, signed by $Signer) -> $dest"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
