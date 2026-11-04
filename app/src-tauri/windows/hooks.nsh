; Hooks do instalador NSIS do streamaZap v3.
; O POSTINSTALL remove a versao antiga (app Python, instalada via Inno Setup).
; AppId do Inno em packaging/installer.iss: {6B1F3E2A-8C4D-4F7B-9A51-2D3E8F0C7A11}
; (chave de desinstalacao = AppId + "_is1").

!macro NSIS_HOOK_PREINSTALL
  ; (vazio) nada a fazer antes de instalar.
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Procura o desinstalador do Inno: HKLM (32-bit), HKLM (64-bit) e por fim HKCU.
  StrCpy $0 ""
  ReadRegStr $0 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\{6B1F3E2A-8C4D-4F7B-9A51-2D3E8F0C7A11}_is1" "UninstallString"
  StrCmp $0 "" 0 zap_old_found
  SetRegView 64
  ReadRegStr $0 HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\{6B1F3E2A-8C4D-4F7B-9A51-2D3E8F0C7A11}_is1" "UninstallString"
  SetRegView lastused
  StrCmp $0 "" 0 zap_old_found
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\{6B1F3E2A-8C4D-4F7B-9A51-2D3E8F0C7A11}_is1" "UninstallString"
  StrCmp $0 "" zap_old_done 0

  zap_old_found:
    ; O UninstallString vem entre aspas: remove-as para passar o caminho puro.
    StrCpy $1 $0 1
    StrCmp $1 '"' 0 zap_old_noquote
    StrCpy $0 $0 "" 1
    StrCpy $1 $0 1 -1
    StrCmp $1 '"' 0 zap_old_noquote
    StrCpy $0 $0 -1
  zap_old_noquote:
    IfFileExists "$0" 0 zap_old_done
    DetailPrint "Removendo versao antiga (Python)..."
    ; Eleva uma unica vez (runas) e espera o desinstalador silencioso terminar.
    ClearErrors
    ExecShellWait "runas" "$0" "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART"
    IfErrors 0 zap_old_done
    ; Falha do desinstalador nao deve abortar a instalacao nova.
    DetailPrint "Nao foi possivel remover a versao antiga (Python); seguindo."
  zap_old_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; (vazio)
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; (vazio)
!macroend
