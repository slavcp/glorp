use regex::Regex;
use std::{io::Read, sync::LazyLock};
use webview2_com::Microsoft::Web::WebView2::Win32::*;
use windows::core::*;
static METADATA_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)\A\s*\/\/ ==UserScript==.*?\/\/ ==\/UserScript=="#).unwrap());
static IIFE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)^\s*(?:['\"]use strict['\"];?\s*)?\(.*\)\s*\(\s*\)\s*;?\s*$"#).unwrap());

use crate::utils;

// TODO: everything
fn parse_metadata(content: &mut String) {
    if let Some(metadata_block) = METADATA_REGEX.find(content) {
        let metadata = metadata_block.as_str();
        if !metadata.contains("// @run-at document-start") {
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

pub fn load(webview: &ICoreWebView2, social: bool) -> Result<()> {
    let scripts_dir = if social {
        std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\scripts\\social"
    } else {
        std::env::var("USERPROFILE").unwrap() + "\\Documents\\glorp\\scripts"
    };

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
                webview.AddScriptToExecuteOnDocumentCreated(PCWSTR(utils::create_utf_string(parsed).as_ptr()), None)?
            }
        }
    }

    Ok(())
}
