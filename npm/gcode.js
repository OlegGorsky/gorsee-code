#!/usr/bin/env node
"use strict";

const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const exe = process.platform === "win32" ? "gcode.exe" : "gcode";
const bin = path.join(__dirname, "bin", exe);

if (!fs.existsSync(bin)) {
  console.error("gcode binary is missing. Reinstall with: npm install -g @gorsee/code");
  process.exit(1);
}

const result = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

if (result.signal) {
  process.kill(process.pid, result.signal);
}

process.exit(result.status ?? 1);
