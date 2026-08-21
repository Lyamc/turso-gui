# Capture each frontend window to screenshots/*.png
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$OutDir = Join-Path $Root "screenshots"
$Db = Join-Path $Root "local.db"
$BinDir = Join-Path $Root "target\debug"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -TypeDefinition @"
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;

public class GuiShot {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc lp, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }

  public static IntPtr FindTitle(string part) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      if (!IsWindowVisible(h)) return true;
      var sb = new StringBuilder(512);
      GetWindowText(h, sb, 512);
      string t = sb.ToString();
      if (t.IndexOf(part, StringComparison.OrdinalIgnoreCase) >= 0) {
        found = h;
        return false;
      }
      return true;
    }, IntPtr.Zero);
    return found;
  }

  public static void Capture(IntPtr h, string path) {
    ShowWindow(h, 9);
    SetForegroundWindow(h);
    System.Threading.Thread.Sleep(500);
    RECT r;
    GetWindowRect(h, out r);
    int w = Math.Max(1, r.R - r.L);
    int ht = Math.Max(1, r.B - r.T);
    using (var bmp = new Bitmap(w, ht, PixelFormat.Format32bppArgb)) {
      using (var g = Graphics.FromImage(bmp)) {
        IntPtr hdc = g.GetHdc();
        bool printed = PrintWindow(h, hdc, 2);
        g.ReleaseHdc(hdc);
        if (!printed) {
          g.CopyFromScreen(r.L, r.T, 0, 0, new Size(w, ht));
        } else {
          // If PrintWindow produced an empty/black frame, fall back.
          bool dark = true;
          for (int i = 0; i < 20 && dark; i++) {
            int x = (w * (i + 1)) / 22;
            int y = (ht * ((i * 3) % 19 + 1)) / 22;
            Color c = bmp.GetPixel(Math.Min(w - 1, x), Math.Min(ht - 1, y));
            if (c.R + c.G + c.B > 30) dark = false;
          }
          if (dark) {
            g.CopyFromScreen(r.L, r.T, 0, 0, new Size(w, ht));
          }
        }
      }
      bmp.Save(path, ImageFormat.Png);
    }
  }
}
"@

function Stop-Frontends {
  Get-Process turso-gui, turso-gui-egui, turso-gui-gpui, turso-gui-tk, turso-gui-tui, turso-gui-dioxus, wish -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
}

function Wait-Window([string]$TitlePart, [int]$Seconds = 20) {
  $deadline = (Get-Date).AddSeconds($Seconds)
  do {
    $h = [GuiShot]::FindTitle($TitlePart)
    if ($h -ne [IntPtr]::Zero) { return $h }
    Start-Sleep -Milliseconds 400
  } while ((Get-Date) -lt $deadline)
  throw "Window not found: $TitlePart"
}

function Capture-Gui {
  param($Name, $Exe, $Title, [string[]]$ExtraArgs = @())
  $exePath = Join-Path $BinDir $Exe
  Write-Host "Launching $Name..."
  $args = @("-d", $Db) + $ExtraArgs
  $proc = Start-Process -FilePath $exePath -ArgumentList $args -WorkingDirectory $Root -PassThru
  try {
    $hwnd = Wait-Window $Title 25
    Start-Sleep -Seconds 2
    $out = Join-Path $OutDir "$Name.png"
    [GuiShot]::Capture($hwnd, $out)
    $len = (Get-Item $out).Length
    Write-Host "  saved $out ($len bytes)"
    if ($len -lt 4000) { throw "screenshot too small: $out" }
  } finally {
    if ($proc -and -not $proc.HasExited) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
    Get-Process wish -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
  }
}

Stop-Frontends
Start-Sleep -Milliseconds 500

Capture-Gui iced "turso-gui.exe" "Turso DB Browser (iced)"
Capture-Gui egui "turso-gui-egui.exe" "Turso DB Browser (egui)"
Capture-Gui gpui "turso-gui-gpui.exe" "Turso DB Browser (GPUI)"
Capture-Gui tk "turso-gui-tk.exe" "Turso DB Browser (Tcl/Tk)"
Capture-Gui dioxus "turso-gui-dioxus.exe" "Turso DB Browser (Dioxus)"

Write-Host "Launching tui..."
$tuiExe = Join-Path $BinDir "turso-gui-tui.exe"
$tuiCmd = "title Turso DB Browser (TUI) & `"$tuiExe`" -d `"$Db`""
$tuiProc = Start-Process -FilePath "cmd.exe" -ArgumentList @("/k", $tuiCmd) -WorkingDirectory $Root -PassThru
try {
  $hwnd = Wait-Window "Turso DB Browser (TUI)" 20
  Start-Sleep -Seconds 2
  $out = Join-Path $OutDir "tui.png"
  [GuiShot]::Capture($hwnd, $out)
  $len = (Get-Item $out).Length
  Write-Host "  saved $out ($len bytes)"
  if ($len -lt 4000) { throw "screenshot too small: $out" }
} finally {
  if ($tuiProc -and -not $tuiProc.HasExited) { Stop-Process -Id $tuiProc.Id -Force -ErrorAction SilentlyContinue }
  Get-Process turso-gui-tui -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
}

Stop-Frontends
Write-Host "Done."
Get-ChildItem $OutDir | Format-Table Name, Length
