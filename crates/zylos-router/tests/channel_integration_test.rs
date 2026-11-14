//! Integration tests for ChannelRegistry and SessionManager

use bytes::Bytes;
use zylos_router::channels::feishu::{FeishuAdapter, FeishuConfig};
use zylos_router::channels::wechat::{WechatAdapter, WechatConfig};
use zylos_router::channels::ChannelAdapter;
use zylos_router::{ChannelRegistry, SessionManager};

fn make_wechat_adapter() -> WechatAdapter {
    WechatAdapter::new(WechatConfig {
        corp_id: "test_corp".into(),
        agent_id: "1000001".into(),
        secret: "test_secret".into(),
        token: "test_token".into(),
        encoding_aes_key: "dGVzdGtleXRlc3RrZXl0ZXN0a2V5dGVzdGtleTE=".into(),
        max_rps: 10,
    })
}

fn make_feishu_adapter() -> FeishuAdapter {
    FeishuAdapter::new(FeishuConfig {
        app_id: "cli_test".into(),
        app_secret: "secret_test".into(),
        verification_token: "vtoken_test".into(),
        encrypt_key: None,
        max_rps: 10,
    })
}

#[tokio::test]
async fn test_registry_register_and_list() {
    let mut reg = ChannelRegistry::new();
    reg.register(make_wechat_adapter()).unwrap();
    reg.register(make_feishu_adapter()).unwrap();

    let configured = reg.list_configured();
    assert!(configured.contains(&"wechat"));
    assert!(configured.contains(&"feishu"));
    assert_eq!(configured.len(), 2);
}

#[tokio::test]
async fn test_registry_duplicate_register_fails() {
    let mut reg = ChannelRegistry::new();
    reg.register(make_wechat_adapter()).unwrap();
    let result = reg.register(make_wechat_adapter());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already registered"));
}

#[tokio::test]
async fn test_registry_receive_unknown_channel() {
    let reg = ChannelRegistry::new();
    let result = reg.receive("nonexistent", Bytes::from("hello")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_registry_wechat_receive_text() {
    let mut reg = ChannelRegistry::new();
    reg.register(make_wechat_adapter()).unwrap();

    let xml = r#"<xml>
        <ToUserName><![CDATA[toUser]]></ToUserName>
        <FromUserName><![CDATA[fromUser]]></FromUserName>
        <CreateTime>1348831860</CreateTime>
        <MsgType><![CDATA[text]]></MsgType>
        <Content><![CDATA[Hello from WeChat]]></Content>
        <MsgId>9876543210</MsgId>
        <AgentID>1</AgentID>
    </xml>"#;

    let msg = reg.receive("wechat", Bytes::from(xml)).await.unwrap();
    assert_eq!(msg.channel_type, "wechat");
    assert_eq!(msg.external_user_id, "fromUser");
    if let zylos_router::channels::MessageContent::Text { body } = &msg.content {
        assert_eq!(body, "Hello from WeChat");
    } else {
        panic!("expected text content");
    }
}

#[tokio::test]
async fn test_registry_feishu_challenge_verification() {
    let mut reg = ChannelRegistry::new();
    reg.register(make_feishu_adapter()).unwrap();

    let challenge_body = serde_json::json!({
        "challenge": "test_challenge_string",
        "token": "vtoken_test",
        "type": "url_verification"
    })
    .to_string();

    let result = reg
        .verify_webhook("feishu", "", None, challenge_body.as_bytes())
        .await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.contains("test_challenge_string"));
}

#[tokio::test]
async fn test_registry_feishu_receive_im_event() {
    let mut reg = ChannelRegistry::new();
    reg.register(make_feishu_adapter()).unwrap();

    let event = serde_json::json!({
        "schema": "2.0",
        "header": {
            "event_id": "evt_001",
            "event_type": "im.message.receive_v1",
            "create_time": "1700000000",
            "token": "vtoken_test",
            "app_id": "cli_test",
            "tenant_key": "tenant_001"
        },
        "event": {
            "sender": { "sender_id": { "open_id": "ou_sender1" } },
            "message": {
                "message_id": "om_msg001",
                "message_type": "text",
                "content": "{\"text\":\"Hello from Feishu\"}",
                "create_time": "1700000000",
                "chat_id": "oc_chat001",
                "chat_type": "p2p"
            }
        }
    })
    .to_string();

    let msg = reg.receive("feishu", Bytes::from(event)).await.unwrap();
    assert_eq!(msg.channel_type, "feishu");
    assert_eq!(msg.external_user_id, "ou_sender1");
    assert_eq!(msg.message_id, "om_msg001");
    if let zylos_router::channels::MessageContent::Text { body } = &msg.content {
        assert_eq!(body, "Hello from Feishu");
    } else {
        panic!("expected text content");
    }
}

#[tokio::test]
async fn test_session_manager_get_or_create() {
    let mut mgr = SessionManager::new();
    let s1 = mgr.get_or_create("wechat", "ext_user_1", "zylos_user_a");
    assert_eq!(s1.channel_type, "wechat");
    assert_eq!(s1.external_user_id, "ext_user_1");
    assert_eq!(s1.zylos_user_id, "zylos_user_a");
    let s1_id = s1.id.clone();

    // Same call returns same session
    let s2 = mgr.get_or_create("wechat", "ext_user_1", "zylos_user_a");
    assert_eq!(s1_id, s2.id);
    assert_eq!(mgr.count(), 1);

    // Different user creates new session
    mgr.get_or_create("feishu", "ext_user_2", "zylos_user_b");
    assert_eq!(mgr.count(), 2);
}

#[tokio::test]
async fn test_session_manager_lookup() {
    let mut mgr = SessionManager::new();
    mgr.get_or_create("wechat", "ext_1", "zylos_a");

    assert!(mgr.lookup("wechat", "ext_1").is_some());
    assert!(mgr.lookup("wechat", "ext_999").is_none());
    assert!(mgr.lookup("feishu", "ext_1").is_none());
}

#[tokio::test]
async fn test_is_configured_checks() {
    let wechat = make_wechat_adapter();
    assert!(wechat.is_configured());

    let empty_wechat = WechatAdapter::new(WechatConfig {
        corp_id: "".into(),
        agent_id: "".into(),
        secret: "".into(),
        token: "".into(),
        encoding_aes_key: "".into(),
        max_rps: 10,
    });
    assert!(!empty_wechat.is_configured());

    let feishu = make_feishu_adapter();
    assert!(feishu.is_configured());

    let empty_feishu = FeishuAdapter::new(FeishuConfig {
        app_id: "".into(),
        app_secret: "".into(),
        verification_token: "".into(),
        encrypt_key: None,
        max_rps: 10,
    });
    assert!(!empty_feishu.is_configured());
}
