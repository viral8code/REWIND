<#
.SYNOPSIS
  Windows-only smoke test of the native GUI: opens examples\notes in a real
  window, types two lines with the keyboard, presses Save with the mouse and
  checks the saved file. Needs an interactive desktop (it moves the mouse).

Usage (from the repository root, after `cargo build --release`):
  powershell -NoProfile -ExecutionPolicy Bypass -File scripts\gui-smoke-windows.ps1
#>
$root = Split-Path (Split-Path $MyInvocation.MyCommand.Path)
$work = Join-Path ([System.IO.Path]::GetTempPath()) ("rewind-gui-smoke-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $work | Out-Null
Copy-Item (Join-Path $root "examples\notes\main.rw") (Join-Path $work "main.rw")
$drive = Join-Path $PSScriptRoot "gui-drive-windows.ps1"
$shot = Join-Path $work "after-typing.png"

& powershell -NoProfile -ExecutionPolicy Bypass -File $drive -Source (Join-Path $work "main.rw") `
  -Title "REWIND notes" -Effects "gui,fileRead,fileWrite" `
  -Steps "clickf:0.5,0.45;keys:hello{ENTER}world;shot:$shot;clickf:0.25,0.92;wait:800"

$saved = Join-Path $work "notes.txt"
if (-not (Test-Path $saved)) { throw "FAIL: notes.txt was not written" }
$text = (Get-Content $saved -Raw).Replace("`r`n", "`n")
if ($text -ne "hello`nworld") { throw "FAIL: unexpected file content: $text" }
if (-not (Test-Path $shot)) { throw "FAIL: no screenshot" }
Write-Output "OK: typed text was saved ($saved)"
Remove-Item -Recurse -Force $work
