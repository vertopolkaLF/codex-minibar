; Codex Minibar NSIS installer template. Placeholders are filled by build.ps1.

Unicode true
ManifestDPIAware true

!include "MUI2.nsh"
!include "x64.nsh"

Name "{{PRODUCT_NAME}}"
BrandingText "{{PRODUCT_NAME}} {{VERSION}}"
OutFile "{{OUT_FILE}}"
InstallDir "{{INSTALL_DIR}}"
RequestExecutionLevel admin
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
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

{{INIT_REG_VIEW}}

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

  DeleteRegKey SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\{{PRODUCT_NAME}} {{ARCH}}"

  RMDir /r "$INSTDIR"
SectionEnd
