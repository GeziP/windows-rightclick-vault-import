!include LogicLib.nsh
!include WinMessages.nsh

!define APP_NAME "KBIntake"
!define APP_PUBLISHER "GeziP"
!define APP_EXE "kbintake.exe"
!define APP_GUI_EXE "kbintakew.exe"
!define APP_ICON "kbintake.ico"
!define APP_VERSION "1.0.1"
!define POWERSHELL_EXE "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"

Name "${APP_NAME}"
OutFile "..\dist\KBIntake-Setup.exe"
InstallDir "$LOCALAPPDATA\Programs\kbintake"
RequestExecutionLevel user
Unicode true

VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "LegalCopyright" "Copyright 2026 ${APP_PUBLISHER}"
VIAddVersionKey "FileDescription" "${APP_NAME} Setup"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"

Section "Install"
  SetOutPath "$INSTDIR"
  File "..\dist\${APP_EXE}"
  File "..\dist\${APP_GUI_EXE}"
  File "..\dist\${APP_ICON}"

  ExecWait '"$INSTDIR\${APP_EXE}" doctor' $0
  ${If} $0 != 0
    DetailPrint "doctor returned $0; continuing installation so the user can repair configuration later."
  ${EndIf}

  ExecWait '"$INSTDIR\${APP_EXE}" explorer install --exe-path "$INSTDIR\${APP_GUI_EXE}" --icon-path "$INSTDIR\${APP_ICON}"' $0
  ${If} $0 != 0
    DetailPrint "Explorer registration returned $0; context menus can be registered later with kbintake explorer install."
  ${EndIf}

  Call AddInstallDirToPath

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "DisplayIcon" "$INSTDIR\${APP_ICON}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  IfFileExists "$INSTDIR\${APP_EXE}" 0 +2
    ExecWait '"$INSTDIR\${APP_EXE}" explorer uninstall'

  Call un.RemoveInstallDirFromPath

  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\${APP_GUI_EXE}"
  Delete "$INSTDIR\${APP_ICON}"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_NAME}"
SectionEnd

Function AddInstallDirToPath
  ExecWait "$\"${POWERSHELL_EXE}$\" -NoProfile -ExecutionPolicy Bypass -Command $\"$$dir = '$INSTDIR'; $$path = [Environment]::GetEnvironmentVariable('Path', 'User'); $$parts = @($$path -split ';' | Where-Object { $$_ }); if ($$parts -notcontains $$dir) { $$parts += $$dir; [Environment]::SetEnvironmentVariable('Path', ($$parts -join ';'), 'User') }$\""
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
FunctionEnd

Function un.RemoveInstallDirFromPath
  ExecWait "$\"${POWERSHELL_EXE}$\" -NoProfile -ExecutionPolicy Bypass -Command $\"$$dir = '$INSTDIR'; $$path = [Environment]::GetEnvironmentVariable('Path', 'User'); $$parts = @($$path -split ';' | Where-Object { $$_ -and $$_ -ne $$dir }); [Environment]::SetEnvironmentVariable('Path', ($$parts -join ';'), 'User')$\""
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
FunctionEnd
