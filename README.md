# openpane

<p>
  <img src="./assets/openpane-logo.svg" alt="openpane logo" width="120" />
</p>

Open a clean grid of terminal windows on the current display or every display, then run the same command in each one.

Homepage: <https://made-by-chris.github.io/openpane/>

## Install

These installers use GitHub Releases and native binaries. No npm. No Node runtime.

### Windows

```powershell
irm https://raw.githubusercontent.com/made-by-chris/openpane/main/scripts/install.ps1 | iex
openpane 2 2 claude
```

### macOS

```bash
curl -fsSL https://raw.githubusercontent.com/made-by-chris/openpane/main/scripts/install.sh | sh
openpane 2 2 claude
```

### Linux

```bash
curl -fsSL https://raw.githubusercontent.com/made-by-chris/openpane/main/scripts/install.sh | sh
openpane 2 2 claude
```

## Usage

```bash
openpane <x-axis cells> <y-axis cells> [*] [command...]
```

- `openpane 4 2 "claude -p 'hello'"` opens a 4-by-2 grid on the active display
- `openpane 3 3` opens a 3-by-3 grid of interactive terminals
- `openpane 1 2` opens two vertically stacked terminals
- `openpane 3 1 opencode` opens three side-by-side terminals running `opencode`
- `openpane 2 1 * "claude -p 'hello'"` opens a 2-by-1 grid on every display

## Platform notes

- Windows uses Windows Terminal via `wt.exe`
- macOS uses Terminal.app via `osascript`
- Linux uses `wmctrl`, `xdotool`, `xrandr`, and the first available terminal from `gnome-terminal`, `xfce4-terminal`, `konsole`, `kitty`, `alacritty`, or `xterm`

## Local development

```bash
cargo run -- 2 2 claude
```

## Release install behavior

- Unix installs the native binary to `~/.local/share/openpane/<version>` and writes shims to `~/.local/bin`
- Windows installs the native binary to `%LOCALAPPDATA%\openpane\<version>` and writes shims to `%USERPROFILE%\.openpane\bin`
- The installers pull the latest GitHub release by default

## Build from source

```bash
cargo build --release
```
