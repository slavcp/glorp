use std::{
    net::{IpAddr, Ipv4Addr},
    sync::{LazyLock, Mutex},
    time,
};
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::core::*;

static LAST_CONNECTED_LOBBY: LazyLock<Mutex<IpAddr>> =
    LazyLock::new(|| Mutex::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));

pub fn load(window: &ICoreWebView2) {
    unsafe {
        window
            .CallDevToolsProtocolMethod(w!("Network.enable"), w!("{}"), None)
            .ok();

        let ws_receiver = window
            .GetDevToolsProtocolEventReceiver(w!("Network.webSocketCreated"))
            .unwrap();

        let handler = DevToolsProtocolEventReceivedEventHandler::create(Box::new(move |_, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let mut params = PWSTR::null();

            args.ParameterObjectAsJson(&mut params)?;

            let params = take_pwstr(params);
            let json = serde_json::from_str::<serde_json::Value>(&params).unwrap();

            let url = json.get("url").unwrap().to_string();
            if url.contains("lobby-") {
                let host = url.split("://").last().unwrap().split("/").next().unwrap().to_string();
                let resolved_ips = dns_lookup::lookup_host(&host)?;
                if let Some(ip) = resolved_ips.into_iter().next() {
                    *LAST_CONNECTED_LOBBY.lock().unwrap() = ip;
                }
            }
            Ok(())
        }));

        ws_receiver
            .add_DevToolsProtocolEventReceived(&handler, crate::TOKEN)
            .ok();
    }
}

pub fn ping(webview: &ICoreWebView2) {
    let result = ping_rs::send_ping(
        &LAST_CONNECTED_LOBBY.lock().unwrap(),
        time::Duration::from_secs(1),
        Default::default(),
        Some(&ping_rs::PingOptions {
            ttl: 128,
            dont_fragment: true,
        }),
    );
    unsafe {
        if let Ok(reply) = result {
            webview
                .PostWebMessageAsJson(PCWSTR(
                    crate::utils::create_utf_string(format!("{{\"pingInfo\":{}}}", reply.rtt)).as_ptr(),
                ))
                .ok();
        }
    }
}
