#!/usr/bin/env python3
"""Require a real Japanese conversion service on an ephemeral Windows test VM."""
import os
import subprocess
import sys

if sys.platform != 'win32':raise SystemExit('Real Windows IME acceptance requires Windows')
args=sys.argv[1:]
if args and args[0]=='--':args=args[1:]
if not args:raise SystemExit('usage: windows-ime-fixture.py -- COMMAND [ARG ...]')
# This installer belongs to CI acceptance, never to the distributed VM or user startup.
# Enable Japanese as a second profile; preserve the existing primary language.
prepare=r"""
$ErrorActionPreference='Stop'
$capability=Get-WindowsCapability -Online -Name 'Language.Basic~~~ja-JP~0.0.1.0'
if ($capability.State -ne 'Installed') {
 $installed=Add-WindowsCapability -Online -Name 'Language.Basic~~~ja-JP~0.0.1.0'
 if ($installed.RestartNeeded) { throw 'Japanese IME installation requires a reboot; this runner cannot complete real-engine acceptance' }
}
$profiles=Get-WinUserLanguageList
if (-not ($profiles.LanguageTag -contains 'ja-JP')) {
 $profiles.Add('ja-JP')
 Set-WinUserLanguageList $profiles -Force
}
# A capability installed after login does not itself start the user's text service.
Start-Process "$env:WINDIR\System32\ctfmon.exe"
$deadline=(Get-Date).AddSeconds(10)
while (-not (Get-Process ctfmon -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) {
 Start-Sleep -Milliseconds 100
}
$japanese=Get-WinUserLanguageList | Where-Object LanguageTag -eq 'ja-JP'
@{input_tips=@($japanese.InputMethodTips);text_service_running=[bool](Get-Process ctfmon -ErrorAction SilentlyContinue);capability=(Get-WindowsCapability -Online -Name 'Language.Basic~~~ja-JP~0.0.1.0').State.ToString();language='ja-JP'} | ConvertTo-Json -Compress
"""
result=subprocess.run(['powershell.exe','-NoProfile','-NonInteractive','-Command',prepare],capture_output=True,text=True,timeout=900)
if result.returncode:raise RuntimeError('Real Windows IME preparation failed: '+result.stderr[-4000:])
print(result.stdout.strip(),flush=True)
env=dict(os.environ,REWIND_TEST_WINDOWS_IME_SERVICE='1')
# Bound real-service setup/activation as well as notification waits. The cargo
# command includes compilation; actual input phases have their own five-second bound.
try:
    result = subprocess.run(args, env=env, timeout=300)
except subprocess.TimeoutExpired as error:
    raise RuntimeError('Real Windows IME command exceeded its 300-second acceptance bound') from error
raise SystemExit(result.returncode)
