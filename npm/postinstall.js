#!/usr/bin/env node
"use strict";

const fs = require("fs");
const https = require("https");
const os = require("os");
const path = require("path");

const pkg = require("../package.json");
const version = pkg.version;
const binDir = path.join(__dirname, "bin");
const target = platformTarget(process.platform, process.arch);

if (process.argv.includes("--check")) {
  checkPackage();
  process.exit(0);
}

install().catch((error) => {
  console.error(`\nInstall failed: ${error.message}`);
  process.exit(1);
});

async function install() {
  const stop = spinner("Installing Gorsee Code");
  fs.mkdirSync(binDir, { recursive: true });

  const url = `https://github.com/OlegGorsky/gorsee-code/releases/download/v${version}/${target.asset}`;
  const output = path.join(binDir, target.exe);
  await download(url, output);
  if (process.platform !== "win32") {
    fs.chmodSync(output, 0o755);
    ensureCommandInPath();
  }
  stop("OK");
}

function platformTarget(platform, arch) {
  const cpu = { x64: "x64", arm64: "arm64" }[arch];
  if (!cpu) {
    throw new Error(`unsupported CPU: ${arch}`);
  }
  if (platform === "linux") {
    return { asset: `gcode-linux-${cpu}`, exe: "gcode" };
  }
  if (platform === "darwin") {
    return { asset: `gcode-darwin-${cpu}`, exe: "gcode" };
  }
  if (platform === "win32" && cpu === "x64") {
    return { asset: "gcode-windows-x64.exe", exe: "gcode.exe" };
  }
  throw new Error(`unsupported platform: ${platform}-${arch}`);
}

function checkPackage() {
  assert(pkg.bin && pkg.bin.gcode === "npm/gcode.js", "package bin.gcode must be npm/gcode.js");
  assert(target.asset && target.exe, "platform target is incomplete");
  assertFile("npm/gcode.js");
  assertFile("npm/postinstall.js");
  for (const file of pkg.files || []) {
    assertFile(file);
  }
  console.log(`asset=${target.asset} bin=${pkg.bin.gcode} files=${(pkg.files || []).length}`);
}

function assertFile(file) {
  assert(fs.existsSync(path.join(__dirname, "..", file)), `missing package file: ${file}`);
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function download(url, output, redirects = 0) {
  if (redirects > 5) {
    return Promise.reject(new Error("too many redirects"));
  }

  return new Promise((resolve, reject) => {
    https
      .get(url, (response) => {
        if (isRedirect(response.statusCode)) {
          response.resume();
          resolve(download(new URL(response.headers.location, url), output, redirects + 1));
          return;
        }

        if (response.statusCode !== 200) {
          response.resume();
          reject(new Error(`download failed with HTTP ${response.statusCode}`));
          return;
        }

        const file = fs.createWriteStream(output);
        response.pipe(file);
        file.on("finish", () => file.close(resolve));
        file.on("error", reject);
      })
      .on("error", reject);
  });
}

function isRedirect(statusCode) {
  return [301, 302, 303, 307, 308].includes(statusCode);
}

function ensureCommandInPath() {
  if (process.env.npm_config_global !== "true") {
    return;
  }

  const pathDirs = (process.env.PATH || "")
    .split(path.delimiter)
    .filter(Boolean)
    .map((dir) => path.resolve(dir));
  const npmBin = path.resolve(process.env.npm_config_prefix || "", "bin");
  if (pathDirs.includes(npmBin)) {
    return;
  }

  const linkDir = pathDirs.find(isUserWritableDir);
  if (!linkDir) {
    return;
  }

  const link = path.join(linkDir, "gcode");
  if (fs.existsSync(link)) {
    return;
  }

  try {
    fs.symlinkSync(path.join(__dirname, "gcode.js"), link);
  } catch {}
}

function isUserWritableDir(dir) {
  const home = path.resolve(os.homedir());
  if (!dir.startsWith(home + path.sep)) {
    return false;
  }

  try {
    if (!fs.statSync(dir).isDirectory()) {
      return false;
    }
    fs.accessSync(dir, fs.constants.W_OK);
    return true;
  } catch {
    return false;
  }
}

function spinner(label) {
  if (!process.stdout.isTTY) {
    console.log(`${label}...`);
    return (status) => console.log(`${status} ${label}`);
  }

  const frames = ["-", "\\", "|", "/"];
  let index = 0;
  const timer = setInterval(() => {
    process.stdout.write(`\r${frames[index++ % frames.length]} ${label}`);
  }, 80);

  return (status) => {
    clearInterval(timer);
    process.stdout.write(`\r${status} ${label}\n`);
  };
}
