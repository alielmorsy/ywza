@echo off
rem Usage: windows-firewall.bat <path-to-ywza-agent.exe> | /remove
net session >nul 2>&1
if errorlevel 1 (
    echo Error: run this from an elevated prompt ^(Run as administrator^).
    exit /b 1
)
if "%~1"=="" goto usage
if /i "%~1"=="/remove" (
    netsh advfirewall firewall delete rule name="ywza-agent"
    exit /b 0
)
rem Block rules from the Windows prompt override Allow rules.
powershell -NoProfile -Command "Get-NetFirewallApplicationFilter | Where-Object { $_.Program -eq '%~f1' } | Get-NetFirewallRule | Where-Object { $_.Action -eq 'Block' } | Remove-NetFirewallRule"
netsh advfirewall firewall delete rule name="ywza-agent" >nul 2>&1
netsh advfirewall firewall add rule name="ywza-agent" dir=in action=allow protocol=UDP program="%~f1" profile=private,public
exit /b %errorlevel%

:usage
echo Error: missing exe path.
echo Usage: %~nx0 ^<path-to-ywza-agent.exe^>
echo        %~nx0 /remove
exit /b 1
