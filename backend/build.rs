// Rebuild when a migration is added or changed, so the server always carries every migration
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
