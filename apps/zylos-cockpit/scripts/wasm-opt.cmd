@echo off
rem Otimiza o bundle WASM (Trunk post_build). O Trunk nao expande variaveis nos argumentos do
rem hook; aqui o cmd expande TRUNK_STAGING_DIR. Sem wasm-opt instalado, o build segue sem otimizar.
set "WOPT=%LOCALAPPDATA%\Temp\binaryen-version_121\bin\wasm-opt.exe"
if not exist "%WOPT%" (
  echo wasm-opt ausente em %WOPT%: bundle nao otimizado
  exit /b 0
)
for %%f in ("%TRUNK_STAGING_DIR%\*_bg.wasm") do "%WOPT%" -Oz --strip-debug --strip-producers "%%f" -o "%%f"
exit /b %ERRORLEVEL%
