# Measure release binary size and 5x open/close cycles (time + peak working set).
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$BinDir = Join-Path $Root "target\release"
$Db = Join-Path $Root "local.db"
$OutJson = Join-Path $Root "screenshots\release-bench.json"
$Cycles = 5
$ReadyTimeoutSec = 45
$CloseTimeoutSec = 8

Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;

public class BenchWin {
  public const uint WM_CLOSE = 0x0010;
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc lp, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint Msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

  public static IntPtr FindForProcess(int pid, string titlePart) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint wpid;
      GetWindowThreadProcessId(h, out wpid);
      if (wpid != (uint)pid) return true;
      if (!IsWindowVisible(h)) return true;
      var sb = new StringBuilder(512);
      GetWindowText(h, sb, 512);
      string t = sb.ToString();
      if (string.IsNullOrEmpty(titlePart) || t.IndexOf(titlePart, StringComparison.OrdinalIgnoreCase) >= 0) {
        found = h;
        return false;
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }

  public static void ClosePid(int pid) {
    EnumWindows((h, l) => {
      uint wpid;
      GetWindowThreadProcessId(h, out wpid);
      if (wpid == (uint)pid) {
        PostMessage(h, WM_CLOSE, IntPtr.Zero, IntPtr.Zero);
      }
      return true;
    }, IntPtr.Zero);
  }

