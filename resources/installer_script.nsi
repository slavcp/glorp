!define Name "glorp"
!define PRODUCT_UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${Name}"
!addplugindir "./plugins"
Name "${Name}"
OutFile "../target/${Name}-silent-setup-x64.exe"
InstallDir "$LOCALAPPDATA\${Name}"

SetCompressor lzma
SilentInstall silent
SilentUnInstall silent

    Section "Install"
    nsis_process::KillProcess "${Name}.exe"
    SetOutPath "$INSTDIR"
    File "..\target\x86_64-pc-windows-msvc\release\${Name}.exe"
    File "..\target\x86_64-pc-windows-msvc\release\render.dll"
    File "..\target\x86_64-pc-windows-msvc\release\webview.dll"

    WriteUninstaller "$INSTDIR\uninstall.exe"

    WriteRegStr HKCU "Software\${Name}" "Install_Dir" "$INSTDIR" ;
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayName" "${Name}"
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "UninstallString" "$INSTDIR/uninstall.exe"
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "DisplayIcon" "$INSTDIR/${Name}.exe,0"
    WriteRegStr HKCU "${PRODUCT_UNINST_KEY}" "Publisher" "slav"

    CreateShortCut "$SMPROGRAMS\${Name}.lnk" "$INSTDIR\${Name}.exe"
    CreateShortCut "$DESKTOP\${Name}.lnk" "$INSTDIR\${Name}.exe"
    Sleep 5000 ; wait 5 second for files to be written
    ExecShell "" "$INSTDIR\${Name}.exe" ;
SectionEnd

Section "Uninstall"
    nsis_process::KillProcess "${Name}.exe"
    DeleteRegKey HKCU "Software\${Name}"
    DeleteRegKey HKCU "${PRODUCT_UNINST_KEY}"
    Delete "$SMPROGRAMS\${Name}.lnk"
    Delete "$DESKTOP\${Name}.lnk"
    RMDir /r "$INSTDIR"


SectionEnd
