// Puts Microsoft's ConPTY (a pinned release, hash-checked) in src-tauri/conpty; the app bundles
// it as conpty/ and loads it instead of the older one built into Windows. Windows only (it uses
// Windows' tar.exe to unpack the package).
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

const PACKAGE = {
  url: "https://github.com/microsoft/terminal/releases/download/v1.24.11911.0/Microsoft.Windows.Console.ConPTY.1.24.260710001.nupkg",
  sha256: "9382ad7becb7e4d84e300578d8e4f4df28f43d979d9055d978c42913c47e0e9d",
};
const FILES = [
  {
    source: "runtimes/win-x64/native/conpty.dll",
    destination: "conpty.dll",
    sha256: "39fba2713e2495117b1591ae8c32a3b904bea7aa66069cf7815e2844c76d75d8",
  },
  {
    source: "build/native/runtimes/x64/OpenConsole.exe",
    destination: "x64/OpenConsole.exe",
    sha256: "b7fd936c2668b87b9ecf7b3366dc6568afc1c6f981874cba3e955a1c35cf8160",
  },
  // conpty.dll picks the host by the machine's architecture, so x64 Solace on ARM needs this one.
  {
    source: "build/native/runtimes/arm64/OpenConsole.exe",
    destination: "arm64/OpenConsole.exe",
    sha256: "ed7622fd0d3bedc9ab9f122f5e58edf0def9e7999224f52dd395ba9f54edbe09",
  },
];
const OUT = "src-tauri/conpty";

const sha256 = (data) => createHash("sha256").update(data).digest("hex");
const present = (file) => {
  const path = join(OUT, file.destination);
  return existsSync(path) && sha256(readFileSync(path)) === file.sha256;
};

if (!FILES.every(present)) {
  const response = await fetch(PACKAGE.url);
  if (!response.ok) throw new Error(`ConPTY download failed: HTTP ${response.status}`);
  const nupkg = Buffer.from(await response.arrayBuffer());
  if (sha256(nupkg) !== PACKAGE.sha256) throw new Error("ConPTY package hash mismatch");
  const work = mkdtempSync(join(tmpdir(), "solace-conpty-"));
  try {
    writeFileSync(join(work, "conpty.zip"), nupkg);
    // Windows' own bsdtar reads zip files; Git Bash's GNU tar does not.
    execFileSync(
      join(process.env.SystemRoot ?? "C:\\Windows", "System32", "tar.exe"),
      ["-xf", "conpty.zip"],
      {
        cwd: work,
      },
    );
    for (const file of FILES) {
      const data = readFileSync(join(work, file.source));
      if (sha256(data) !== file.sha256) throw new Error(`${file.source}: hash mismatch`);
      mkdirSync(dirname(join(OUT, file.destination)), { recursive: true });
      writeFileSync(join(OUT, file.destination), data);
    }
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
}
for (const name of ["LICENSE.txt", "NOTICE.md"])
  copyFileSync(join("packaging/conpty", name), join(OUT, name));
