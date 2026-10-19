# DocCraft Suite installer + file-association registration (current user, no admin)
$ErrorActionPreference = 'Stop'
$dest = Join-Path $env:LOCALAPPDATA 'DocCraft'
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item (Join-Path $PSScriptRoot 'DocCraft.exe') $dest -Force
if (Test-Path (Join-Path $PSScriptRoot 'doccraft.ico')) { Copy-Item (Join-Path $PSScriptRoot 'doccraft.ico') $dest -Force }
$exe = Join-Path $dest 'DocCraft.exe'
$exts = @('docx','doc','odt','rtf','xlsx','xlsm','xls','ods','csv','tsv','pptx','potx','ppsx','pdf','fig','figcraft','svg','drawio','xml','vec','md','markdown','typ','tex','ts','note','notes')
$craft = @{'docx'='Writer' 'doc'='Writer' 'odt'='Writer' 'rtf'='Writer' 'xlsx'='Sheets' 'xlsm'='Sheets' 'xls'='Sheets' 'ods'='Sheets' 'csv'='Sheets' 'tsv'='Sheets' 'pptx'='Slides' 'potx'='Slides' 'ppsx'='Slides' 'pdf'='Reader' 'fig'='Design' 'figcraft'='Design' 'svg'='Design' 'drawio'='Draw' 'xml'='Draw' 'vec'='Vector' 'md'='Markdown' 'markdown'='Markdown' 'typ'='Typeset' 'tex'='Typeset' 'ts'='Typeset' 'note'='Notes' 'notes'='Notes'}
foreach ($ext in $exts) {
  $pid2 = "DocCraft.$ext"
  New-Item -Path "HKCU:\Software\Classes\$pid2\shell\open\command" -Force | Out-Null
  Set-ItemProperty "HKCU:\Software\Classes\$pid2" '(default)' "DocCraft $($craft[$ext]) Document"
  Set-ItemProperty "HKCU:\Software\Classes\$pid2\shell\open\command" '(default)' "`"$exe`" `"%1`""
  Set-ItemProperty "HKCU:\Software\Classes\$pid2\DefaultIcon" '(default)' "$exe,0"
  New-Item -Path "HKCU:\Software\Classes\.$ext\OpenWithProgids" -Force | Out-Null
  New-ItemProperty "HKCU:\Software\Classes\.$ext\OpenWithProgids" -Name $pid2 -Value "DocCraft $($craft[$ext]) Document" -PropertyType String -Force | Out-Null
}
$ws = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut([Environment]::GetFolderPath('StartMenu') + '\Programs\DocCraft.lnk')
$lnk.TargetPath = $exe; $lnk.IconLocation = "$exe,0"; $lnk.Save()
Write-Host "DocCraft installed to $dest and registered for $($exts.Count) file types."
