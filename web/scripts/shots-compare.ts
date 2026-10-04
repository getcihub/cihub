// Takes the screenshots of the build of a git ref ("before") and of the working
// tree ("after") with the scenarios of the working tree, then writes
// .screenshots/compare.html, which shows them side by side.
//
// Usage: node scripts/shots-compare.ts [ref], `main` by default.

import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const ref = process.argv[2] ?? "main";
const web = resolve(import.meta.dirname, "..");
const out = join(web, ".screenshots");
const before = join(out, "before");
const after = join(out, "after");

function run(command: string, args: string[], cwd: string, env: Record<string, string> = {}) {
  execFileSync(command, args, { cwd, stdio: "inherit", env: { ...process.env, ...env } });
}

rmSync(before, { recursive: true, force: true });
rmSync(after, { recursive: true, force: true });

const worktree = mkdtempSync(join(tmpdir(), "cihub-shots-"));
run("git", ["worktree", "add", "--detach", worktree, ref], web);
try {
  const baseWeb = join(worktree, "web");
  // A ref without the dashboard has no "before": each screenshot is "added".
  if (existsSync(join(baseWeb, "package.json"))) {
    run("pnpm", ["install", "--frozen-lockfile"], baseWeb);
    run("pnpm", ["exec", "vite", "build"], baseWeb);
    run("pnpm", ["run", "shots"], web, { SHOTS_DIST: join(baseWeb, "dist"), SHOTS_DIR: before });
  }
} finally {
  run("git", ["worktree", "remove", "--force", worktree], web);
}

run("pnpm", ["run", "build"], web);
run("pnpm", ["run", "shots"], web, { SHOTS_DIR: after });

function shots(dir: string): string[] {
  return existsSync(dir) ? readdirSync(dir).filter((name) => name.endsWith(".png")) : [];
}

function status(name: string): string {
  const old = join(before, name);
  const now = join(after, name);
  if (!existsSync(old)) return "added";
  if (!existsSync(now)) return "removed";
  return readFileSync(old).equals(readFileSync(now)) ? "unchanged" : "changed";
}

const names = [...new Set([...shots(before), ...shots(after)])].sort();
const rows = names.map((name) => {
  const cell = (dir: string) =>
    existsSync(join(out, dir, name)) ? `<img src="${dir}/${name}" alt="${dir} ${name}">` : "";
  return `<h2>${name} <small>${status(name)}</small></h2>
<div class="pair"><div>${cell("before")}</div><div>${cell("after")}</div></div>`;
});

writeFileSync(
  join(out, "compare.html"),
  `<!doctype html>
<meta charset="utf-8">
<title>Screenshots: ${ref} against the working tree</title>
<style>
  body { font-family: system-ui, sans-serif; margin: 1.5rem; }
  .pair { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  img { width: 100%; border: 1px solid #d1d9e0; }
  small { font-weight: normal; color: #59636e; }
</style>
<h1>Before: ${ref}, after: the working tree</h1>
${rows.join("\n")}
`,
);
process.stdout.write(`${join(out, "compare.html")}\n`);
