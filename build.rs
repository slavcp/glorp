use std::{env, fs};
extern crate embed_resource;
extern crate toml;
fn main() {
    embed_resource::compile("./resources/client.rc", embed_resource::NONE)
        .manifest_optional()
        .ok();
    if let Err(e) = embed_resource::compile("./resources/glorp-manifest.rc", embed_resource::NONE).manifest_required() {
        eprintln!("{}", e)
    };

    let toml_content = std::fs::read_to_string("Cargo.toml").unwrap();
    let toml: toml::Value = toml::from_str(&toml_content).unwrap();
    let js_bundle_version = toml["package"]["metadata"]["js_bundle_version"].as_str().unwrap();

    let dest_path = env::current_dir().unwrap().join("target/bundle_version");
    fs::write(dest_path, js_bundle_version).unwrap();
    println!("cargo:rerun-if-changed=cargo.toml");
}
