extern crate embed_resource;
fn main() {
    embed_resource::compile("./resources/client.rc", embed_resource::NONE)
        .manifest_optional()
        .ok();
    if let Err(e) = embed_resource::compile("./resources/glorp-manifest.rc", embed_resource::NONE)
        .manifest_required()
    {
        eprintln!("{}", e)
    };
    println!("cargo:rerun-if-changed=Cargo.toml");
}
