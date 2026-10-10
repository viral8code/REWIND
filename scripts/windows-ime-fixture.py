#!/usr/bin/env python3
"""Require a real Japanese conversion service on an ephemeral Windows test VM."""
import os
import queue
import subprocess
import sys
import threading
import time

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
# The modern Microsoft input service needs the OS text-input broker, not just
# ctfmon. Hosted images may leave optional desktop services stopped/disabled.
$inputServices=@(Get-Service -Name TabletInputService,TextInputManagementService -ErrorAction SilentlyContinue)
foreach ($service in $inputServices) {
 if ($service.StartType -eq 'Disabled') { Set-Service -Name $service.Name -StartupType Manual }
 if ($service.Status -ne 'Running') { Start-Service -Name $service.Name }
}
@{text_input_services=@(Get-Service -Name TabletInputService,TextInputManagementService -ErrorAction SilentlyContinue | ForEach-Object { @{name=$_.Name;status=$_.Status.ToString();start_type=$_.StartType.ToString()} })} | ConvertTo-Json -Depth 4 -Compress
# Refresh this disposable runner session after installing a new input service.
# Starting ctfmon while its pre-install instance is running does not refresh it.
$session=(Get-Process -Id $PID).SessionId
Get-Process ctfmon -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -eq $session } | Stop-Process -Force
Start-Process "$env:WINDIR\System32\ctfmon.exe"
$deadline=(Get-Date).AddSeconds(10)
while (-not (Get-Process ctfmon -ErrorAction SilentlyContinue | Where-Object { $_.SessionId -eq $session }) -and (Get-Date) -lt $deadline) {
 Start-Sleep -Milliseconds 100
}
$services=@(Get-Process ctfmon,imebroker,MicrosoftIME -ErrorAction SilentlyContinue | ForEach-Object { @{name=$_.ProcessName;session=$_.SessionId} })
$dictionaryRoots=@("$env:WINDIR\IME\IMEJP", "$env:WINDIR\System32\IME\IMEJP", "$env:WINDIR\System32\InputMethod\JPN")
$dictionaries=@($dictionaryRoots | Where-Object { Test-Path $_ } | ForEach-Object { Get-ChildItem -LiteralPath $_ -Recurse -File -Filter '*.dic' -ErrorAction SilentlyContinue } | ForEach-Object { $_.Name })
@{runner_session=$session;input_services=$services;japanese_dictionary_count=$dictionaries.Count;japanese_dictionary_names=@($dictionaries | Select-Object -First 40);system_locale=(Get-WinSystemLocale).Name} | ConvertTo-Json -Depth 4 -Compress
$japanese=Get-WinUserLanguageList | Where-Object { $_.LanguageTag -like 'ja*' }
@{input_tips=@($japanese.InputMethodTips);text_service_running=[bool](Get-Process ctfmon -ErrorAction SilentlyContinue);capability=(Get-WindowsCapability -Online -Name 'Language.Basic~~~ja-JP~0.0.1.0').State.ToString();language='ja-JP'} | ConvertTo-Json -Compress
"""
result=subprocess.run(['powershell.exe','-NoProfile','-NonInteractive','-Command',prepare],capture_output=True,text=True,timeout=900)
if result.returncode:raise RuntimeError('Real Windows IME preparation failed: '+result.stderr[-4000:])
print(result.stdout.strip(),flush=True)
env=dict(os.environ,REWIND_TEST_WINDOWS_IME_SERVICE='1')
# Bound real-service setup/activation as well as notification waits. The cargo
# command includes compilation; actual input phases have their own five-second bound.
# Keep the complete gate on success. Once this exact fixture has already
# panicked, stop its disposable process tree rather than waiting for the OS
# profile's known slow failure teardown. A killed/timeout run always fails.
process = subprocess.Popen(args, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
lines = queue.Queue(maxsize=128)
def read_output():
    try:
        for line in iter(process.stdout.readline, b''):
            lines.put(line)
    finally:
        lines.put(None)
reader = threading.Thread(target=read_output, daemon=True)
reader.start()
def terminate_fixture(reason):
    audit = r"""
$children=@(Get-CimInstance Win32_Process -Filter "ParentProcessId = PARENT_ID")
$modules=@($children | ForEach-Object { Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue } | ForEach-Object { $_.Modules } | Where-Object { $_.ModuleName -match 'ime|imjp|msctf' } | ForEach-Object { @{name=$_.ModuleName;version=$_.FileVersionInfo.FileVersion} })
$services=@(Get-Process ctfmon,imebroker,MicrosoftIME,TextInputHost -ErrorAction SilentlyContinue | ForEach-Object { @{name=$_.ProcessName;session=$_.SessionId} })
@{loaded_input_modules=$modules;input_processes=$services} | ConvertTo-Json -Depth 4 -Compress
""".replace('PARENT_ID', str(process.pid))
    try:
        details = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', audit], capture_output=True, timeout=10)
        sys.stdout.buffer.write(details.stdout)
        sys.stdout.buffer.flush()
    except subprocess.TimeoutExpired:
        print('Input-component diagnostics exceeded their ten-second bound', flush=True)
    # cargo starts the test executable; terminate both, never other sessions.
    subprocess.run(['taskkill.exe', '/PID', str(process.pid), '/T', '/F'],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
    if process.poll() is None:
        process.kill()
    process.wait(timeout=10)
    raise RuntimeError(reason)
deadline = time.monotonic() + 300
failed = False
while True:
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        terminate_fixture('Real Windows IME test panicked; the failed fixture process tree was stopped' if failed else 'Real Windows IME command exceeded its 300-second acceptance bound')
    try:
        line = lines.get(timeout=min(remaining, 1))
    except queue.Empty:
        continue
    if line is None:
        break
    sys.stdout.buffer.write(line)
    sys.stdout.buffer.flush()
    if b'panicked at' in line:
        # The original failing assertion is already in the log. Never turn a
        # failed notification/commit/focus/close assertion into qualification.
        failed = True
        # Drain the assertion text emitted immediately after the panic header.
        deadline = min(deadline, time.monotonic()+1)
if failed:
    terminate_fixture('Real Windows IME test panicked; the failed fixture process tree was stopped')
raise SystemExit(process.wait(timeout=max(1, deadline-time.monotonic())))
