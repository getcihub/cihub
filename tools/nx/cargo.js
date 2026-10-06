// Nx plugin: one project per crate of the Cargo workspace, tagged `rust`, and
// an edge for each dependency between crates, both read from `cargo metadata`.
// Cargo.toml stays the only place that lists the crates and their dependencies;
// `[package.metadata.nx]` adds tags and the files outside the crate that its
// build reads.
//
// Each crate gets one target, `cargo`, that runs nothing. Nx counts a file
// outside every project as a change only when a target lists it in its inputs;
// this target carries the files of the whole workspace (the `inputs` option)
// and those of the crate. The CI then runs one cargo invocation over the
// affected crates (`mise run affected`): one invocation per crate resolves the
// features again for each crate and rebuilds the shared dependencies.
const { execFileSync } = require("node:child_process");
const { dirname, relative } = require("node:path");

function workspaceCrates(workspaceRoot) {
  const metadata = JSON.parse(
    execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
      cwd: workspaceRoot,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    }),
  );
  return metadata.packages.map((pkg) => ({
    name: pkg.name,
    root: relative(workspaceRoot, dirname(pkg.manifest_path)),
    // A change in a dev-dependency changes the tests of this crate, so every
    // kind of dependency counts.
    paths: pkg.dependencies
      .filter((dep) => dep.path)
      .map((dep) => relative(workspaceRoot, dep.path)),
    nx: pkg.metadata?.nx ?? {},
  }));
}

module.exports = {
  name: "cihub-cargo",

  createNodes: [
    "**/Cargo.toml",
    (manifests, options, context) => {
      const crates = new Map(
        workspaceCrates(context.workspaceRoot).map((crate) => [crate.root, crate]),
      );
      const inputs = options?.inputs ?? [];
      // The root Cargo.toml has no [package] and gives no project.
      return manifests
        .map((manifest) => crates.get(dirname(manifest)))
        .filter((crate) => crate)
        .map((crate) => [
          `${crate.root}/Cargo.toml`,
          {
            projects: {
              [crate.root]: {
                name: crate.name,
                tags: ["rust", ...(crate.nx.tags ?? [])],
                targets: {
                  cargo: {
                    executor: "nx:noop",
                    inputs: [...inputs, ...(crate.nx.inputs ?? [])].map(
                      (path) => `{workspaceRoot}/${path}`,
                    ),
                  },
                },
              },
            },
          },
        ]);
    },
  ],

  createDependencies: (_options, context) => {
    const crates = workspaceCrates(context.workspaceRoot);
    const byRoot = new Map(crates.map((crate) => [crate.root, crate.name]));
    return crates.flatMap((crate) =>
      crate.paths
        .filter((path) => byRoot.has(path))
        .map((path) => ({
          source: crate.name,
          target: byRoot.get(path),
          type: "static",
          sourceFile: `${crate.root}/Cargo.toml`,
        })),
    );
  },
};
