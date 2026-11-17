; Shuusshy installer hooks (tauri.conf.json > bundle > windows > nsis > installerHooks).
; An upgrade runs the previous uninstaller with /UPDATE ($UpdateMode = 1): both hooks skip
; that case, so upgrades keep "Start with Windows" and the virtual mic name.

!macro NSIS_HOOK_POSTINSTALL
  ; Optional VB-CABLE step. The official VB-CABLE package (VB-Audio Software, shipped as is in
  ; $INSTDIR\vbcable, see scripts\fetch-vbcable.ps1) is offered when the driver is not
  ; installed yet. Nothing is installed silently: the user confirms here, then VB-Audio's own
  ; setup window opens (with Windows' administrator prompt) and the user clicks "Install
  ; Driver" there. Skipped for updates, silent and passive installs, and when VB-CABLE's driver
  ; service already exists.
  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
  ${AndIf} ${FileExists} "$INSTDIR\vbcable\VBCABLE_Setup_x64.exe"
    ClearErrors
    ReadRegStr $0 HKLM "SYSTEM\CurrentControlSet\Services\VBAudioVACMME" "ImagePath"
    ${If} ${Errors}
      MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON1 \
        "Shuusshy needs a free virtual audio cable to pass your clean voice to other apps.$\n$\n\
Install VB-CABLE now (recommended)?$\n$\n\
VB-CABLE is made by VB-Audio Software (www.vb-cable.com). VB-CABLE is a donationware, all participations are welcome.$\n$\n\
VB-Audio's installer opens next and Windows asks for administrator permission. Click $\"Install Driver$\" there, then restart Windows when it asks. You can also install it later from Shuusshy's settings." \
        IDNO vbcable_skip
      ExecShellWait "runas" "$INSTDIR\vbcable\VBCABLE_Setup_x64.exe"
      vbcable_skip:
    ${EndIf}
  ${EndIf}
!macroend

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
