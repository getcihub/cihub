//! Embeds the build of the dashboard (`web/dist`, `mise run web-build`) in
//! the binary, as a table of paths and contents. Without a build, the table is
//! empty and the server answers 404 on the pages of the dashboard: a build of
//! the Rust code alone does not need Node. The script creates `web/dist`
//! empty when it is missing (git ignores it): cargo runs a script again at
//! each command while a path that it watches does not exist.

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

fn main() -> io::Result<()> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(io::Error::other)?);
    let dist = manifest.join("../../web/dist");
    let out = PathBuf::from(env::var("OUT_DIR").map_err(io::Error::other)?);
    let mut files = Vec::new();

    fs::create_dir_all(&dist)?;
    println!("cargo::rerun-if-changed={}", dist.display());
    collect(&dist, &dist, &mut files)?;
    files.sort();

    let mut table = String::from("pub(crate) static FILES: &[(&str, &[u8])] = &[\n");
    for (path, file) in &files {
        println!("cargo::rerun-if-changed={}", file.display());
        // `Debug` writes a string literal, with its quotes and its escapes.
        let absolute = file.canonicalize()?;
        let absolute = absolute.to_string_lossy();
        let _ = writeln!(table, "    ({path:?}, include_bytes!({absolute:?})),");
    }
    table.push_str("];\n");
    fs::write(out.join("dashboard.rs"), table)
}

/// Each file under `dir`, with its path on the server: `/` and its path in
/// `root`.
fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, files)?;
            continue;
        }

        let relative = path.strip_prefix(root).map_err(io::Error::other)?;
        let served = relative
            .components()
            .fold(String::new(), |mut served, part| {
                served.push('/');
                served.push_str(&part.as_os_str().to_string_lossy());
                served
            });
        files.push((served, path));
    }
    Ok(())
}
