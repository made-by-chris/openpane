#!/usr/bin/env node

const { execFileSync } = require("node:child_process");

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

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function runOsa(language, script) {
  return execFileSync("osascript", ["-l", language, "-e", script], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"]
  }).trim();
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

function getDesktopContext() {
  const script = String.raw`
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
`;

  return JSON.parse(runOsa("JavaScript", script));
}

function normalizeScreens(context) {
  const maxY = Math.max(...context.screens.map((screen) => screen.y + screen.height));

  return context.screens.map((screen) => ({
    x: screen.x,
    y: maxY - (screen.y + screen.height),
    width: screen.width,
    height: screen.height
  }));
}

function pickActiveScreen(context) {
  const screens = normalizeScreens(context);

  if (!context.frontWindow) {
    return { screens, activeScreen: screens[0] };
  }

  const centerX = context.frontWindow.x + Math.floor(context.frontWindow.width / 2);
  const centerY = context.frontWindow.y + Math.floor(context.frontWindow.height / 2);
  const activeScreen = screens.find((screen) => (
    centerX >= screen.x &&
    centerX < screen.x + screen.width &&
    centerY >= screen.y &&
    centerY < screen.y + screen.height
  )) || screens[0];

  return { screens, activeScreen };
}

function toAppleScriptString(value) {
  return String(value).replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function toTerminalCommand(commandText) {
  if (!commandText) {
    return "";
  }

  return commandText;
}

async function launchWindow(commandText, bounds) {
  const script = `
tell application "Terminal"
  activate
  do script "${toAppleScriptString(commandText)}"
  delay 0.25
  set bounds of front window to {${bounds.x}, ${bounds.y}, ${bounds.x + bounds.width}, ${bounds.y + bounds.height}}
end tell
`;

  runOsa("AppleScript", script);

  for (let attempt = 0; attempt < 3; attempt += 1) {
    await sleep(160);
    runOsa("AppleScript", `tell application "Terminal" to set bounds of front window to {${bounds.x}, ${bounds.y}, ${bounds.x + bounds.width}, ${bounds.y + bounds.height}}`);
  }
}

async function main() {
  const { cols, rows, allDisplays, commandText } = decodeConfig();
  const context = getDesktopContext();

  if (!Array.isArray(context.screens) || context.screens.length === 0) {
    fail("No screens reported by macOS.");
  }

  const screenInfo = pickActiveScreen(context);
  const targets = allDisplays ? screenInfo.screens : [screenInfo.activeScreen];
  const terminalCommand = toTerminalCommand(commandText);

  for (const screen of targets) {
    const total = cols * rows;

    for (let index = 0; index < total; index += 1) {
      const bounds = getCellBounds(screen, cols, rows, index);
      await launchWindow(terminalCommand, bounds);
    }
  }
}

main().catch((error) => {
  fail(error.message);
});
