fn main() {
    for name in [
        "OPENBITFUN_RELEASE_CHANNEL",
        "OPENBITFUN_PRODUCT_ID",
        "OPENBITFUN_DESKTOP_BINARY_NAME",
        "OPENBITFUN_DATA_MIGRATOR_BINARY_NAME",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    tauri_build::build();
}
