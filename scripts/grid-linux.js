#!/usr/bin/env node

const { execFileSync, spawn } = require("node:child_process");

function fail(message) {
  console.error(message);
  process.exit(1);
}

function decodeConfig() {
  const encoded = process.argv[2] || "";

  if (!encoded) {
    fail("Missing encoded configuration.");
  }

  let config;

  try {
    config = JSON.parse(Buffer.from(encoded, "base64").toString("utf8"));
  } catch {
    fail("Invalid encoded configuration.");
  }

  const cols = Number.parseInt(String(config.cols), 10);
  const rows = Number.parseInt(String(config.rows), 10);

  if (!Number.isInteger(cols) || cols < 1) {
    fail("`cols` must be a positive integer.");
  }

  if (!Number.isInteger(rows) || rows < 1) {
    fail("`rows` must be a positive integer.");
  }

  return {
    cols,
    rows,
    allDisplays: Boolean(config.allDisplays),
    commandText: typeof config.commandText === "string" ? config.commandText.trim() : ""
  };
}

function run(command, args) {
  return execFileSync(command, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"]
  }).trim();
}

function shellQuote(value) {
  return `'${String(value).replace(/'/g, `'"'"'`)}'`;
}

function commandExists(command) {
  try {
    execFileSync("sh", ["-lc", `command -v ${shellQuote(command)} >/dev/null 2>&1`], {
      stdio: "ignore"
    });
    return true;
  } catch {
    return false;
  }
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function getCellBounds(workArea, cols, rows, index) {
  const row = Math.floor(index / cols);
  const col = index % cols;
  const cellWidth = Math.floor(workArea.width / cols);
  const cellHeight = Math.floor(workArea.height / rows);
  const x = workArea.x + (col * cellWidth);
  const y = workArea.y + (row * cellHeight);
  const width = col === cols - 1 ? workArea.x + workArea.width - x : cellWidth;
  const height = row === rows - 1 ? workArea.y + workArea.height - y : cellHeight;

  return { x, y, width, height };
}

function parseShellGeometry(output) {
  const result = {};

  for (const line of output.split(/\r?\n/)) {
    const [key, value] = line.split("=");
    if (key && value) {
      result[key] = Number.parseInt(value, 10);
    }
  }

  return result;
}

function getDisplays() {
  const activeWindowId = run("xdotool", ["getactivewindow"]);
  const geometry = parseShellGeometry(run("xdotool", ["getwindowgeometry", "--shell", activeWindowId]));
  const centerX = geometry.X + Math.floor(geometry.WIDTH / 2);
  const centerY = geometry.Y + Math.floor(geometry.HEIGHT / 2);
  const xrandrOutput = run("xrandr", ["--query"]);
  const displays = [];

  for (const line of xrandrOutput.split(/\r?\n/)) {
    const match = line.match(/ connected(?: primary)? (\d+)x(\d+)\+(\d+)\+(\d+)/);
    if (!match) {
      continue;
    }

    displays.push({
      width: Number.parseInt(match[1], 10),
      height: Number.parseInt(match[2], 10),
      x: Number.parseInt(match[3], 10),
      y: Number.parseInt(match[4], 10)
    });
  }

  if (displays.length === 0) {
    fail("No displays reported by xrandr.");
  }

  const activeDisplay = displays.find((display) => (
    centerX >= display.x &&
    centerX < display.x + display.width &&
    centerY >= display.y &&
    centerY < display.y + display.height
  )) || displays[0];

  return { displays, activeDisplay };
}

function pickTerminal() {
  const terminals = [
    {
      command: "gnome-terminal",
      buildArgs: (title, shellCommand) => ["--title", title, "--", "bash", "-lc", shellCommand]
    },
    {
      command: "xfce4-terminal",
      buildArgs: (title, shellCommand) => ["--title", title, "--command", `bash -lc ${shellQuote(shellCommand)}`]
    },
    {
      command: "konsole",
      buildArgs: (title, shellCommand) => ["--title", title, "-e", "bash", "-lc", shellCommand]
    },
    {
      command: "kitty",
      buildArgs: (title, shellCommand) => ["--title", title, "bash", "-lc", shellCommand]
    },
    {
      command: "alacritty",
      buildArgs: (title, shellCommand) => ["--title", title, "-e", "bash", "-lc", shellCommand]
    },
    {
      command: "xterm",
      buildArgs: (title, shellCommand) => ["-T", title, "-e", "bash", "-lc", shellCommand]
    }
  ];

  return terminals.find((terminal) => commandExists(terminal.command));
}

async function waitForWindowId(title) {
  const deadline = Date.now() + 15000;

  while (Date.now() < deadline) {
    try {
      const lines = run("wmctrl", ["-l"]).split(/\r?\n/);
      const match = lines.find((line) => line.includes(title));

      if (match) {
        return match.split(/\s+/)[0];
      }
    } catch {
      // keep polling
    }

    await sleep(100);
  }

  throw new Error(`Timed out waiting for terminal window '${title}'.`);
}

function moveWindow(windowId, bounds) {
  try {
    execFileSync("wmctrl", ["-i", "-r", windowId, "-b", "remove,maximized_vert,maximized_horz"], {
      stdio: "ignore"
    });
  } catch {
    // ignore; some WMs do not support this hint change
  }

  execFileSync("wmctrl", [
    "-i",
    "-r",
    windowId,
    "-e",
    `0,${bounds.x},${bounds.y},${bounds.width},${bounds.height}`
  ], {
    stdio: "ignore"
  });
}

function buildShellCommand(commandText) {
  if (!commandText) {
    return "exec " + '${SHELL:-/bin/bash} -i';
  }

  return `${commandText}; exec ` + '${SHELL:-/bin/bash} -i';
}

async function main() {
  const { cols, rows, allDisplays, commandText } = decodeConfig();

  for (const command of ["xdotool", "xrandr", "wmctrl"]) {
    if (!commandExists(command)) {
      fail(`Required command not found on PATH: ${command}`);
    }
  }

  const terminal = pickTerminal();

  if (!terminal) {
    fail("No supported terminal emulator found. Tried gnome-terminal, xfce4-terminal, konsole, kitty, alacritty, and xterm.");
  }

  const displayInfo = getDisplays();
  const targets = allDisplays ? displayInfo.displays : [displayInfo.activeDisplay];
  const shellCommand = buildShellCommand(commandText);

  for (let displayIndex = 0; displayIndex < targets.length; displayIndex += 1) {
    const display = targets[displayIndex];
    const total = cols * rows;

    for (let index = 0; index < total; index += 1) {
      const title = `grid-${process.pid}-${displayIndex}-${index}-${Date.now()}`;
      const bounds = getCellBounds(display, cols, rows, index);

      spawn(terminal.command, terminal.buildArgs(title, shellCommand), {
        detached: true,
        stdio: "ignore"
      }).unref();

      const windowId = await waitForWindowId(title);

      for (let attempt = 0; attempt < 4; attempt += 1) {
        moveWindow(windowId, bounds);
        await sleep(120);
      }
    }
  }
}

main().catch((error) => {
  fail(error.message);
});
