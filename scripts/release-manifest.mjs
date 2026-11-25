// Writes latest.json, the file installed copies of Solace read to find updates, from the bundles
// and their updater signatures (.sig). Replaces what tauri-action generated, so installers can be
// Authenticode-signed (and their updater signatures recomputed) before the manifest is made.
//
// Usage: node scripts/release-manifest.mjs <version> <bundle-dir> <out-file>
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [version, bundleDir, out] = process.argv.slice(2);
if (!version || !bundleDir || !out) {
  throw new Error("usage: release-manifest.mjs <version> <bundle-dir> <out-file>");
}
const repo = process.env.GITHUB_REPOSITORY ?? "angeldqr/solace";
const base = `https://github.com/${repo}/releases/download/v${version}`;

/** The installer and its signature in bundle/<kind>/; the file name carries the version. */
function entry(kind, suffix) {
  const dir = join(bundleDir, kind);
  const file = readdirSync(dir).find((f) => f.endsWith(suffix) && f.includes(`_${version}_`));
  if (!file) throw new Error(`no ${suffix} for ${version} in ${dir}`);
  const signature = readFileSync(join(dir, `${file}.sig`), "utf8").trim();
  // GitHub replaces spaces in asset names with dots.
  return { signature, url: `${base}/${file.replaceAll(" ", ".")}` };
}

const nsis = entry("nsis", "-setup.exe");
const msi = entry("msi", ".msi");
const manifest = {
  version,
  notes: `Solace ${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    // The plain key is what older updaters read; the NSIS installer is the recommended one.
    "windows-x86_64": nsis,
    "windows-x86_64-nsis": nsis,
    "windows-x86_64-msi": msi,
  },
};
writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`wrote ${out}`);
