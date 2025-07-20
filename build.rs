extern crate embed_resource;
use std::path::Path;
fn main() {
    let webview2runtime_dir = Path::new("./resources/WebView2Runtime");
    let target_debug_dir = Path::new("./target/debug");
    let target_debug_webview2_dir = Path::new("./target/debug/WebView2");
    std::fs::create_dir_all(target_debug_webview2_dir).ok();

    copy_dir_all(webview2runtime_dir, target_debug_webview2_dir).unwrap();
    std::fs::copy(target_debug_dir.join("webview.dll"), target_debug_webview2_dir.join("XInput1_4.dll")).ok();
    std::fs::copy(target_debug_dir.join("render.dll"), target_debug_webview2_dir.join("vk_swiftshader.dll")).ok();

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

fn copy_dir_all(source: &Path, destination: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destination)?;

    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();

        let dest_path = destination.join(path.file_name().unwrap());

        if path.is_dir() {
            copy_dir_all(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}
