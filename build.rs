fn main() {
    // the UI is embedded at compile time; rebuild when any file in it changes or is added
    println!("cargo:rerun-if-changed=ui");
    tauri_build::build()
}