  public static IntPtr FindTitle(string part) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      if (!IsWindowVisible(h)) return true;
      var sb = new StringBuilder(512);
      GetWindowText(h, sb, 512);
      if (sb.ToString().IndexOf(part, StringComparison.OrdinalIgnoreCase) >= 0) {
        found = h;
        return false;
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@

function Stop-Related {
  param([int]$PidHint)
  if ($PidHint) {
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$PidHint" -ErrorAction SilentlyContinue |
      ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Stop-Process -Id $PidHint -Force -ErrorAction SilentlyContinue
  }
  Get-Process wish, turso-gui, turso-gui-egui, turso-gui-gpui, turso-gui-tk, turso-gui-dioxus, turso-gui-tui -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
}

function Get-TreeWorkingSet {
  param([int]$RootPid)
  $sum = [int64]0
  $queue = New-Object System.Collections.Generic.Queue[int]
  $queue.Enqueue($RootPid)
  $seen = @{}
  while ($queue.Count -gt 0) {
    $id = $queue.Dequeue()
    if ($seen.ContainsKey($id)) { continue }
    $seen[$id] = $true
    try {
      $p = Get-Process -Id $id -ErrorAction Stop
      $sum += [int64]$p.WorkingSet64
    } catch {}
    Get-CimInstance Win32_Process -Filter "ParentProcessId=$id" -ErrorAction SilentlyContinue |
      ForEach-Object { $queue.Enqueue([int]$_.ProcessId) }
  }
  return $sum
}

function Wait-ReadyWindow {
  param($Proc, [string]$Title, [int]$Seconds)
  $deadline = (Get-Date).AddSeconds($Seconds)
  do {
    if ($Proc.HasExited) { throw "process exited before window: $($Proc.ExitCode)" }
    $h = [BenchWin]::FindForProcess($Proc.Id, $Title)
    if ($h -eq [IntPtr]::Zero -and $Title) { $h = [BenchWin]::FindTitle($Title) }
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 80
  } while ((Get-Date) -lt $deadline)
  throw "window not ready: $Title pid=$($Proc.Id)"
}

function Wait-Exit {
  param($Proc, [int]$Seconds)
  $deadline = (Get-Date).AddSeconds($Seconds)
  do {
    if ($Proc.HasExited) { return }
    Start-Sleep -Milliseconds 50
  } while ((Get-Date) -lt $deadline)
  throw "process did not exit: pid=$($Proc.Id)"
}

$frontends = @(
  @{ Name = "iced";   Exe = "turso-gui.exe";        Title = "Turso DB Browser (iced)";   Kind = "gui" }
  @{ Name = "egui";   Exe = "turso-gui-egui.exe";   Title = "Turso DB Browser (egui)";   Kind = "gui" }
  @{ Name = "gpui";   Exe = "turso-gui-gpui.exe";   Title = "Turso DB Browser (GPUI)";   Kind = "gui" }
  @{ Name = "tk";     Exe = "turso-gui-tk.exe";     Title = "Turso DB Browser (Tcl/Tk)"; Kind = "gui" }
  @{ Name = "dioxus"; Exe = "turso-gui-dioxus.exe"; Title = "Turso DB Browser (Dioxus)"; Kind = "gui" }
  @{ Name = "tui";    Exe = "turso-gui-tui.exe";    Title = "";                          Kind = "tui" }
)

Stop-Related -PidHint 0

$results = @()

foreach ($fe in $frontends) {
  $exePath = Join-Path $BinDir $fe.Exe
  if (-not (Test-Path $exePath)) { throw "missing $exePath" }
  $size = [int64](Get-Item $exePath).Length
  Write-Host "=== $($fe.Name) size=$size ==="

  $peak = [int64]0
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  for ($i = 1; $i -le $Cycles; $i++) {
    Write-Host "  cycle $i"
    if ($fe.Kind -eq "tui") {
      $cmdArgs = "/c title Turso DB Browser (TUI)& `"$exePath`" -d `"$Db`""
      $cmdProc = Start-Process -FilePath "cmd.exe" -ArgumentList $cmdArgs -WorkingDirectory $Root -PassThru
      try {
        $deadline = (Get-Date).AddSeconds($ReadyTimeoutSec)
        $hwnd = [IntPtr]::Zero
        $proc = $null
        do {
          if ($cmdProc.HasExited) { throw "tui cmd exited before window: $($cmdProc.ExitCode)" }
          $hwnd = [BenchWin]::FindTitle("Turso DB Browser (TUI)")
          $proc = Get-Process -Name "turso-gui-tui" -ErrorAction SilentlyContinue | Select-Object -First 1
          if ($hwnd -ne [IntPtr]::Zero -and $proc) { break }
          Start-Sleep -Milliseconds 80
        } while ((Get-Date) -lt $deadline)
        if ($hwnd -eq [IntPtr]::Zero -or -not $proc) { throw "tui window not ready" }
        Start-Sleep -Milliseconds 200
        $ws = Get-TreeWorkingSet $proc.Id
        if ($ws -gt $peak) { $peak = $ws }
        [void][BenchWin]::ShowWindow($hwnd, 9)
        [void][BenchWin]::SetForegroundWindow($hwnd)
        Start-Sleep -Milliseconds 80
        [System.Windows.Forms.SendKeys]::SendWait("q")
        try { Wait-Exit $proc $CloseTimeoutSec } catch {
          Write-Host "    tui close timed out; killing"
          Stop-Related -PidHint $proc.Id
        }
      } catch {
        Stop-Related -PidHint $cmdProc.Id
        throw
      }
      if ($proc -and -not $proc.HasExited) { Stop-Related -PidHint $proc.Id }
      if (-not $cmdProc.HasExited) { Stop-Process -Id $cmdProc.Id -Force -ErrorAction SilentlyContinue }
    } else {
      $proc = Start-Process -FilePath $exePath -ArgumentList @("-d", $Db) -WorkingDirectory $Root -PassThru
      try {
        $hwnd = Wait-ReadyWindow $proc $fe.Title $ReadyTimeoutSec
        Start-Sleep -Milliseconds 150
        $ws = Get-TreeWorkingSet $proc.Id
        if ($ws -gt $peak) { $peak = $ws }
        [void][BenchWin]::PostMessage($hwnd, [BenchWin]::WM_CLOSE, [IntPtr]::Zero, [IntPtr]::Zero)
        [void]$proc.CloseMainWindow()
        [BenchWin]::ClosePid($proc.Id)
        Get-CimInstance Win32_Process -Filter "ParentProcessId=$($proc.Id)" -ErrorAction SilentlyContinue | ForEach-Object {
          [BenchWin]::ClosePid([int]$_.ProcessId)
        }
        try {
          Wait-Exit $proc $CloseTimeoutSec
        } catch {
          Write-Host "    close timed out; killing pid $($proc.Id)"
          Stop-Related -PidHint $proc.Id
        }
      } catch {
        Stop-Related -PidHint $proc.Id
        throw
      }
      if (-not $proc.HasExited) {
        Stop-Related -PidHint $proc.Id
      }
    }
  }
  $sw.Stop()
  Stop-Related -PidHint 0

  $row = [ordered]@{
    name              = $fe.Name
    exe               = $fe.Exe
    file_bytes        = $size
    cycles            = $Cycles
    open_close_ms     = [int64]$sw.ElapsedMilliseconds
    peak_working_set  = $peak
  }
  $results += $row
  Write-Host ("  {0}: {1:N1} MiB file, {2} ms / {3} cycles, peak WS {4:N1} MiB" -f `
    $fe.Name, ($size/1MB), $row.open_close_ms, $Cycles, ($peak/1MB))
}

New-Item -ItemType Directory -Force -Path (Split-Path $OutJson) | Out-Null
[System.IO.File]::WriteAllText($OutJson, ($results | ConvertTo-Json -Depth 4))
Write-Host "Wrote $OutJson"
$results | ForEach-Object { $_ | Format-List }
