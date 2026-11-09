; Hooks del instalador de Windows (NSIS), `bundle.windows.nsis.installerHooks`.
;
; Con la 4.0.0 la app pasa de «Proxy Builder» a «Tapete». La carpeta de
; instalación y la entrada de «Desinstalar» salen del `productName`, así que sin
; esto quedarían las dos apps instaladas. Antes de copiar nada, se desinstala en
; silencio la vieja si está:
;
; - En PREINSTALL y no después: su desinstalador borra las entradas de registro de
;   los enlaces (`proxybuilder://`, `tapete://`), y la nueva las escribe después.
; - `/S` (silencioso): no sale la casilla de «Borrar los datos», que se queda sin
;   marcar; los datos (`%APPDATA%\com.estigio.proxybuilder`) son de las dos, porque
;   salen del `identifier`, que no cambia. Sin `/UPDATE`, para que quite también
;   sus accesos directos con el nombre viejo.
; - `_?=` lo ejecuta en su sitio para poder esperarlo; por eso después se borra
;   `uninstall.exe` y la carpeta a mano.
; - Las claves son las de la plantilla de Tauri en modo `currentUser` (el de
;   fábrica): `Software\<fabricante>\<productName>` guarda la carpeta, y el
;   fabricante sale del `identifier` (`estigio`).

!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R9 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Proxy Builder" "UninstallString"
  ${If} $R9 != ""
    ReadRegStr $R8 HKCU "Software\estigio\Proxy Builder" ""
    ${If} $R8 != ""
    ${AndIf} $R8 != $INSTDIR
    ${AndIf} ${FileExists} "$R8\uninstall.exe"
      DetailPrint "Quitando Proxy Builder (ahora se llama Tapete)…"
      ExecWait '"$R8\uninstall.exe" /S _?=$R8'
      Delete "$R8\uninstall.exe"
      RMDir "$R8"
    ${EndIf}
  ${EndIf}
!macroend
