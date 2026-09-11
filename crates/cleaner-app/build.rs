//! Compiles the Windows resources: the icon, the manifest, and the version information.
//!
//! `app.rc` is a template. Filling in the version from Cargo.toml keeps the executable's
//! properties on the same version as the release, which a hand-edited copy did not.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=assets/app.ico");

    let crate_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let version = env::var("CARGO_PKG_VERSION").expect("Cargo sets CARGO_PKG_VERSION");
    let version_number = ["MAJOR", "MINOR", "PATCH"]
        .map(|part| {
            env::var(format!("CARGO_PKG_VERSION_{part}")).expect("Cargo sets each version part")
        })
        .join(",")
        + ",0";

    let template = fs::read_to_string(crate_dir.join("app.rc")).expect("app.rc must be readable");
    let script = template
        .replace(
            "@ICON@",
            &rc_path(&crate_dir.join("assets").join("app.ico")),
        )
        .replace("@MANIFEST@", &rc_path(&crate_dir.join("app.manifest")))
        .replace("@VERSION_NUMBER@", &version_number)
        .replace("@VERSION@", &version);
    assert!(
        !script.contains('@'),
        "app.rc has a placeholder build.rs does not fill"
    );

    // The generated script sits in OUT_DIR, away from the files it names, so they are named by
    // absolute path.
    let generated = out_dir.join("app.rc");
    fs::write(&generated, script).expect("the generated app.rc must be writable");
    embed_resource::compile(&generated, embed_resource::NONE)
        .manifest_required()
        .expect("Windows application resources must compile");
}

/// Quotes a path for a resource script string, where a backslash starts an escape.
fn rc_path(path: &Path) -> String {
    path.to_str()
        .expect("the crate path must be UTF-8")
        .replace('\\', "\\\\")
}
