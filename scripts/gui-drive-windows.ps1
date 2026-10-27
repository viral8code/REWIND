<#
.SYNOPSIS
  Runs a REWIND GUI program in a real Windows window, drives it with mouse and
  keyboard input, and saves a screenshot of its client area.

.PARAMETER Source   Path of the .rw program.
.PARAMETER Title    Window title the program sets (used to find the window).
.PARAMETER Steps    Semicolon separated actions, run in order:
                      click:X,Y     left click at client coordinates (physical pixels)
                      clickf:FX,FY  left click at a fraction (0..1) of the client area
                      keys:TEXT     SendKeys text (see System.Windows.Forms.SendKeys)
                      wait:MS       sleep
                      shot:FILE     save the client area as PNG
                      resize:W,H    resize the window so its client area is W x H
.PARAMETER Rewind   Path of rewind.exe (default: target\release\rewind.exe).

Example:
  .\gui-drive-windows.ps1 -Source main.rw -Title Counter -Steps "click:80,100;wait:500;shot:out.png"
#>
param(
  [Parameter(Mandatory = $true)][string]$Source,
  [Parameter(Mandatory = $true)][string]$Title,
  [string]$Steps = "",
  [string]$Rewind = "",
  [string]$Effects = "gui",
  [int]$StartupMs = 2500
)
if (-not $Rewind) { $Rewind = Join-Path (Split-Path (Split-Path $MyInvocation.MyCommand.Path)) "target\release\rewind.exe" }
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @"
using System; using System.Text; using System.Runtime.InteropServices;
public class W {
  public struct RECT { public int Left, Top, Right, Bottom; }
  public struct POINT { public int X, Y; }
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr a, int x, int y, int w, int hgt, uint f);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public static IntPtr Find(uint pid, string title) {
    IntPtr found = IntPtr.Zero;
    EnumWindows(delegate (IntPtr h, IntPtr l) {
      uint owner; GetWindowThreadProcessId(h, out owner);
      if (owner == pid && IsWindowVisible(h)) {
        StringBuilder sb = new StringBuilder(256); GetWindowText(h, sb, 256);
        if (sb.ToString() == title) { found = h; return false; }
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@
[void][W]::SetProcessDPIAware()
$here = (Get-Location).Path
$process = Start-Process -FilePath $Rewind -ArgumentList @('run', $Source, '--allow-effects', $Effects) `
  -WorkingDirectory (Split-Path (Resolve-Path $Source)) -PassThru -NoNewWindow
Start-Sleep -Milliseconds $StartupMs

function Client-Origin($h) {
  $p = New-Object W+POINT
  [void][W]::ClientToScreen($h, [ref]$p)
  return $p
}
$window = [W]::Find([uint32]$process.Id, $Title)
if ($window -eq [IntPtr]::Zero) {
  if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
  throw "window '$Title' was not found"
}
[void][W]::SetWindowPos($window, [IntPtr]::Zero, 40, 40, 0, 0, 0x0001 -bor 0x0004)
[void][W]::SetForegroundWindow($window)
Start-Sleep -Milliseconds 300

try {
  foreach ($step in ($Steps -split ';')) {
    if ($step.Trim() -eq '') { continue }
    $name, $arg = $step -split ':', 2
    switch ($name) {
      'click' {
        $xy = $arg -split ','
        $origin = Client-Origin $window
        [void][W]::SetCursorPos($origin.X + [int]$xy[0], $origin.Y + [int]$xy[1])
        Start-Sleep -Milliseconds 200
        [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 80
        [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 500
      }
      'clickf' {
        $f = $arg -split ','
        $client = New-Object W+RECT
        [void][W]::GetClientRect($window, [ref]$client)
        $origin = Client-Origin $window
        [void][W]::SetCursorPos($origin.X + [int]($client.Right * [double]$f[0]), $origin.Y + [int]($client.Bottom * [double]$f[1]))
        Start-Sleep -Milliseconds 200
        [W]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 80
        [W]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 500
      }
      'keys' { [void][W]::SetForegroundWindow($window); [System.Windows.Forms.SendKeys]::SendWait($arg); Start-Sleep -Milliseconds 500 }
      'wait' { Start-Sleep -Milliseconds ([int]$arg) }
      'resize' {
        $wh = $arg -split ','
        $outer = New-Object W+RECT; $inner = New-Object W+RECT
        [void][W]::GetWindowRect($window, [ref]$outer); [void][W]::GetClientRect($window, [ref]$inner)
        $extraW = ($outer.Right - $outer.Left) - $inner.Right
        $extraH = ($outer.Bottom - $outer.Top) - $inner.Bottom
        [void][W]::SetWindowPos($window, [IntPtr]::Zero, 0, 0, [int]$wh[0] + $extraW, [int]$wh[1] + $extraH, 0x0002 -bor 0x0004)
        Start-Sleep -Milliseconds 700
      }
      'shot' {
        $client = New-Object W+RECT
        [void][W]::GetClientRect($window, [ref]$client)
        $origin = Client-Origin $window
        $bmp = New-Object System.Drawing.Bitmap $client.Right, $client.Bottom
        $graphics = [System.Drawing.Graphics]::FromImage($bmp)
        $graphics.CopyFromScreen($origin.X, $origin.Y, 0, 0, $bmp.Size)
        $target = if ([System.IO.Path]::IsPathRooted($arg)) { $arg } else { Join-Path $here $arg }
        $bmp.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
        $graphics.Dispose(); $bmp.Dispose()
      }
      default { throw "unknown step $name" }
    }
  }
}
finally {
  $process.Refresh()
  if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
}
