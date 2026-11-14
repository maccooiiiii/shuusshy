; Shuusshy installer hooks (tauri.conf.json > bundle > windows > nsis > installerHooks).
; An upgrade runs the previous uninstaller with /UPDATE ($UpdateMode = 1): both hooks skip
; that case, so upgrades keep "Start with Windows" and the virtual mic name.

!macro NSIS_HOOK_PREUNINSTALL
  ; Give VB-CABLE's microphone back its own name if Shuusshy renamed it to
  ; "Shuusshy Microphone". The helper does nothing when it was not renamed; otherwise Windows
  ; asks for administrator permission (the user may decline). Not in silent uninstalls.
  ${If} $UpdateMode <> 1
  ${AndIfNot} ${Silent}
    ExecWait '"$INSTDIR\shuusshy.exe" --restore-mic-name'
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Remove the "Start with Windows" entry so nothing points at the removed program.
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Shuusshy"
  ${EndIf}
!macroend
