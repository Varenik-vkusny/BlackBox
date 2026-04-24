use std::path::Path;

fn main() {
    // Ensure the assets/dist directory exists so rust-embed doesn't
    // fail compilation on a fresh checkout (CI downloads artifacts here).
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/dist");
    if !dist.exists() {
        std::fs::create_dir_all(&dist).unwrap();
    }
}
