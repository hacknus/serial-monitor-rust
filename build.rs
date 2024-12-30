use std::fs;
fn main() {
    println!("cargo:rustc-env=MACOSX_DEPLOYMENT_TARGET=10.8");

    // Get the package version from the environment variable
    let version = env!("CARGO_PKG_VERSION");

    // Define the paths to `Cargo.toml`
    let cargo_toml_path = "Cargo.toml";

    // Read `Cargo.toml`
    let content = fs::read_to_string(cargo_toml_path).expect("Failed to read Cargo.toml");

    // Replace `version = ...` only within `[package.metadata.bundle]` and `[package.metadata.wix]`
    let updated_content = content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("version =") {
                // Check if we're inside the relevant sections
                if content.contains("[package.metadata.bundle]")
                    || content.contains("[package.metadata.wix]")
                {
                    format!(r#"version = "{}""#, version)
                } else {
                    line.to_string()
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Write the updated content back to `Cargo.toml`
    fs::write(cargo_toml_path, updated_content).expect("Failed to write updated Cargo.toml");

    println!("cargo:rerun-if-changed={}", cargo_toml_path);
}
