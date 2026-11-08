# Installs the latest ywza release: irm https://<host>/install.ps1 | iex
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$repo = 'alielmorsy/ywza'
$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest" -Headers @{ 'User-Agent' = 'ywza-install' }
$asset = $release.assets | Where-Object name -like '*-setup.exe' | Select-Object -First 1
if (-not $asset) { throw "No Windows installer found in release $($release.tag_name)" }

$file = Join-Path $env:TEMP $asset.name
Write-Host "Downloading $($asset.name) ($($release.tag_name))"
Invoke-WebRequest $asset.browser_download_url -OutFile $file

Write-Host 'Installing (accept the admin prompt)'
try {
    $p = Start-Process $file -ArgumentList '/S' -Verb RunAs -Wait -PassThru
    if ($p.ExitCode -ne 0) { throw "Installer exited with code $($p.ExitCode)" }
} finally {
    Remove-Item $file -Force -ErrorAction SilentlyContinue
}
Write-Host 'ywza installed.'
