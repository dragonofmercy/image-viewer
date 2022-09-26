param(
     [Parameter()]
     [string]$source,
 
     [Parameter()]
     [string]$dest
)

Write-Host "Installing Image Viewer update, please wait..."

$deleteSource = 0;

if(!$source -Or !$dest)
{
    $source = Split-Path $PSScriptRoot -Parent
    $dest = "$env:LOCALAPPDATA\Dragon Industries\Image Viewer"
}
else
{
    Start-Sleep -Seconds 2
    $deleteSource = 1;
}

if(Test-Path -Path $dest)
{
    Remove-Item -Force -Recurse -Path $dest
}

md -path $dest

Copy-Item -Path $source\* -Destination $dest -Recurse -Force
Remove-Item -Force -Recurse -Path $dest\installer

if($deleteSource)
{
    Remove-Item -Force -Recurse -Path $source
}

$TargetFile = "$dest\ImageViewer.exe"
$ShortcutFile = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Image Viewer.lnk"
$WScriptShell = New-Object -ComObject WScript.Shell
$Shortcut = $WScriptShell.CreateShortcut($ShortcutFile)
$Shortcut.TargetPath = $TargetFile
$Shortcut.Save()

Start-Process -FilePath $dest\ImageViewer.exe