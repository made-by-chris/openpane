#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const path = require("node:path");

const executableName = path.basename(process.argv[1] || "grid", path.extname(process.argv[1] || "")) || "grid";

function printUsage() {
  console.error(`Usage: ${executableName} <cols> <rows> [*] [command...]`);
  console.error("Examples:");
  console.error(`  ${executableName} 4 2 \"claude -p 'hello'\"`);
  console.error(`  ${executableName} 3 1 opencode`);
  console.error(`  ${executableName} 2 1 * \"claude -p 'hello'\"`);
}

function fail(message) {
  console.error(message);
  printUsage();
  process.exit(1);
}

const args = process.argv.slice(2);

if (args.length < 2) {
  printUsage();
  process.exit(1);
}

const cols = Number.parseInt(args[0], 10);
const rows = Number.parseInt(args[1], 10);

if (!Number.isInteger(cols) || cols < 1) {
  fail("`cols` must be a positive integer.");
}

if (!Number.isInteger(rows) || rows < 1) {
  fail("`rows` must be a positive integer.");
}

let commandStartIndex = 2;
let allDisplays = false;

if (args[2] === "*") {
  allDisplays = true;
  commandStartIndex = 3;
}

const commandText = args.slice(commandStartIndex).join(" ").trim();
const scriptsDir = path.join(__dirname, "..", "scripts");
const encodedConfig = Buffer.from(JSON.stringify({
  cols,
  rows,
  allDisplays,
  commandText
}), "utf8").toString("base64");

let executable;
let scriptArgs;

switch (process.platform) {
  case "win32": {
    executable = "powershell.exe";
    scriptArgs = [
      "-NoProfile",
      "-ExecutionPolicy",
      "Bypass",
      "-File",
      path.join(scriptsDir, "grid.ps1"),
      encodedConfig
    ];
    break;
  }
  case "linux": {
    executable = process.execPath;
    scriptArgs = [
      path.join(scriptsDir, "grid-linux.js"),
      encodedConfig
    ];
    break;
  }
  case "darwin": {
    executable = process.execPath;
    scriptArgs = [
      path.join(scriptsDir, "grid-macos.js"),
      encodedConfig
    ];
    break;
  }
  default: {
    console.error(`Unsupported platform: ${process.platform}`);
    process.exit(1);
  }
}

const result = spawnSync(executable, scriptArgs, {
  stdio: "inherit",
  env: process.env
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 0);
