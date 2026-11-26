// Builds the `solace` CLI into src-tauri/binaries, which the bundle installs as bin/solace(.exe);
// the app puts that folder on PATH for its shells. On Windows, also fetches the bundled ConPTY.
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";

if (process.platform === "win32") await import("./fetch-conpty.mjs");

const release = process.argv[2] === "release";
execFileSync(
  "cargo",
  ["build", "--locked", "-p", "solace-cli", ...(release ? ["--release"] : [])],
  { stdio: "inherit" },
);
const targetDir = process.env.CARGO_TARGET_DIR ?? "target";
const exe = process.platform === "win32" ? "solace.exe" : "solace";
mkdirSync("src-tauri/binaries", { recursive: true });
copyFileSync(join(targetDir, release ? "release" : "debug", exe), join("src-tauri/binaries", exe));
