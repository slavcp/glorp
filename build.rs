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

    let toml_content = fs::read_to_string("Cargo.toml").unwrap();
    let toml: toml::Value = toml::from_str(&toml_content).unwrap();
    let package_version = toml["package"]["version"].as_str().unwrap();
    let js_bundle_version = toml["package"]["metadata"]["js_bundle_version"].as_str().unwrap();

    let dest_path = env::current_dir().unwrap().join("target/bundle_version");
    fs::write(dest_path, js_bundle_version).unwrap();

    let wxs_path = "resources/installer_script.wxs";
    let wxs_content = fs::read_to_string(wxs_path).unwrap();
    let regex = regex::Regex::new(r#"Version="([^"]+)""#).unwrap();

    let current_version = regex
        .captures(&wxs_content)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str());

    if current_version != Some(package_version) {
        let updated_wxs = regex
            .replace(&wxs_content, format!(r#"Version="{}""#, package_version))
            .to_string();
        fs::write(wxs_path, updated_wxs).unwrap();
    }

    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=resources/installer_script.wxs");
}
