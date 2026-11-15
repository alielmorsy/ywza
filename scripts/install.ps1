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

# The older NSIS build registers the same service and folder; it has to go before the MSI runs.
$steps = @()
$nsis = Get-ItemProperty 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*' -ErrorAction SilentlyContinue |
    Where-Object { $_.DisplayName -eq 'ywza' -and $_.UninstallString -like '*uninstall.exe*' } | Select-Object -First 1
if ($nsis) {
    $dir = $nsis.InstallLocation.Trim('"')
    $steps += "Start-Process '$dir\uninstall.exe' -ArgumentList '/S','_?=$dir' -Wait"
}
$steps += "exit (Start-Process msiexec.exe -ArgumentList '/i','`"$file`"','/qn','/norestart' -Wait -PassThru).ExitCode"
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($steps -join '; '))

Write-Host 'Installing (accept the admin prompt)'
try {
    $p = Start-Process powershell.exe -ArgumentList '-NoProfile', '-EncodedCommand', $encoded -Verb RunAs -Wait -PassThru
    # 3010: installed, a restart is pending.
    if ($p.ExitCode -notin 0, 3010) { throw "Installer exited with code $($p.ExitCode)" }
} finally {
    Remove-Item $file -Force -ErrorAction SilentlyContinue
}
Write-Host 'ywza installed.'
