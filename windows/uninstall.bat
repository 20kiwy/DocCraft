@echo off
REM Uninstall DocCraft + remove all file associations
setlocal
echo Removing DocCraft file associations ...
reg delete "HKCU\Software\Classes\DocCraft.docx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.docx\OpenWithProgids" /v "DocCraft.docx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.doc" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.doc\OpenWithProgids" /v "DocCraft.doc" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.odt" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.odt\OpenWithProgids" /v "DocCraft.odt" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.rtf" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.rtf\OpenWithProgids" /v "DocCraft.rtf" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.xlsx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.xlsx\OpenWithProgids" /v "DocCraft.xlsx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.xlsm" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.xlsm\OpenWithProgids" /v "DocCraft.xlsm" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.xls" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.xls\OpenWithProgids" /v "DocCraft.xls" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.ods" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.ods\OpenWithProgids" /v "DocCraft.ods" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.csv" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.csv\OpenWithProgids" /v "DocCraft.csv" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.tsv" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.tsv\OpenWithProgids" /v "DocCraft.tsv" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.pptx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.pptx\OpenWithProgids" /v "DocCraft.pptx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.potx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.potx\OpenWithProgids" /v "DocCraft.potx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.ppsx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.ppsx\OpenWithProgids" /v "DocCraft.ppsx" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.pdf" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.pdf\OpenWithProgids" /v "DocCraft.pdf" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.fig" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.fig\OpenWithProgids" /v "DocCraft.fig" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.figcraft" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.figcraft\OpenWithProgids" /v "DocCraft.figcraft" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.svg" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.svg\OpenWithProgids" /v "DocCraft.svg" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.drawio" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.drawio\OpenWithProgids" /v "DocCraft.drawio" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.xml" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.xml\OpenWithProgids" /v "DocCraft.xml" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.vec" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.vec\OpenWithProgids" /v "DocCraft.vec" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.md" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.md\OpenWithProgids" /v "DocCraft.md" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.markdown" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.markdown\OpenWithProgids" /v "DocCraft.markdown" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.typ" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.typ\OpenWithProgids" /v "DocCraft.typ" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.tex" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.tex\OpenWithProgids" /v "DocCraft.tex" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.ts" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.ts\OpenWithProgids" /v "DocCraft.ts" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.note" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.note\OpenWithProgids" /v "DocCraft.note" /f >nul 2>&1
reg delete "HKCU\Software\Classes\DocCraft.notes" /f >nul 2>&1
reg delete "HKCU\Software\Classes\.notes\OpenWithProgids" /v "DocCraft.notes" /f >nul 2>&1
reg delete "HKCU\Software\Classes\Applications\DocCraft.exe" /f >nul 2>&1
powershell -NoProfile -Command "Remove-Item ([Environment]::GetFolderPath('StartMenu') + '\Programs\DocCraft.lnk') -ErrorAction SilentlyContinue"
rmdir /s /q "%LOCALAPPDATA%\DocCraft" 2>nul
echo DocCraft uninstalled.
pause
