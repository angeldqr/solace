// Writes latest.json, the file installed copies of Solace read to find updates, from the release
// files and their updater signatures (.sig). Written by hand rather than by tauri-action, so
// installers can be Authenticode-signed (and their updater signatures recomputed) first.
//
// Usage: node scripts/release-manifest.mjs <version> <release-dir> <out-file>
// <release-dir> holds every platform's files side by side, as the release workflow collects them.
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [version, dir, out] = process.argv.slice(2);
if (!version || !dir || !out) {
  throw new Error("usage: release-manifest.mjs <version> <release-dir> <out-file>");
}
const repo = process.env.GITHUB_REPOSITORY ?? "angeldqr/solace";
const base = `https://github.com/${repo}/releases/download/v${version}`;
const files = readdirSync(dir);

/** The update bundle ending in `suffix` and its signature; the file name carries the version. */
function entry(suffix) {
  const file = files.find((f) => f.endsWith(suffix) && f.includes(`_${version}_`));
  if (!file) throw new Error(`no ${suffix} for ${version} in ${dir}`);
  const signature = readFileSync(join(dir, `${file}.sig`), "utf8").trim();
  // GitHub replaces spaces in asset names with dots.
  return { signature, url: `${base}/${file.replaceAll(" ", ".")}` };
}

const nsis = entry("-setup.exe");
const msi = entry(".msi");
const appImage = entry(".AppImage");
const app = entry(".app.tar.gz");
const manifest = {
  version,
  notes: `Solace ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    // The plain keys are what older updaters read; the NSIS installer is the recommended one.
    "windows-x86_64": nsis,
    "windows-x86_64-nsis": nsis,
    "windows-x86_64-msi": msi,
    "linux-x86_64": appImage,
    "linux-x86_64-appimage": appImage,
    "darwin-aarch64": app,
    "darwin-aarch64-app": app,
  },
};
writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`wrote ${out}`);
