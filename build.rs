extern crate embed_resource;
const VER_DIR: &str = "./target";
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    embed_resource::compile("./resources/client.rc", embed_resource::NONE)
        .manifest_optional()
        .ok();
    if let Err(e) = embed_resource::compile("./resources/glorp-manifest.rc", embed_resource::NONE)
        .manifest_required()
    {
        eprintln!("{}", e)
    };

    let dest_path = std::path::Path::new(VER_DIR).join("version.rs");
    std::fs::write(
        dest_path,
        format!("pub const VERSION: &str = \"{}\";", VERSION),
    )
    .unwrap();

    println!("cargo:rerun-if-changed=Cargo.toml");
}
