# orig.ps1 : drives the original game (Windows) for comparison.
#   . .\tools\orig.ps1 ; Start-Orig ; Save-OrigShot out\compare\orig\titre.png ; Send-OrigClick 480 505 ; Stop-Orig
# Coordinates: client-area pixels (960x600 by default), top-left origin (like our script steps).
param([string]$Exe = "J:\NGUvita\NGU IDLE\NGUIdle.exe")
Add-Type -ReferencedAssemblies System.Drawing @"
using System; using System.Runtime.InteropServices; using System.Drawing;
public static class W {
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint f);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte sc, uint f, IntPtr e);
  public struct RECT { public int L, T, R, B; } public struct POINT { public int X, Y; }
  public static Bitmap Client(IntPtr h) {
    RECT wr; GetWindowRect(h, out wr); RECT cr; GetClientRect(h, out cr);
    var full = new Bitmap(wr.R - wr.L, wr.B - wr.T);
    using (var g = Graphics.FromImage(full)) { var dc = g.GetHdc(); PrintWindow(h, dc, 2); g.ReleaseHdc(dc); }
    POINT p = new POINT(); ClientToScreen(h, ref p);
    return full.Clone(new Rectangle(p.X - wr.L, p.Y - wr.T, cr.R, cr.B), full.PixelFormat);
  }
}
"@
$script:OrigExe = $Exe
function Get-OrigWindow { (Get-Process NGUIdle -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1).MainWindowHandle }
function Start-Orig([int]$WaitS = 20) {
  # the game requires Steam (launched directly it starts Steam and relaunches from the Steam installation): go through Steam
  if (-not (Get-OrigWindow)) { Start-Process "steam://rungameid/1147690" | Out-Null }
  $t = 0; while (-not (Get-OrigWindow) -and $t -lt $WaitS) { Start-Sleep 1; $t++ }
  Start-Sleep $WaitS
  Get-OrigWindow
}
function Save-OrigShot([string]$Path) {
  New-Item -ItemType Directory -Force (Split-Path $Path) | Out-Null
  $b = [W]::Client((Get-OrigWindow)); $b.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png); "$Path $($b.Width)x$($b.Height)"
}
# Unity ignores posted messages: real mouse click (window in the foreground, cursor restored afterwards).
function Send-OrigClick([int]$X, [int]$Y, [int]$WaitMs = 600, [switch]$Right) {
  $h = Get-OrigWindow
  [W]::keybd_event(0x12, 0, 0, [IntPtr]::Zero); [void][W]::SetForegroundWindow($h); [W]::keybd_event(0x12, 0, 2, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 150
  if ([W]::GetForegroundWindow() -ne $h) { throw "original game not in the foreground: click cancelled" }
  $old = New-Object W+POINT; [void][W]::GetCursorPos([ref]$old)
  $p = New-Object W+POINT; $p.X = $X; $p.Y = $Y; [void][W]::ClientToScreen($h, [ref]$p)
  $down, $up = if ($Right) { 0x08, 0x10 } else { 0x02, 0x04 }
  [void][W]::SetCursorPos($p.X, $p.Y); Start-Sleep -Milliseconds 120
  [W]::mouse_event($down, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 80
  [W]::mouse_event($up, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 80
  [void][W]::SetCursorPos($old.X, $old.Y)
  Start-Sleep -Milliseconds $WaitMs
}
function Stop-Orig { Get-Process NGUIdle -ErrorAction SilentlyContinue | Stop-Process -Force }
