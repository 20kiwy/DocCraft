@echo off
REM ============================================================
REM  DocCraft Suite - install + register file associations
REM  Double-click to install DocCraft for the current user and
REM  register every supported file type (Word/Excel/PowerPoint/
REM  PDF/drawio/Markdown/Typeset/Notes/SVG...). No admin needed.
REM ============================================================
setlocal
set "DEST=%LOCALAPPDATA%\DocCraft"
echo Installing DocCraft to %DEST% ...
if not exist "%DEST%" mkdir "%DEST%"
copy /Y "%~dp0DocCraft.exe" "%DEST%\DocCraft.exe" >nul
copy /Y "%~dp0doccraft.ico" "%DEST%\doccraft.ico" >nul 2>&1

echo Registering file associations (Open with DocCraft) ...
reg add "HKCU\Software\Classes\DocCraft.docx" /ve /d "DocCraft Writer Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.docx\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.docx\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.docx\OpenWithProgids" /v "DocCraft.docx" /t REG_SZ /d "DocCraft Writer Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.doc" /ve /d "DocCraft Writer Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.doc\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.doc\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.doc\OpenWithProgids" /v "DocCraft.doc" /t REG_SZ /d "DocCraft Writer Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.odt" /ve /d "DocCraft Writer Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.odt\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.odt\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.odt\OpenWithProgids" /v "DocCraft.odt" /t REG_SZ /d "DocCraft Writer Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.rtf" /ve /d "DocCraft Writer Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.rtf\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.rtf\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.rtf\OpenWithProgids" /v "DocCraft.rtf" /t REG_SZ /d "DocCraft Writer Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xlsx" /ve /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xlsx\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xlsx\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.xlsx\OpenWithProgids" /v "DocCraft.xlsx" /t REG_SZ /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xlsm" /ve /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xlsm\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xlsm\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.xlsm\OpenWithProgids" /v "DocCraft.xlsm" /t REG_SZ /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xls" /ve /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xls\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xls\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.xls\OpenWithProgids" /v "DocCraft.xls" /t REG_SZ /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.ods" /ve /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.ods\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.ods\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.ods\OpenWithProgids" /v "DocCraft.ods" /t REG_SZ /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.csv" /ve /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.csv\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.csv\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.csv\OpenWithProgids" /v "DocCraft.csv" /t REG_SZ /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.tsv" /ve /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.tsv\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.tsv\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.tsv\OpenWithProgids" /v "DocCraft.tsv" /t REG_SZ /d "DocCraft Sheets Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.pptx" /ve /d "DocCraft Slides Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.pptx\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.pptx\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.pptx\OpenWithProgids" /v "DocCraft.pptx" /t REG_SZ /d "DocCraft Slides Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.potx" /ve /d "DocCraft Slides Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.potx\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.potx\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.potx\OpenWithProgids" /v "DocCraft.potx" /t REG_SZ /d "DocCraft Slides Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.ppsx" /ve /d "DocCraft Slides Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.ppsx\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.ppsx\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.ppsx\OpenWithProgids" /v "DocCraft.ppsx" /t REG_SZ /d "DocCraft Slides Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.pdf" /ve /d "DocCraft Reader Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.pdf\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.pdf\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.pdf\OpenWithProgids" /v "DocCraft.pdf" /t REG_SZ /d "DocCraft Reader Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.fig" /ve /d "DocCraft Design Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.fig\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.fig\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.fig\OpenWithProgids" /v "DocCraft.fig" /t REG_SZ /d "DocCraft Design Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.figcraft" /ve /d "DocCraft Design Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.figcraft\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.figcraft\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.figcraft\OpenWithProgids" /v "DocCraft.figcraft" /t REG_SZ /d "DocCraft Design Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.svg" /ve /d "DocCraft Design Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.svg\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.svg\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.svg\OpenWithProgids" /v "DocCraft.svg" /t REG_SZ /d "DocCraft Design Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.drawio" /ve /d "DocCraft Draw Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.drawio\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.drawio\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.drawio\OpenWithProgids" /v "DocCraft.drawio" /t REG_SZ /d "DocCraft Draw Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xml" /ve /d "DocCraft Draw Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xml\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.xml\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.xml\OpenWithProgids" /v "DocCraft.xml" /t REG_SZ /d "DocCraft Draw Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.vec" /ve /d "DocCraft Vector Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.vec\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.vec\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.vec\OpenWithProgids" /v "DocCraft.vec" /t REG_SZ /d "DocCraft Vector Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.md" /ve /d "DocCraft Markdown Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.md\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.md\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.md\OpenWithProgids" /v "DocCraft.md" /t REG_SZ /d "DocCraft Markdown Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.markdown" /ve /d "DocCraft Markdown Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.markdown\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.markdown\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.markdown\OpenWithProgids" /v "DocCraft.markdown" /t REG_SZ /d "DocCraft Markdown Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.typ" /ve /d "DocCraft Typeset Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.typ\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.typ\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.typ\OpenWithProgids" /v "DocCraft.typ" /t REG_SZ /d "DocCraft Typeset Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.tex" /ve /d "DocCraft Typeset Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.tex\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.tex\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.tex\OpenWithProgids" /v "DocCraft.tex" /t REG_SZ /d "DocCraft Typeset Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.ts" /ve /d "DocCraft Typeset Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.ts\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.ts\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.ts\OpenWithProgids" /v "DocCraft.ts" /t REG_SZ /d "DocCraft Typeset Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.note" /ve /d "DocCraft Notes Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.note\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.note\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.note\OpenWithProgids" /v "DocCraft.note" /t REG_SZ /d "DocCraft Notes Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.notes" /ve /d "DocCraft Notes Document" /f >nul
reg add "HKCU\Software\Classes\DocCraft.notes\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul
reg add "HKCU\Software\Classes\DocCraft.notes\DefaultIcon" /ve /d "%LOCALAPPDATA%\DocCraft\DocCraft.exe,0" /f >nul
reg add "HKCU\Software\Classes\.notes\OpenWithProgids" /v "DocCraft.notes" /t REG_SZ /d "DocCraft Notes Document" /f >nul
rem Also offer DocCraft in the generic "Open with" dialog
reg add "HKCU\Software\Classes\Applications\DocCraft.exe\shell\open\command" /ve /d "\"%LOCALAPPDATA%\DocCraft\DocCraft.exe\" \"%1\"" /f >nul

echo Creating Start Menu shortcut ...
powershell -NoProfile -Command "$ws = New-Object -ComObject WScript.Shell; $s = $ws.CreateShortcut([Environment]::GetFolderPath('StartMenu') + '\Programs\DocCraft.lnk'); $s.TargetPath = '%DEST%\DocCraft.exe'; $s.IconLocation = '%DEST%\DocCraft.exe,0'; $s.Save()"

echo.
echo Done! Right-click any document and choose "Open with ^> DocCraft".
echo (If "Open with" does not list DocCraft yet, log out/in or run this once more.)
pause
