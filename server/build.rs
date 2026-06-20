use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// Served at /pt.js when the client hasn't been built (e.g. a server-only
// `cargo build`). The crate still compiles; CI and deploy/install.sh build the
// client first so the real bundle ships.
const PLACEHOLDER: &str = "console.warn('pagetally: tracking script not built; run the client build before the server');\n";

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("pt.js");

    // Where to find the built IIFE, in order of precedence:
    //  1. PAGETALLY_SCRIPT — explicit override. deploy/install.sh sets this
    //     because it builds the server from an isolated source copy that has no
    //     sibling client/ directory.
    //  2. ../client/dist/pt.js — the repo layout (dev + CI).
    let candidates = [
        env::var("PAGETALLY_SCRIPT").ok(),
        Some(format!(
            "{}/../client/dist/pt.js",
            env!("CARGO_MANIFEST_DIR")
        )),
    ];

    println!("cargo:rerun-if-env-changed=PAGETALLY_SCRIPT");
    for cand in candidates.into_iter().flatten() {
        println!("cargo:rerun-if-changed={cand}");
        if Path::new(&cand).exists() {
            fs::copy(&cand, &out).expect("copy client script into OUT_DIR");
            return;
        }
    }

    fs::write(&out, PLACEHOLDER).expect("write placeholder script");
    println!(
        "cargo:warning=client/dist/pt.js not found; /pt.js will serve a placeholder. \
         Build the client (npm run build) or set PAGETALLY_SCRIPT to embed the real bundle."
    );
}
