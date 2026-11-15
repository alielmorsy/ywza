# Installs the latest ywza release:
# irm https://raw.githubusercontent.com/alielmorsy/ywza/main/scripts/install.ps1 | iex
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$repo = 'alielmorsy/ywza'
$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest" -Headers @{ 'User-Agent' = 'ywza-install' }
$asset = $release.assets | Where-Object name -like '*.msi' | Select-Object -First 1
if (-not $asset) { throw "No Windows installer found in release $($release.tag_name)" }

$file = Join-Path $env:TEMP $asset.name
Write-Host "Downloading $($asset.name) ($($release.tag_name))"
Invoke-WebRequest $asset.browser_download_url -OutFile $file

Write-Host 'Installing (accept the admin prompt)'
try {
    $p = Start-Process msiexec.exe -ArgumentList "/i `"$file`" /qn /norestart" -Verb RunAs -Wait -PassThru
    # 3010: installed, a restart is pending.
    if ($p.ExitCode -notin 0, 3010) { throw "Installer exited with code $($p.ExitCode)" }
} finally {
    Remove-Item $file -Force -ErrorAction SilentlyContinue
}
Write-Host 'ywza installed.'
