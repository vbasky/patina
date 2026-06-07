// The server embeds the built UI via `include_bytes!`/`include_str!`. Cargo
// doesn't track those asset paths on its own, so changes to the frontend bundle
// wouldn't trigger a rebuild and the binary would keep serving a stale UI.
// Register the dist directory as a build input so `cargo build` re-embeds it
// whenever the frontend is rebuilt.
fn main() {
    println!("cargo:rerun-if-changed=../browser/ui/dist");
    println!("cargo:rerun-if-changed=../browser/ui/dist/index.html");
    println!("cargo:rerun-if-changed=../browser/ui/dist/assets/index.js.gz");
    println!("cargo:rerun-if-changed=../browser/ui/dist/assets/index.css.gz");
    println!("cargo:rerun-if-changed=../browser/ui/dist/patina.svg");
}
