// Prepares a release: sets the version everywhere, commits and tags it.
//
//   npm run release -- 1.0.1
//   git push origin main --follow-tags
//
// Pushing the tag starts the "Build installers" workflow, which publishes
// the release that installed copies of DropLink update from.
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2]?.replace(/^v/, "");
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error("usage: npm run release -- <major.minor.patch>");
  process.exit(1);
}

const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
if (git("status", "--porcelain")) {
  console.error("Commit or stash your changes first.");
  process.exit(1);
}

const edit = (file, from, to) => {
  const text = readFileSync(file, "utf8");
  const next = text.replace(from, to);
  if (next === text) throw new Error(`version not found in ${file}`);
  writeFileSync(file, next);
};

edit("package.json", /("version":\s*")[^"]+(")/, `$1${version}$2`);
edit("src-tauri/tauri.conf.json", /("version":\s*")[^"]+(")/, `$1${version}$2`);
edit("Cargo.toml", /(\[workspace\.package\][^[]*?version\s*=\s*")[^"]+(")/, `$1${version}$2`);
execFileSync("npm", ["install", "--package-lock-only", "--ignore-scripts"], { stdio: "inherit", shell: true });
execFileSync("cargo", ["update", "--workspace"], { stdio: "inherit" });

git("add", "package.json", "package-lock.json", "src-tauri/tauri.conf.json", "Cargo.toml", "Cargo.lock");
git("commit", "-m", `DropLink ${version}`);
git("tag", "-a", `v${version}`, "-m", `DropLink ${version}`);
console.log(`\nTagged v${version}. Publish it with:\n  git push origin main --follow-tags`);
