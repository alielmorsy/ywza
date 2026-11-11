; Installs ywza-agent.exe and the ywza CLI next to the app, registers the agent as the
; "ywza-agent" service (LocalSystem, automatic start, restarted on failure), opens it in the
; firewall and puts the install dir on the machine PATH. Uninstall undoes all of it but keeps
; the identity in %ProgramData%\ywza.

!define RELEASE "${__FILEDIR__}\..\..\..\target\release"
!define SERVICE "ywza-agent"
!define ENV_KEY "SYSTEM\CurrentControlSet\Control\Session Manager\Environment"

; Read and written raw (REG_EXPAND_SZ kept), in PowerShell: NSIS strings would truncate a long PATH.
!macro YWZA_PATH OP
  nsExec::ExecToLog `powershell -NoProfile -NonInteractive -Command "$$k = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey('${ENV_KEY}', $$true); $$d = '$INSTDIR'; $$p = @($$k.GetValue('Path', '', 'DoNotExpandEnvironmentNames') -split ';' | Where-Object { $$_ -and $$_ -ne $$d }); if ('${OP}' -eq 'add') { $$p += $$d }; $$k.SetValue('Path', ($$p -join ';'), 'ExpandString')"`
  ; HWND_BROADCAST, WM_SETTINGCHANGE: new terminals see the change without a sign-out.
  SendMessage 0xFFFF 0x1A 0 "STR:Environment" /TIMEOUT=5000
!macroend

!macro NSIS_HOOK_PREINSTALL
  ; An upgrade replaces the exe: stop the service (and with it every agent) first.
  nsExec::ExecToLog 'net stop ${SERVICE}'
!macroend

!macro NSIS_HOOK_POSTINSTALL
  File "/oname=$INSTDIR\ywza-agent.exe" "${RELEASE}\ywza-agent.exe"
  File "/oname=$INSTDIR\ywza.exe" "${RELEASE}\ywza.exe"
  !insertmacro YWZA_PATH add

  ; Keep the installing user's identity and pairings from a per-user install, once.
  ReadEnvStr $0 PROGRAMDATA
  ReadEnvStr $1 APPDATA
  ${IfNot} ${FileExists} "$0\ywza\agent.key"
  ${AndIf} ${FileExists} "$1\ywza\agent.key"
    CreateDirectory "$0\ywza"
    CopyFiles /SILENT "$1\ywza\*.*" "$0\ywza"
  ${EndIf}

  nsExec::ExecToLog 'sc.exe create ${SERVICE} binPath= "\"$INSTDIR\ywza-agent.exe\" --log-file \"$0\ywza\agent.log\" service" start= auto DisplayName= "ywza agent"'
  nsExec::ExecToLog 'sc.exe config ${SERVICE} binPath= "\"$INSTDIR\ywza-agent.exe\" --log-file \"$0\ywza\agent.log\" service" start= auto'
  nsExec::ExecToLog 'sc.exe description ${SERVICE} "Keeps the ywza agent on the signed-in desktop and the sign-in screen."'
  nsExec::ExecToLog 'sc.exe failure ${SERVICE} reset= 86400 actions= restart/5000/restart/5000/restart/30000'
  nsExec::ExecToLog 'sc.exe failureflag ${SERVICE} 1'

  ; Block rules (from an earlier Windows prompt) beat allow rules.
  nsExec::ExecToLog `powershell -NoProfile -NonInteractive -Command "Get-NetFirewallApplicationFilter -Program '$INSTDIR\ywza-agent.exe' -ErrorAction SilentlyContinue | Get-NetFirewallRule | Where-Object Action -eq Block | Remove-NetFirewallRule"`
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${SERVICE}"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${SERVICE}" dir=in action=allow protocol=UDP program="$INSTDIR\ywza-agent.exe" profile=any'

  nsExec::ExecToLog 'net start ${SERVICE}'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::ExecToLog 'net stop ${SERVICE}'
  nsExec::ExecToLog 'sc.exe delete ${SERVICE}'
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${SERVICE}"'
  Delete "$INSTDIR\ywza-agent.exe"
  Delete "$INSTDIR\ywza.exe"
  !insertmacro YWZA_PATH remove
!macroend
