//! E.2.11 — Cockpit UI serves static assets and WASM bundle.

use crate::helpers::TestGateway;

#[tokio::test]
async fn cockpit_serves_static_and_wasm() {
    let gw = TestGateway::spawn().await.expect("spawn gateway");

    // Check index.html
    let index_url = format!("{}/", gw.base_url());
    let index_resp = gw.client.get(&index_url).send().await.expect("fetch index");
    assert!(
        index_resp.status().is_success(),
        "index.html should be served: {}",
        index_resp.status()
    );
    let index_body = index_resp.text().await.expect("read index body");
    assert!(
        index_body.contains("<!DOCTYPE html>") || index_body.contains("<html"),
        "index.html should contain HTML doctype or html tag"
    );

    // Check WASM bundle exists
    let wasm_url = format!("{}/pkg/zylos_cockpit_bg.wasm", gw.base_url());
    let wasm_resp = gw.client.head(&wasm_url).send().await.expect("head wasm");
    assert!(
        wasm_resp.status().is_success(),
        "WASM bundle should be served at /pkg/zylos_cockpit_bg.wasm: {}",
        wasm_resp.status()
    );

    // Check service worker
    let sw_url = format!("{}/sw.js", gw.base_url());
    let sw_resp = gw.client.get(&sw_url).send().await.expect("fetch sw.js");
    assert!(
        sw_resp.status().is_success(),
        "service worker should be served: {}",
        sw_resp.status()
    );
}
