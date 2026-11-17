//! Testes multi-cliente do servidor CRDT (Fase 11.9 — T11.9.7).
//!
//! Cada teste sobe o servidor numa porta aleatória e conecta clientes Yjs
//! nativos (yrs + tokio-tungstenite). Critérios: convergência de todos os
//! clientes, zero perda de dados (todo caractere inserido aparece no estado
//! final) e suporte a ≥10 clientes simultâneos.

mod crdt_common;

use crdt_common::{wait_converged, wait_until, Client, TestServer};
use std::sync::Arc;
use std::time::Duration;
use zylos_auth::crdt_persistence::{text_of_state, CrdtStore};

const T: Duration = Duration::from_secs(10);

async fn connect_n(server: &TestServer, doc: &str, n: u64) -> Vec<Client> {
    let mut out = Vec::new();
    for i in 0..n {
        out.push(Client::connect(&server.url(doc), 1000 + i).await);
    }
    out
}

fn count_char(s: &str, c: char) -> usize {
    s.chars().filter(|&x| x == c).count()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_three_clients_concurrent_inserts_converge() {
    let server = TestServer::start(None).await;
    let clients = connect_n(&server, "doc-3", 3).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    for (i, c) in clients.iter().enumerate() {
        let ch = (b'a' + i as u8) as char;
        for _ in 0..20 {
            c.insert(0, &ch.to_string());
        }
    }
    assert!(wait_converged(&clients, T).await, "clientes não convergiram");
    let text = clients[0].text();
    assert_eq!(text.len(), 60);
    for ch in ['a', 'b', 'c'] {
        assert_eq!(count_char(&text, ch), 20, "perda de dados para {ch}");
    }
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_ten_clients_simultaneous_edits_zero_data_loss() {
    let server = TestServer::start(None).await;
    let clients = connect_n(&server, "doc-10", 10).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut handles = Vec::new();
    let clients = Arc::new(clients);
    for i in 0..10usize {
        let cs = clients.clone();
        handles.push(tokio::spawn(async move {
            for k in 0..25 {
                cs[i].push(&format!("[{i}:{k}]"));
                if k % 5 == 0 {
                    tokio::task::yield_now().await;
                }
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
    assert!(wait_converged(&clients, T).await, "10 clientes não convergiram");
    let text = clients[0].text();
    for i in 0..10 {
        for k in 0..25 {
            assert!(text.contains(&format!("[{i}:{k}]")), "faltou [{i}:{k}]");
        }
    }
    assert_eq!(server.state.room("doc-10").unwrap().text(), text);
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_late_joiner_receives_full_state() {
    let server = TestServer::start(None).await;
    let a = Client::connect(&server.url("late"), 1).await;
    a.push("estado inicial");
    let b = Client::connect(&server.url("late"), 2).await;
    a.push(" + mais");
    assert!(wait_until(T, || b.text() == "estado inicial + mais").await);
    let c = Client::connect(&server.url("late"), 3).await;
    assert!(wait_until(T, || c.text() == a.text()).await);
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_concurrent_insert_and_delete_converge() {
    let server = TestServer::start(None).await;
    let clients = connect_n(&server, "del", 3).await;
    clients[0].push("0123456789");
    assert!(wait_converged(&clients, T).await);
    assert!(wait_until(T, || clients[2].text() == "0123456789").await);
    // Concorrência: um apaga o meio, outro insere no início, outro no fim.
    clients[0].delete(3, 4);
    clients[1].insert(0, "X");
    clients[2].push("Y");
    assert!(wait_converged(&clients, T).await);
    assert!(wait_until(T, || clients[0].text() == "X012789Y").await, "texto: {}", clients[0].text());
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_same_position_conflict_is_deterministic() {
    let server = TestServer::start(None).await;
    let clients = connect_n(&server, "same-pos", 4).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    for (i, c) in clients.iter().enumerate() {
        c.insert(0, &format!("<{i}>"));
    }
    assert!(wait_converged(&clients, T).await);
    assert!(wait_until(T, || clients[0].text().len() == 12).await);
    let text = clients[0].text();
    for i in 0..4 {
        assert!(text.contains(&format!("<{i}>")));
    }
    assert!(clients.iter().all(|c| c.text() == text));
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_offline_edits_merge_on_reconnect() {
    let server = TestServer::start(None).await;
    let url = server.url("offline");
    let mut a = Client::connect(&url, 1).await;
    let b = Client::connect(&url, 2).await;
    a.push("base");
    assert!(wait_until(T, || b.text() == "base").await);
    a.disconnect().await;
    // Edições divergentes: A offline, B online.
    a.push("-offlineA");
    b.insert(0, "onlineB-");
    tokio::time::sleep(Duration::from_millis(50)).await;
    a.attach(&url).await;
    assert!(wait_until(T, || a.text() == b.text() && a.text().len() == 21).await, "a={} b={}", a.text(), b.text());
    assert_eq!(a.text(), "onlineB-base-offlineA");
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_rooms_are_isolated() {
    let server = TestServer::start(None).await;
    let a1 = Client::connect(&server.url("room-a"), 1).await;
    let a2 = Client::connect(&server.url("room-a"), 2).await;
    let b1 = Client::connect(&server.url("room-b"), 3).await;
    a1.push("só na sala A");
    b1.push("só na sala B");
    assert!(wait_until(T, || a2.text() == "só na sala A").await);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(b1.text(), "só na sala B");
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_presence_cursor_broadcast_and_cleanup() {
    let server = TestServer::start(None).await;
    let url = server.url("presence");
    let mut a = Client::connect(&url, 11).await;
    let b = Client::connect(&url, 12).await;
    let c = Client::connect(&url, 13).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    a.set_presence(r##"{"user":{"name":"Ana","color":"#e11d48"},"cursor":{"anchor":2,"head":5}}"##);
    assert!(wait_until(T, || b.presence_of(11).is_some() && c.presence_of(11).is_some()).await);
    let p = b.presence_of(11).unwrap();
    assert_eq!(p["user"]["name"], "Ana");
    assert_eq!(p["cursor"]["head"], 5);
    // Cliente tardio recebe presença existente no handshake.
    let d = Client::connect(&url, 14).await;
    assert!(wait_until(T, || d.presence_of(11).is_some()).await);
    // Ao desconectar, a presença de A é removida em todos.
    a.disconnect().await;
    assert!(wait_until(T, || b.presence_of(11).is_none() && d.presence_of(11).is_none()).await);
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_persistence_survives_server_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("crdt.db");
    {
        let store = Arc::new(CrdtStore::open(&path).unwrap());
        let server = TestServer::start(Some(store.clone())).await;
        let a = Client::connect(&server.url("persist"), 1).await;
        let b = Client::connect(&server.url("persist"), 2).await;
        a.push("persistido ");
        b.push("por dois");
        assert!(wait_converged(&[a, b], T).await);
        assert!(wait_until(T, || store.load("persist").unwrap().map(|s| text_of_state(&s).unwrap().len()) == Some(19)).await);
        server.stop();
    }
    let store = Arc::new(CrdtStore::open(&path).unwrap());
    let server = TestServer::start(Some(store)).await;
    let c = Client::connect(&server.url("persist"), 3).await;
    assert!(wait_until(T, || c.text().len() == 19).await, "texto: {}", c.text());
    assert!(c.text().contains("persistido ") && c.text().contains("por dois"));
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_last_client_leaving_creates_snapshot_version() {
    let store = Arc::new(CrdtStore::open_in_memory().unwrap());
    let server = TestServer::start(Some(store.clone())).await;
    let mut a = Client::connect(&server.url("snap"), 1).await;
    a.push("versão 1");
    assert!(wait_until(T, || store.pending_updates("snap").unwrap_or(0) > 0).await);
    a.disconnect().await;
    assert!(wait_until(T, || !store.list_versions("snap").unwrap().is_empty()).await);
    let v = store.list_versions("snap").unwrap();
    let st = store.load_version("snap", v[0].version).unwrap().unwrap();
    assert_eq!(text_of_state(&st).unwrap(), "versão 1");
    assert_eq!(store.pending_updates("snap").unwrap(), 0);
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_twelve_clients_interleaved_insert_delete_stress() {
    let server = TestServer::start(None).await;
    let clients = Arc::new(connect_n(&server, "stress", 12).await);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut hs = Vec::new();
    for i in 0..12usize {
        let cs = clients.clone();
        hs.push(tokio::spawn(async move {
            for k in 0..30u32 {
                let c = &cs[i];
                if k % 7 == 6 {
                    c.delete(0, 1);
                } else {
                    c.insert((k * 3) % 17, "z");
                }
                tokio::task::yield_now().await;
            }
        }));
    }
    for h in hs {
        h.await.unwrap();
    }
    assert!(wait_converged(&clients, T).await, "stress não convergiu");
    let room = server.state.room("stress").unwrap();
    assert!(wait_until(T, || room.text() == clients[0].text()).await);
    assert_eq!(room.client_count(), 12);
    server.stop();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn crdt_rest_lists_active_documents_and_versions() {
    let store = Arc::new(CrdtStore::open_in_memory().unwrap());
    let server = TestServer::start(Some(store.clone())).await;
    let a = Client::connect(&server.url("rest-doc"), 1).await;
    a.push("conteúdo REST");
    assert!(wait_until(T, || store.pending_updates("rest-doc").unwrap_or(0) > 0).await);
    // Usa uma conexão TCP crua para não exigir cliente HTTP extra.
    let body = http_get(server.addr, "/api/v1/crdt/documents?active=true").await;
    assert!(body.contains("\"doc_id\":\"rest-doc\""), "body: {body}");
    assert!(body.contains("\"clients\":1"));
    let body = http_get(server.addr, "/api/v1/crdt/documents/rest-doc").await;
    assert!(body.contains("conteúdo REST"), "body: {body}");
    let body = http_post(server.addr, "/api/v1/crdt/documents/rest-doc/compact").await;
    assert!(body.contains("\"version\":1"), "body: {body}");
    let body = http_get(server.addr, "/api/v1/crdt/documents/rest-doc/versions/1").await;
    assert!(body.contains("conteúdo REST"), "body: {body}");
    server.stop();
}

async fn http_req(addr: std::net::SocketAddr, method: &str, path: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut s = tokio::net::TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"
    );
    s.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    String::from_utf8_lossy(&buf).to_string()
}

async fn http_get(addr: std::net::SocketAddr, path: &str) -> String {
    http_req(addr, "GET", path).await
}

async fn http_post(addr: std::net::SocketAddr, path: &str) -> String {
    http_req(addr, "POST", path).await
}
