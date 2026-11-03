; Allow DropLink's listener through Windows Defender Firewall on private and
; domain networks (never on public networks). Runs only when the installer
; has administrator rights; otherwise Windows asks on first launch.

!macro NSIS_HOOK_POSTINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="DropLink"'
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="DropLink" dir=in action=allow program="$INSTDIR\DropLink.exe" enable=yes profile=private,domain'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="DropLink"'
!macroend
