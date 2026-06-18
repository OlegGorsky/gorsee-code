#!/usr/bin/env node
"use strict";

const fs = require("fs");
const https = require("https");
const path = require("path");

const version = require("../package.json").version;
const binDir = path.join(__dirname, "bin");
const target = platformTarget(process.platform, process.arch);

if (process.argv.includes("--check")) {
  console.log(`asset=${target.asset}`);
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
