use std::ffi::OsString;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "macos")]
use serde::Deserialize;

#[derive(Clone, Debug)]
struct Config {
    cols: usize,
    rows: usize,
    all_displays: bool,
    command_text: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("openpane: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let executable_name = std::env::args_os()
        .next()
        .and_then(|arg| {
            std::path::Path::new(&arg)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "openpane".to_string());

    let config = parse_args(&executable_name)?;

    #[cfg(target_os = "windows")]
    {
        return windows::launch(&config);
    }

    #[cfg(target_os = "linux")]
    {
        return linux::launch(&config);
    }

    #[cfg(target_os = "macos")]
    {
        return macos::launch(&config);
    }

    #[allow(unreachable_code)]
    Err(format!("unsupported platform: {}", std::env::consts::OS))
}

fn parse_args(executable_name: &str) -> Result<Config, String> {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|value| value.to_string_lossy().into_owned())
        .collect();

    if args.len() < 2 {
        return Err(usage(executable_name));
    }

    let cols = args[0].parse::<usize>().map_err(|_| {
        format!(
            "`cols` must be a positive integer.\n\n{}",
            usage(executable_name)
        )
    })?;
    let rows = args[1].parse::<usize>().map_err(|_| {
        format!(
            "`rows` must be a positive integer.\n\n{}",
            usage(executable_name)
        )
    })?;

    if cols == 0 {
        return Err(format!(
            "`cols` must be a positive integer.\n\n{}",
            usage(executable_name)
        ));
    }
    if rows == 0 {
        return Err(format!(
            "`rows` must be a positive integer.\n\n{}",
            usage(executable_name)
        ));
    }

    let mut command_start = 2;
    let mut all_displays = false;
    if args.get(2).is_some_and(|value| value == "*") {
        all_displays = true;
        command_start = 3;
    }

    let command_text = args[command_start..].join(" ").trim().to_string();
    let command_text = if command_text.is_empty() {
        None
    } else {
        Some(command_text)
    };

    Ok(Config {
        cols,
        rows,
        all_displays,
        command_text,
    })
}

fn usage(executable_name: &str) -> String {
    format!(
        "Usage: {executable_name} <cols> <rows> [*] [command...]\n\nExamples:\n  {executable_name} 4 2 claude\n  {executable_name} 3 3\n  {executable_name} 3 1 opencode\n  {executable_name} 2 1 * claude"
    )
}

fn get_cell_bounds(work_area: Rect, cols: usize, rows: usize, index: usize) -> Rect {
    let row = (index / cols) as i32;
    let col = (index % cols) as i32;
    let cell_width = work_area.width / cols as i32;
    let cell_height = work_area.height / rows as i32;
    let x = work_area.x + (col * cell_width);
    let y = work_area.y + (row * cell_height);
    let width = if col == cols as i32 - 1 {
        (work_area.x + work_area.width) - x
    } else {
        cell_width
    };
    let height = if row == rows as i32 - 1 {
        (work_area.y + work_area.height) - y
    } else {
        cell_height
    };

    Rect {
        x,
        y,
        width,
        height,
    }
}

fn now_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_nanos();
    format!("{}-{}", std::process::id(), nanos)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_capture(command: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(command)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run `{command}`: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(if stderr.trim().is_empty() {
            format!("`{command}` exited with status {}", output.status)
        } else {
            stderr.trim().to_string()
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    #[derive(Clone)]
    struct Display {
        rect: Rect,
    }

    #[derive(Clone, Copy)]
    enum TerminalKind {
        Gnome,
        Xfce,
        Konsole,
        Kitty,
        Alacritty,
        Xterm,
    }

    #[derive(Clone, Copy)]
    struct Terminal {
        command: &'static str,
        kind: TerminalKind,
    }

    pub fn launch(config: &Config) -> Result<(), String> {
        for command in ["xdotool", "xrandr", "wmctrl"] {
            if !command_exists(command) {
                return Err(format!("required command not found on PATH: {command}"));
            }
        }

        let terminal = pick_terminal()
            .ok_or_else(|| "no supported terminal emulator found. Tried gnome-terminal, xfce4-terminal, konsole, kitty, alacritty, and xterm.".to_string())?;

        let (displays, active_display) = get_displays()?;
        let targets = if config.all_displays {
            displays
        } else {
            vec![active_display]
        };
        let shell_command = build_shell_command(config.command_text.as_deref());

        for (display_index, display) in targets.iter().enumerate() {
            let total = config.cols * config.rows;
            for index in 0..total {
                let title = format!("openpane-{}-{display_index}-{index}", now_token());
                let bounds = get_cell_bounds(display.rect, config.cols, config.rows, index);

                Command::new(terminal.command)
                    .args(build_terminal_args(terminal, &title, &shell_command))
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|error| format!("failed to launch {}: {error}", terminal.command))?;

                let window_id = wait_for_window_id(&title)?;
                for _ in 0..4 {
                    move_window(&window_id, bounds)?;
                    thread::sleep(Duration::from_millis(120));
                }
            }
        }

        Ok(())
    }

    fn command_exists(command: &str) -> bool {
        Command::new("sh")
            .args([
                "-lc",
                &format!("command -v {} >/dev/null 2>&1", shell_quote(command)),
            ])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    fn shell_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }

    fn get_displays() -> Result<(Vec<Display>, Display), String> {
        let active_window_id = run_capture("xdotool", &["getactivewindow"])?;
        let geometry = parse_shell_geometry(&run_capture(
            "xdotool",
            &["getwindowgeometry", "--shell", &active_window_id],
        )?)?;
        let center_x = geometry.0 + (geometry.2 / 2);
        let center_y = geometry.1 + (geometry.3 / 2);
        let xrandr_output = run_capture("xrandr", &["--query"])?;

        let mut displays = Vec::new();
        for line in xrandr_output.lines() {
            if !line.contains(" connected") {
                continue;
            }

            let Some(bounds) = line
                .split_whitespace()
                .find(|part| part.contains('x') && part.contains('+'))
            else {
                continue;
            };

            displays.push(Display {
                rect: parse_display_bounds(bounds)?,
            });
        }

        if displays.is_empty() {
            return Err("no displays reported by xrandr".to_string());
        }

        let active = displays
            .iter()
            .find(|display| {
                center_x >= display.rect.x
                    && center_x < display.rect.x + display.rect.width
                    && center_y >= display.rect.y
                    && center_y < display.rect.y + display.rect.height
            })
            .cloned()
            .unwrap_or_else(|| displays[0].clone());

        Ok((displays, active))
    }

    fn parse_shell_geometry(output: &str) -> Result<(i32, i32, i32, i32), String> {
        let mut x = None;
        let mut y = None;
        let mut width = None;
        let mut height = None;

        for line in output.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let parsed = value
                .parse::<i32>()
                .map_err(|_| format!("invalid geometry value: {line}"))?;
            match key {
                "X" => x = Some(parsed),
                "Y" => y = Some(parsed),
                "WIDTH" => width = Some(parsed),
                "HEIGHT" => height = Some(parsed),
                _ => {}
            }
        }

        Ok((
            x.ok_or_else(|| "missing X in xdotool output".to_string())?,
            y.ok_or_else(|| "missing Y in xdotool output".to_string())?,
            width.ok_or_else(|| "missing WIDTH in xdotool output".to_string())?,
            height.ok_or_else(|| "missing HEIGHT in xdotool output".to_string())?,
        ))
    }

    fn parse_display_bounds(bounds: &str) -> Result<Rect, String> {
        let mut first_plus = bounds.splitn(2, '+');
        let size = first_plus
            .next()
            .ok_or_else(|| format!("invalid display bounds: {bounds}"))?;
        let position = first_plus
            .next()
            .ok_or_else(|| format!("invalid display bounds: {bounds}"))?;
        let mut position_parts = position.split('+');
        let x = position_parts
            .next()
            .ok_or_else(|| format!("invalid display X: {bounds}"))?
            .parse::<i32>()
            .map_err(|_| format!("invalid display X: {bounds}"))?;
        let y = position_parts
            .next()
            .ok_or_else(|| format!("invalid display Y: {bounds}"))?
            .parse::<i32>()
            .map_err(|_| format!("invalid display Y: {bounds}"))?;
        let (width, height) = size
            .split_once('x')
            .ok_or_else(|| format!("invalid display size: {bounds}"))?;

        Ok(Rect {
            x,
            y,
            width: width
                .parse::<i32>()
                .map_err(|_| format!("invalid display width: {bounds}"))?,
            height: height
                .parse::<i32>()
                .map_err(|_| format!("invalid display height: {bounds}"))?,
        })
    }

    fn pick_terminal() -> Option<Terminal> {
        let terminals = [
            Terminal {
                command: "gnome-terminal",
                kind: TerminalKind::Gnome,
            },
            Terminal {
                command: "xfce4-terminal",
                kind: TerminalKind::Xfce,
            },
            Terminal {
                command: "konsole",
                kind: TerminalKind::Konsole,
            },
            Terminal {
                command: "kitty",
                kind: TerminalKind::Kitty,
            },
            Terminal {
                command: "alacritty",
                kind: TerminalKind::Alacritty,
            },
            Terminal {
                command: "xterm",
                kind: TerminalKind::Xterm,
            },
        ];

        terminals
            .into_iter()
            .find(|terminal| command_exists(terminal.command))
    }

    fn build_terminal_args(terminal: Terminal, title: &str, shell_command: &str) -> Vec<String> {
        match terminal.kind {
            TerminalKind::Gnome => vec![
                "--title".into(),
                title.into(),
                "--".into(),
                "bash".into(),
                "-lc".into(),
                shell_command.into(),
            ],
            TerminalKind::Xfce => vec![
                "--title".into(),
                title.into(),
                "--command".into(),
                format!("bash -lc {}", shell_quote(shell_command)),
            ],
            TerminalKind::Konsole => vec![
                "--title".into(),
                title.into(),
                "-e".into(),
                "bash".into(),
                "-lc".into(),
                shell_command.into(),
            ],
            TerminalKind::Kitty => vec![
                "--title".into(),
                title.into(),
                "bash".into(),
                "-lc".into(),
                shell_command.into(),
            ],
            TerminalKind::Alacritty => vec![
                "--title".into(),
                title.into(),
                "-e".into(),
                "bash".into(),
                "-lc".into(),
                shell_command.into(),
            ],
            TerminalKind::Xterm => vec![
                "-T".into(),
                title.into(),
                "-e".into(),
                "bash".into(),
                "-lc".into(),
                shell_command.into(),
            ],
        }
    }

    fn build_shell_command(command_text: Option<&str>) -> String {
        match command_text {
            Some(command) => format!("{command}; exec ${{SHELL:-/bin/bash}} -i"),
            None => "exec ${SHELL:-/bin/bash} -i".to_string(),
        }
    }

    fn wait_for_window_id(title: &str) -> Result<String, String> {
        let deadline = SystemTime::now() + Duration::from_secs(15);
        while SystemTime::now() < deadline {
            let output = run_capture("wmctrl", &["-l"]).unwrap_or_default();
            for line in output.lines() {
                if line.contains(title) {
                    if let Some(window_id) = line.split_whitespace().next() {
                        return Ok(window_id.to_string());
                    }
                }
            }
            thread::sleep(Duration::from_millis(100));
        }

        Err(format!("timed out waiting for terminal window `{title}`"))
    }

    fn move_window(window_id: &str, bounds: Rect) -> Result<(), String> {
        let _ = Command::new("wmctrl")
            .args([
                "-i",
                "-r",
                window_id,
                "-b",
                "remove,maximized_vert,maximized_horz",
            ])
            .status();

        let status = Command::new("wmctrl")
            .args([
                "-i",
                "-r",
                window_id,
                "-e",
                &format!(
                    "0,{},{},{},{}",
                    bounds.x, bounds.y, bounds.width, bounds.height
                ),
            ])
            .status()
            .map_err(|error| format!("failed to move window: {error}"))?;

        if status.success() {
            Ok(())
        } else {
            Err("wmctrl failed to move a window".to_string())
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    #[derive(Clone, Deserialize)]
    struct ScreenRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    #[derive(Clone, Deserialize)]
    struct DesktopContext {
        screens: Vec<ScreenRect>,
        #[serde(rename = "frontWindow")]
        front_window: Option<ScreenRect>,
    }

    pub fn launch(config: &Config) -> Result<(), String> {
        let context = get_desktop_context()?;
        if context.screens.is_empty() {
            return Err("no screens reported by macOS".to_string());
        }

        let (screens, active_screen) = pick_active_screen(&context);
        let targets = if config.all_displays {
            screens
        } else {
            vec![active_screen]
        };

        for screen in targets {
            let total = config.cols * config.rows;
            for index in 0..total {
                let bounds = get_cell_bounds(screen, config.cols, config.rows, index);
                launch_window(config.command_text.as_deref(), bounds)?;
            }
        }

        Ok(())
    }

    fn run_osascript(language: &str, script: &str) -> Result<String, String> {
        run_capture("osascript", &["-l", language, "-e", script])
    }

    fn get_desktop_context() -> Result<DesktopContext, String> {
        let script = r#"
ObjC.import('AppKit');

function rectToObject(rect) {
  return {
    x: Number(rect.origin.x),
    y: Number(rect.origin.y),
    width: Number(rect.size.width),
    height: Number(rect.size.height)
  };
}

const systemEvents = Application('System Events');
const frontmostProcess = systemEvents.applicationProcesses.whose({ frontmost: true })[0];
let frontWindow = null;

try {
  const window = frontmostProcess.windows[0];
  const position = window.position();
  const size = window.size();
  frontWindow = {
    x: Number(position[0]),
    y: Number(position[1]),
    width: Number(size[0]),
    height: Number(size[1])
  };
} catch (error) {
  frontWindow = null;
}

const screens = [];
const allScreens = $.NSScreen.screens;
const screenCount = allScreens.count;

for (let index = 0; index < screenCount; index += 1) {
  const screen = allScreens.objectAtIndex(index);
  screens.push(rectToObject(screen.visibleFrame));
}

JSON.stringify({ screens, frontWindow });
"#;

        serde_json::from_str(&run_osascript("JavaScript", script)?)
            .map_err(|error| format!("failed to parse macOS screen context: {error}"))
    }

    fn normalize_screens(context: &DesktopContext) -> Vec<Rect> {
        let max_y = context
            .screens
            .iter()
            .map(|screen| screen.y + screen.height)
            .max()
            .unwrap_or(0);

        context
            .screens
            .iter()
            .map(|screen| Rect {
                x: screen.x,
                y: max_y - (screen.y + screen.height),
                width: screen.width,
                height: screen.height,
            })
            .collect()
    }

    fn pick_active_screen(context: &DesktopContext) -> (Vec<Rect>, Rect) {
        let screens = normalize_screens(context);
        let active = context.front_window.as_ref().and_then(|window| {
            let center_x = window.x + (window.width / 2);
            let center_y = window.y + (window.height / 2);
            screens
                .iter()
                .find(|screen| {
                    center_x >= screen.x
                        && center_x < screen.x + screen.width
                        && center_y >= screen.y
                        && center_y < screen.y + screen.height
                })
                .copied()
        });

        let active = active.unwrap_or(screens[0]);
        (screens, active)
    }

    fn apple_escape(value: &str) -> String {
        value.replace('\\', "\\\\").replace('"', "\\\"")
    }

    fn launch_window(command_text: Option<&str>, bounds: Rect) -> Result<(), String> {
        let script = format!(
            "tell application \"Terminal\"\n  activate\n  do script \"{}\"\n  delay 0.25\n  set bounds of front window to {{{}, {}, {}, {}}}\nend tell",
            apple_escape(command_text.unwrap_or("")),
            bounds.x,
            bounds.y,
            bounds.x + bounds.width,
            bounds.y + bounds.height
        );
        run_osascript("AppleScript", &script)?;

        for _ in 0..3 {
            thread::sleep(Duration::from_millis(160));
            let retry = format!(
                "tell application \"Terminal\" to set bounds of front window to {{{}, {}, {}, {}}}",
                bounds.x,
                bounds.y,
                bounds.x + bounds.width,
                bounds.y + bounds.height
            );
            run_osascript("AppleScript", &retry)?;
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use super::*;
    use std::mem::{size_of, zeroed};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFOEXW,
        MONITOR_DEFAULTTOPRIMARY,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
        MoveWindow, ShowWindow, SW_RESTORE,
    };

    const MONITORINFOF_PRIMARY: u32 = 1;

    #[derive(Clone, Copy)]
    struct Display {
        monitor: HMONITOR,
        work_area: Rect,
        primary: bool,
    }

    struct WindowSearch {
        title_fragment: String,
        found: Option<HWND>,
    }

    pub fn launch(config: &Config) -> Result<(), String> {
        let displays = enumerate_displays()?;
        let targets = if config.all_displays {
            displays
        } else {
            vec![active_display(&displays)]
        };

        for (screen_index, display) in targets.iter().enumerate() {
            let total = config.cols * config.rows;
            for index in 0..total {
                let title = format!("openpane-{}-{screen_index}-{index}", now_token());
                let bounds = get_cell_bounds(display.work_area, config.cols, config.rows, index);

                Command::new("wt.exe")
                    .args(build_wt_args(&title, config.command_text.as_deref()))
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|error| format!("failed to launch wt.exe: {error}"))?;

                let handle = wait_for_window_handle(&title)?;
                for _ in 0..4 {
                    unsafe {
                        ShowWindow(handle, SW_RESTORE);
                        MoveWindow(handle, bounds.x, bounds.y, bounds.width, bounds.height, 1);
                    }
                    thread::sleep(Duration::from_millis(120));
                }
            }
        }

        Ok(())
    }

    fn build_wt_args(title: &str, command_text: Option<&str>) -> Vec<OsString> {
        let mut args = vec![
            OsString::from("--window"),
            OsString::from("new"),
            OsString::from("new-tab"),
            OsString::from("--title"),
            OsString::from(title),
            OsString::from("cmd.exe"),
        ];

        if let Some(command) = command_text {
            args.push(OsString::from("/k"));
            args.push(OsString::from(command));
        }

        args
    }

    fn enumerate_displays() -> Result<Vec<Display>, String> {
        unsafe extern "system" fn callback(
            monitor: HMONITOR,
            _: HDC,
            _: *mut RECT,
            lparam: LPARAM,
        ) -> i32 {
            let displays = unsafe { &mut *(lparam as *mut Vec<Display>) };
            let mut info: MONITORINFOEXW = unsafe { zeroed() };
            info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;

            if unsafe { GetMonitorInfoW(monitor, &mut info as *mut _ as *mut _) } != 0 {
                let work = info.monitorInfo.rcWork;
                displays.push(Display {
                    monitor,
                    work_area: Rect {
                        x: work.left,
                        y: work.top,
                        width: work.right - work.left,
                        height: work.bottom - work.top,
                    },
                    primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
                });
            }

            1
        }

        let mut displays = Vec::new();
        let success = unsafe {
            EnumDisplayMonitors(
                std::ptr::null_mut(),
                std::ptr::null(),
                Some(callback),
                &mut displays as *mut _ as isize,
            )
        };

        if success == 0 || displays.is_empty() {
            return Err("failed to enumerate displays".to_string());
        }

        Ok(displays)
    }

    fn active_display(displays: &[Display]) -> Display {
        let foreground = unsafe { GetForegroundWindow() };
        let monitor = unsafe { MonitorFromWindow(foreground, MONITOR_DEFAULTTOPRIMARY) };
        displays
            .iter()
            .find(|display| display.monitor == monitor)
            .copied()
            .or_else(|| displays.iter().find(|display| display.primary).copied())
            .unwrap_or(displays[0])
    }

    fn wait_for_window_handle(title_fragment: &str) -> Result<HWND, String> {
        let deadline = SystemTime::now() + Duration::from_secs(15);
        while SystemTime::now() < deadline {
            let mut state = WindowSearch {
                title_fragment: title_fragment.to_string(),
                found: None,
            };

            unsafe extern "system" fn callback(window: HWND, lparam: LPARAM) -> i32 {
                let state = unsafe { &mut *(lparam as *mut WindowSearch) };

                if unsafe { IsWindowVisible(window) } == 0 {
                    return 1;
                }

                let length = unsafe { GetWindowTextLengthW(window) };
                if length <= 0 {
                    return 1;
                }

                let mut buffer = vec![0u16; (length + 1) as usize];
                let copied = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), length + 1) };
                if copied <= 0 {
                    return 1;
                }

                let title = String::from_utf16_lossy(&buffer[..copied as usize]);
                if title.contains(&state.title_fragment) {
                    state.found = Some(window);
                    return 0;
                }

                1
            }

            unsafe {
                EnumWindows(Some(callback), &mut state as *mut _ as isize);
            }

            if let Some(handle) = state.found {
                return Ok(handle);
            }

            thread::sleep(Duration::from_millis(100));
        }

        Err(format!(
            "timed out waiting for Windows Terminal window `{title_fragment}`"
        ))
    }
}
