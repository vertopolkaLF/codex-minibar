; Codex Minibar NSIS installer template. Placeholders are filled by build.ps1.

Unicode true
ManifestDPIAware true

!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"

Name "{{PRODUCT_NAME}}"
BrandingText "{{PRODUCT_NAME}} {{VERSION}}"
OutFile "{{OUT_FILE}}"
InstallDir "{{INSTALL_DIR}}"
RequestExecutionLevel user
SetCompressor /SOLID lzma

VIProductVersion "{{VERSION}}.0"
VIAddVersionKey "ProductName" "{{PRODUCT_NAME}}"
VIAddVersionKey "CompanyName" "{{PUBLISHER}}"
VIAddVersionKey "FileDescription" "{{PRODUCT_NAME}} Setup ({{ARCH}})"
VIAddVersionKey "FileVersion" "{{VERSION}}"
VIAddVersionKey "ProductVersion" "{{VERSION}}"
VIAddVersionKey "LegalCopyright" "Copyright (C) {{PUBLISHER}}"

!define MUI_ABORTWARNING
!define MUI_ICON "{{ICON_FILE}}"
!define MUI_UNICON "{{ICON_FILE}}"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES

; Finish page: both checkboxes checked by default (do not set *_NOTCHECKED).
; Custom leave runs BEFORE MUI launches the app so the Run key exists first.
; The app then reconciles start_at_login from that key (NSIS must not rewrite
; UTF-8 settings.toml — Unicode FileWrite would corrupt it).
!define MUI_FINISHPAGE_RUN "$INSTDIR\codex-minibar.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch Codex Minibar"
!define MUI_FINISHPAGE_SHOWREADME ""
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Add to Startup"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION StartupCheckboxNoop
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE FinishLeave
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

{{INIT_REG_VIEW}}

!macro KillRunningAppBody
  ; Ignore exit code — the process may not be running.
  ClearErrors
  ExecWait 'taskkill /F /IM "codex-minibar.exe" /T' $0
  Sleep 500
!macroend

Function KillRunningApp
  !insertmacro KillRunningAppBody
FunctionEnd

Function un.KillRunningApp
  !insertmacro KillRunningAppBody
FunctionEnd

Function .onInit
  Call KillRunningApp
FunctionEnd

Function un.onInit
  Call un.KillRunningApp
FunctionEnd

Function StartupCheckboxNoop
FunctionEnd

; $R0 must be "true" or "false".
Function SyncStartAtLogin
  ${If} $R0 == "true"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" \
      "Codex Minibar" '"$INSTDIR\codex-minibar.exe"'
  ${Else}
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Codex Minibar"
  ${EndIf}
FunctionEnd

Function FinishLeave
  ${NSD_GetState} $mui.FinishPage.ShowReadme $0
  ${If} $0 == ${BST_CHECKED}
    StrCpy $R0 "true"
  ${Else}
    StrCpy $R0 "false"
  ${EndIf}
  Call SyncStartAtLogin
FunctionEnd

Section "Install"
  {{SET_REG_VIEW}}
  SetOutPath "$INSTDIR"
  File /r "{{SOURCE_DIR}}\*.*"

  CreateDirectory "$SMPROGRAMS\{{PRODUCT_NAME}}"
  CreateShortCut "$SMPROGRAMS\{{PRODUCT_NAME}}\{{PRODUCT_NAME}}.lnk" "$INSTDIR\codex-minibar.exe"
  CreateShortCut "$DESKTOP\{{PRODUCT_NAME}}.lnk" "$INSTDIR\codex-minibar.exe"

  WriteUninstaller "$INSTDIR\Uninstall.exe"

  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}" \
    "DisplayName" "{{PRODUCT_NAME}} ({{ARCH}})"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}" \
    "DisplayVersion" "{{VERSION}}"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}" \
    "Publisher" "{{PUBLISHER}}"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}" \
    "DisplayIcon" "$INSTDIR\codex-minibar.exe"
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}" \
    "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}" \
    "InstallLocation" "$INSTDIR"
  WriteRegDWORD SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}" \
    "NoModify" 1
  WriteRegDWORD SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}" \
    "NoRepair" 1
SectionEnd

Section "Uninstall"
  {{SET_REG_VIEW}}
  Delete "$DESKTOP\{{PRODUCT_NAME}}.lnk"
  Delete "$SMPROGRAMS\{{PRODUCT_NAME}}\{{PRODUCT_NAME}}.lnk"
  RMDir "$SMPROGRAMS\{{PRODUCT_NAME}}"

  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Codex Minibar"
  DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}"

  RMDir /r "$INSTDIR"
SectionEnd
