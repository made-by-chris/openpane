param(
  [string]$EncodedConfig
)

$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($EncodedConfig)) {
  throw "Missing encoded configuration."
}

$configJson = [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String($EncodedConfig))
$config = $configJson | ConvertFrom-Json
$cols = [int]$config.cols
$rows = [int]$config.rows
$allDisplays = [bool]$config.allDisplays
$commandText = [string]$config.commandText

if ($cols -lt 1) {
  throw "`cols` must be a positive integer."
}

if ($rows -lt 1) {
  throw "`rows` must be a positive integer."
}

if (-not (Get-Command wt.exe -ErrorAction SilentlyContinue)) {
  throw "Windows Terminal (`wt.exe`) is required but was not found on PATH."
}

Add-Type -AssemblyName System.Windows.Forms

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;

public static class GridNative {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool EnumWindows(EnumWindowsProc lpEnumFunc, IntPtr lParam);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool MoveWindow(IntPtr hWnd, int X, int Y, int nWidth, int nHeight, bool bRepaint);

    [DllImport("user32.dll")]
    public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool IsWindowVisible(IntPtr hWnd);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    public static extern int GetWindowText(IntPtr hWnd, StringBuilder lpString, int nMaxCount);

    [DllImport("user32.dll")]
    public static extern int GetWindowTextLength(IntPtr hWnd);
}
"@

function Get-TargetScreens {
  if ($allDisplays) {
    return [System.Windows.Forms.Screen]::AllScreens
  }

  $foregroundHandle = [GridNative]::GetForegroundWindow()

  if ($foregroundHandle -ne [IntPtr]::Zero) {
    return @([System.Windows.Forms.Screen]::FromHandle($foregroundHandle))
  }

  return @([System.Windows.Forms.Screen]::PrimaryScreen)
}

function Get-CellBounds([System.Drawing.Rectangle]$workArea, [int]$cols, [int]$rows, [int]$index) {
  $row = [int][Math]::Floor($index / $cols)
  $col = $index % $cols
  $cellWidth = [int][Math]::Floor($workArea.Width / $cols)
  $cellHeight = [int][Math]::Floor($workArea.Height / $rows)
  $x = $workArea.X + ($col * $cellWidth)
  $y = $workArea.Y + ($row * $cellHeight)

  if ($col -eq ($cols - 1)) {
    $width = $workArea.Right - $x
  } else {
    $width = $cellWidth
  }

  if ($row -eq ($rows - 1)) {
    $height = $workArea.Bottom - $y
  } else {
    $height = $cellHeight
  }

  return @($x, $y, $width, $height)
}

function Get-WindowHandlesByTitle([string]$titleFragment) {
  $handles = New-Object System.Collections.Generic.List[IntPtr]

  $callback = [GridNative+EnumWindowsProc]{
    param([IntPtr]$handle, [IntPtr]$lParam)

    if (-not [GridNative]::IsWindowVisible($handle)) {
      return $true
    }

    $length = [GridNative]::GetWindowTextLength($handle)

    if ($length -le 0) {
      return $true
    }

    $builder = New-Object System.Text.StringBuilder ($length + 1)
    [void][GridNative]::GetWindowText($handle, $builder, $builder.Capacity)
    $windowTitle = $builder.ToString()

    if ($windowTitle -like "*$titleFragment*") {
      [void]$handles.Add($handle)
    }

    return $true
  }

  [void][GridNative]::EnumWindows($callback, [IntPtr]::Zero)
  return $handles
}

function Wait-ForWindowHandle([string]$titleFragment, [int]$timeoutMs = 15000) {
  $deadline = (Get-Date).AddMilliseconds($timeoutMs)

  while ((Get-Date) -lt $deadline) {
    $handles = Get-WindowHandlesByTitle -titleFragment $titleFragment

    if ($handles.Count -gt 0) {
      return $handles[0]
    }

    Start-Sleep -Milliseconds 100
  }

  throw "Timed out waiting for a Windows Terminal window titled '$titleFragment'."
}

function Move-TerminalWindow([IntPtr]$handle, [int[]]$bounds) {
  [GridNative]::ShowWindow($handle, 9) | Out-Null

  for ($attempt = 0; $attempt -lt 4; $attempt++) {
    [GridNative]::MoveWindow($handle, $bounds[0], $bounds[1], $bounds[2], $bounds[3], $true) | Out-Null
    Start-Sleep -Milliseconds 120
  }
}

function Get-TerminalArguments([string]$title, [string]$commandText) {
  if ([string]::IsNullOrWhiteSpace($commandText)) {
    return @(
      "--window", "new",
      "new-tab",
      "--title", $title,
      "cmd.exe"
    )
  }

  return @(
    "--window", "new",
    "new-tab",
    "--title", $title,
    "cmd.exe",
    "/k",
    $commandText
  )
}

$screens = Get-TargetScreens

for ($screenIndex = 0; $screenIndex -lt $screens.Count; $screenIndex++) {
  $screen = $screens[$screenIndex]
  $workArea = $screen.WorkingArea
  $total = $cols * $rows

  for ($index = 0; $index -lt $total; $index++) {
    $title = "grid-$([guid]::NewGuid().ToString('N'))-$screenIndex-$index"
    $arguments = Get-TerminalArguments -title $title -commandText $commandText
    Start-Process -FilePath "wt.exe" -ArgumentList $arguments | Out-Null

    $handle = Wait-ForWindowHandle -titleFragment $title
    $bounds = Get-CellBounds -workArea $workArea -cols $cols -rows $rows -index $index
    Move-TerminalWindow -handle $handle -bounds $bounds
  }
}
