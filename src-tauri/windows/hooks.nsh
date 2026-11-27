; Installer hooks, wired in tauri.conf.json (bundle.windows.nsis.installerHooks).

!macro NSIS_HOOK_POSTINSTALL
  ; Make Explorer and the taskbar drop their cached icons, so an update shows the new Solace icon.
  nsExec::Exec '"$SYSDIR\ie4uinit.exe" -show'
  ; SHCNE_ASSOCCHANGED, SHCNF_IDLIST
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Tauri's template sets $UpdateMode from the /UPDATE flag when the updater replaces the old
  ; version; only a real uninstall should take Solace's hooks and skill out of the agents' configs.
  ${If} $UpdateMode <> 1
    nsExec::Exec '"$INSTDIR\bin\solace.exe" integration remove'
    Pop $0
  ${EndIf}
!macroend
