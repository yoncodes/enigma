fn main() {
    // sqlx::migrate! embeds migrations at compile time. Watch the directory so
    // adding a new migration cannot silently reuse an older compiled list.
    println!("cargo:rerun-if-changed=migrations");
}
