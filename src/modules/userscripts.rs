use once_cell::sync::Lazy;
use regex::Regex;
use std::io::Read;
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::*;
static METADATA_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)\A\s*\/\/ ==UserScript==.*?\/\/ ==\/UserScript=="#).unwrap());
static IIFE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?s)^\s*(?:['\"]use strict['\"];?\s*)?\(.*\)\s*\(\s*\)\s*;?\s*$"#).unwrap()
});

use crate::utils;

// TODO: everything
fn parse_metadata(content: &mut String) {
    if let Some(metadata_block) = METADATA_REGEX.find(content) {
        let metadata = metadata_block.as_str();

        if metadata.contains("// @run-at document-end") {
            *content = format!(
                "document.addEventListener('DOMContentLoaded', function() {{\n{}\n}});",
                content
            );
        }
    }
}

fn parse(mut content: String) -> String {
    if METADATA_REGEX.is_match(&content) {
        parse_metadata(&mut content);
    }


    // wrap it in an IIFE if it's not already
    if IIFE_REGEX.is_match(content.as_str()) {
        return content;
    }

    format!("(function() {{\n{}\n}})();", content)
}

pub fn load(webview: &ICoreWebView2) -> Result<()> {
    let scripts_dir = std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\scripts";

    if let Ok(entries) = std::fs::read_dir(scripts_dir) {
        for entry in entries.flatten() {
            if !match entry.path().extension() {
                Some(ext) => ext.eq_ignore_ascii_case("js"),
                None => false,
            } {
                continue;
            }

            let mut file = std::fs::File::open(entry.path())?;
            let mut content = String::new();
            file.read_to_string(&mut content)?;

            let parsed = parse(content);
            
            unsafe {
                webview.AddScriptToExecuteOnDocumentCreated(
                    PCWSTR(utils::create_utf_string(&parsed).as_ptr()),
                    None,
                )?
            }
        }
    }

    Ok(())
}
