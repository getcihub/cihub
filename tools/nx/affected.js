// Prints the crates that the CI checks, as `key=value` lines:
//   crates: the cargo arguments that select them, empty when no crate changed;
//   core:   `true` when a crate of the core (tag `core`) is among them.
// NX_BASE names the commit to compare with (Nx reads it, and NX_HEAD); without
// it, every crate is selected.
//
// Nx finds the crates whose files changed. It reads the lock files of
// JavaScript package by package, but not Cargo.lock, so this script reads the
// files of the workspace that Nx sees as a whole:
//   Cargo.lock: a crate is touched when the set of packages it reaches changes;
//   Cargo.toml: a line of [workspace.dependencies] touches the crates that use
//     the dependency; any other change touches every crate;
//   mise.toml: a change touches every crate, except a change to a tool that
//     does not build or check Rust.
// Nx then adds the crates that depend on the touched ones.
const { execFileSync } = require("node:child_process");
const { readFileSync } = require("node:fs");
const { relative } = require("node:path");

const NON_RUST_TOOLS = new Set(["actionlint", "aqua:pnpm/pnpm", "lefthook", "node", "zizmor"]);

const run = (command, args) =>
  execFileSync(command, args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });

function projects(...args) {
  return JSON.parse(run("pnpm", ["--silent", "exec", "nx", "show", "projects", "--json", ...args]));
}

function workspaceCrates() {
  const metadata = JSON.parse(run("cargo", ["metadata", "--no-deps", "--format-version", "1"]));
  return metadata.packages.map((pkg) => ({
    ...pkg,
    manifest: relative(metadata.workspace_root, pkg.manifest_path),
  }));
}

// The content of a file at the base and at the head of the comparison, like
// Nx: from the merge base, to NX_HEAD or else the working tree.
function contents(base, file) {
  const head = process.env.NX_HEAD;
  return [
    run("git", ["show", `${base}:${file}`]),
    head ? run("git", ["show", `${head}:${file}`]) : readFileSync(file, "utf8"),
  ];
}

// The lines of a TOML file with their section, without the empty lines and
// the comments.
function tomlLines(text) {
  let section = "";
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "" && !line.startsWith("#"))
    .map((line) => {
      if (line.startsWith("[")) {
        section = line;
      }
      return { section, line, key: line.split("=")[0].trim().replace(/^"|"$/g, "") };
    });
}

const differ = (a, b) => a.join("\n") !== b.join("\n");

// Every crate changes with mise.toml, unless only a tool without a role in the
// build or the checks of the Rust code changed.
function miseTouchesAll(base) {
  const kept = (text) =>
    tomlLines(text)
      .filter(
        ({ section, key, line }) =>
          !(section === "[tools]" && line !== section && NON_RUST_TOOLS.has(key)),
      )
      .map(({ line }) => line);
  const [before, after] = contents(base, "mise.toml");
  return differ(kept(before), kept(after));
}

// The root Cargo.toml: `all` when a line outside [workspace.dependencies]
// changed, and the names of the dependencies whose line changed.
function cargoTomlChanges(base) {
  const [before, after] = contents(base, "Cargo.toml").map(tomlLines);
  const outside = (lines) =>
    lines.filter(({ section }) => section !== "[workspace.dependencies]").map(({ line }) => line);
  const deps = (lines) =>
    new Map(
      lines
        .filter(({ section, line }) => section === "[workspace.dependencies]" && line !== section)
        .map(({ key, line }) => [key, line]),
    );
  const [depsBefore, depsAfter] = [deps(before), deps(after)];
  const names = new Set([...depsBefore.keys(), ...depsAfter.keys()]);
  return {
    all: differ(outside(before), outside(after)),
    deps: [...names].filter((name) => depsBefore.get(name) !== depsAfter.get(name)),
  };
}

// For a package of Cargo.lock, by name: the sorted [[package]] blocks
// it reaches, itself included. A block carries the version, the source, the
// checksum and the dependencies, so a change to any of them shows.
function lockClosures(text) {
  const packages = text
    .split("[[package]]")
    .slice(1)
    .map((block) => ({
      name: /^name = "(.*)"$/m.exec(block)[1],
      version: /^version = "(.*)"$/m.exec(block)[1],
      block: block.trim(),
      deps: [
        ...(/^dependencies = \[([^\]]*)\]/m.exec(block)?.[1] ?? "").matchAll(/"([^"]+)"/g),
      ].map((m) => m[1]),
    }));
  // A dependency is "name", or "name version" when several versions exist.
  const find = (dep) => {
    const [name, version] = dep.split(" ");
    return packages.find((pkg) => pkg.name === name && (!version || pkg.version === version));
  };
  // No memoization: the dev-dependencies between crates of the workspace can
  // form cycles, and a cycle leaves a memoized set incomplete.
  return (name) => {
    const start = packages.find((pkg) => pkg.name === name);
    const reached = new Set();
    const pending = start ? [start] : [];
    while (pending.length > 0) {
      const pkg = pending.pop();
      if (!reached.has(pkg)) {
        reached.add(pkg);
        pending.push(...pkg.deps.map(find));
      }
    }
    return [...reached].map((pkg) => pkg.block).sort();
  };
}

// The crates touched by the files that Nx sees as a whole, or `all`.
function touchedByWorkspaceFiles(base) {
  if (miseTouchesAll(base)) {
    return "all";
  }
  const cargoToml = cargoTomlChanges(base);
  if (cargoToml.all) {
    return "all";
  }
  const crates = workspaceCrates();
  const [lockBefore, lockAfter] = contents(base, "Cargo.lock").map(lockClosures);
  return crates
    .filter(
      (crate) =>
        crate.dependencies.some((dep) => cargoToml.deps.includes(dep.name)) ||
        differ(lockBefore(crate.name), lockAfter(crate.name)),
    )
    .map((crate) => crate.manifest);
}

function selectedCrates() {
  if (!process.env.NX_BASE) {
    return projects("--projects", "tag:rust");
  }
  const base = run("git", [
    "merge-base",
    process.env.NX_BASE,
    process.env.NX_HEAD || "HEAD",
  ]).trim();
  const touched = touchedByWorkspaceFiles(base);
  if (touched === "all") {
    return projects("--projects", "tag:rust");
  }
  const selected = new Set(projects("--affected", "--projects", "tag:rust"));
  if (touched.length > 0) {
    for (const crate of projects(
      "--affected",
      "--files",
      touched.join(","),
      "--projects",
      "tag:rust",
    )) {
      selected.add(crate);
    }
  }
  return [...selected];
}

const crates = selectedCrates();
const core = new Set(projects("--projects", "tag:core"));
process.stdout.write(`crates=${crates.map((crate) => `-p ${crate}`).join(" ")}\n`);
process.stdout.write(`core=${crates.some((crate) => core.has(crate))}\n`);
