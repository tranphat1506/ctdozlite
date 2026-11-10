use serde_json::json;
use zalo_rust::chat::{merge_thread_into, ChatMessage, Thread};
use zalo_rust::message_model::{DeliveryState, WireMessageRef};

fn message(id: &str, client: &str) -> ChatMessage {
    ChatMessage {
        id: id.into(),
        client_message_id: client.into(),
        text: "Repeated text".into(),
        ts: 1_793_425_680_000,
        time: "12:48".into(),
        is_outgoing: true,
        ..Default::default()
    }
}

fn thread(messages: Vec<ChatMessage>) -> Thread {
    Thread {
        id: "contact-1".into(),
        name: "Contact".into(),
        preview: String::new(),
        time: String::new(),
        avatar: None,
        is_group: false,
        messages,
        presentation: Default::default(),
        unread: Default::default(),
    }
}

#[test]
fn distinct_server_ids_survive_when_text_and_time_match() {
    let mut target = thread(vec![message("server-a", "client-a")]);
    merge_thread_into(
        &mut target,
        thread(vec![message("server-b", "client-b")]),
        false,
    );
    assert_eq!(target.messages.len(), 2);
}

#[test]
fn pending_echo_reconciles_when_client_reference_matches_exactly() {
    let mut target = thread(vec![message("out_local", "client-a")]);
    let mut server = message("server-a", "client-a");
    server.text = "Server content differs".into();
    server.ts += 60_000;
    server.time = "12:49".into();
    merge_thread_into(&mut target, thread(vec![server]), false);
    assert_eq!(target.messages.len(), 1);
    assert_eq!(target.messages[0].id, "server-a");
}

#[test]
fn missing_or_conflicting_identity_never_falls_back_to_text() {
    for (local, server) in [
        (message("out_local", ""), message("server-a", "")),
        (
            message("out_local", "client-a"),
            message("server-a", "client-b"),
        ),
        (
            message("server-a", "client-a"),
            message("server-b", "client-a"),
        ),
        (message("", ""), message("", "")),
    ] {
        let mut target = thread(vec![local]);
        merge_thread_into(&mut target, thread(vec![server]), false);
        assert_eq!(target.messages.len(), 2);
    }
}

#[test]
fn ambiguous_client_reference_does_not_consume_pending_rows() {
    let mut target = thread(vec![
        message("out_a", "client-a"),
        message("out_b", "client-a"),
    ]);
    merge_thread_into(
        &mut target,
        thread(vec![message("server-a", "client-a")]),
        false,
    );
    assert_eq!(target.messages.len(), 3);
}

#[test]
fn differing_direction_or_wire_sender_does_not_merge() {
    for exact_server in [false, true] {
        for differing_sender in [false, true] {
            let mut local = message(
                if exact_server {
                    "server-a"
                } else {
                    "out_local"
                },
                "client-a",
            );
            let mut server = message("server-a", "client-a");
            local.wire_ref = WireMessageRef::from_raw(&json!({"uidFrom":"sender-a"}));
            if differing_sender {
                server.wire_ref = WireMessageRef::from_raw(&json!({"uidFrom":"sender-b"}));
            } else {
                server.is_outgoing = false;
            }
            let mut target = thread(vec![local]);
            merge_thread_into(&mut target, thread(vec![server]), false);
            assert_eq!(target.messages.len(), 2);
        }
    }
}

#[test]
fn wire_identity_reconciles_and_metadata_enrichment_is_idempotent() {
    let mut local = message("out_local", "");
    local.wire_ref = WireMessageRef::from_raw(&json!({"clientId":1234,"uidFrom":"sender-a"}));
    let mut server = message("server-a", "");
    server.wire_ref = WireMessageRef::from_raw(
        &json!({"cliMsgId":"1234","msgId":"server-a","uidFrom":"sender-a"}),
    );
    server.delivery = DeliveryState::Delivered;
    server.reply_to_text = Some("Quoted text".into());
    server.link_url = Some("https://example.invalid/link".into());
    server.history_source = Some(json!({"cursor":"fake-cursor"}));
    server.is_pinned = true;
    let mut target = thread(vec![local]);
    merge_thread_into(&mut target, thread(vec![server.clone()]), false);
    assert_eq!(target.messages.len(), 1);
    assert_eq!(target.messages[0].id, "server-a");
    assert_eq!(target.messages[0].delivery, DeliveryState::Delivered);
    for (field, value) in &server.wire_ref.fields {
        assert_eq!(target.messages[0].wire_ref.fields.get(field), Some(value));
    }
    assert_eq!(
        target.messages[0].wire_ref.fields.get("clientId"),
        Some(&json!(1234))
    );
    assert_eq!(target.messages[0].reply_to_text, server.reply_to_text);
    assert_eq!(target.messages[0].link_url, server.link_url);
    assert_eq!(target.messages[0].history_source, server.history_source);
    assert!(target.messages[0].is_pinned);
    let before = serde_json::to_value(&target.messages).unwrap();
    merge_thread_into(&mut target, thread(vec![server]), false);
    assert_eq!(serde_json::to_value(&target.messages).unwrap(), before);
}

#[test]
fn repeated_large_merge_preserves_distinct_messages() {
    let mut target = thread(
        (0..5_000)
            .map(|i| message(&format!("server-{i}"), ""))
            .collect(),
    );
    let incoming = thread(
        (2_500..7_500)
            .map(|i| message(&format!("server-{i}"), ""))
            .collect(),
    );
    merge_thread_into(&mut target, incoming.clone(), false);
    assert_eq!(target.messages.len(), 7_500);
    merge_thread_into(&mut target, incoming, false);
    assert_eq!(target.messages.len(), 7_500);
}

#[test]
fn previews_preserve_distinct_server_ids_and_reconcile_exact_client_echoes() {
    use zalo_rust::chat::merge_previews;
    let mut threads = vec![thread(vec![
        message("server-a", "client-a"),
        message("out_local", "client-c"),
    ])];
    let response = json!({"error_code":0,"data":[{"messages":[
        {"msgId":"server-b","cliMsgId":"client-b","idTo":"contact-1","uidFrom":"0","ts":1_793_425_680_000_i64,"content":"Repeated text"},
        {"msgId":"server-c","cliMsgId":"client-c","idTo":"contact-1","uidFrom":"0","ts":1_793_425_740_000_i64,"content":"Changed text"}
    ]}]});
    merge_previews(&mut threads, &response);
    assert_eq!(threads[0].messages.len(), 3);
    for expected in ["server-a", "server-b", "server-c"] {
        assert!(
            threads[0]
                .messages
                .iter()
                .any(|message| message.id == expected),
            "Missing {expected}"
        );
    }
    merge_previews(&mut threads, &response);
    assert_eq!(threads[0].messages.len(), 3);
}

#[test]
fn group_history_preserves_distinct_server_ids_when_text_and_time_match() {
    let mut target = thread(vec![message("server-a", "client-a")]);
    zalo_rust::chat::merge_group_history(
        &mut target,
        &json!({"error_code":0,"data":{"messages":[
            {"msgId":"server-b","cliMsgId":"client-b","idTo":"contact-1","uidFrom":"0","ts":1_793_425_680_000_i64,"content":"Repeated text"}
        ]}}),
    );
    assert_eq!(target.messages.len(), 2);
}

#[test]
fn preview_parser_retains_legacy_id_and_explicit_wire_namespaces() {
    let mut threads = vec![thread(Vec::new())];
    zalo_rust::chat::merge_previews(
        &mut threads,
        &json!({"error_code":0,"data":[{"messages":[
            {"globalMsgId":"server-a","cliMsgId":"client-a","idTo":"contact-1","uidFrom":"0","content":"First"},
            {"cliMsgId":"client-only","idTo":"contact-1","uidFrom":"0","content":"Second"}
        ]}]}),
    );
    assert_eq!(threads[0].messages[0].id, "client-a");
    assert_eq!(
        threads[0].messages[0].wire_ref.server_id,
        Some(json!("server-a"))
    );
    assert_eq!(threads[0].messages[0].client_message_id, "client-a");
    assert_eq!(threads[0].messages[1].id, "client-only");
    assert_eq!(threads[0].messages[1].client_message_id, "client-only");
    assert!(threads[0].messages[1].wire_ref.server_id.is_none());
}

#[test]
fn client_only_packets_are_idempotent_in_previews_and_group_history() {
    let packet = json!({"cliMsgId":"client-only","idTo":"contact-1","uidFrom":"sender-a","ts":1000,"content":"Client only"});
    let previews = json!({"error_code":0,"data":[{"messages":[packet.clone()]}]});
    let history = json!({"error_code":0,"data":{"messages":[packet]}});
    let mut threads = vec![thread(Vec::new())];
    let mut group = thread(Vec::new());
    for _ in 0..3 {
        zalo_rust::chat::merge_previews(&mut threads, &previews);
        zalo_rust::chat::merge_group_history(&mut group, &history);
    }
    assert_eq!(threads[0].messages.len(), 1);
    assert_eq!(group.messages.len(), 1);
    assert_eq!(threads[0].messages[0].id, "client-only");
    assert_eq!(group.messages[0].id, "client-only");
}

#[test]
fn invalid_numeric_wire_ids_do_not_establish_identity() {
    for invalid in [json!(0), json!(1.5), json!(-1)] {
        let mut local = message("out_local", "");
        local.wire_ref = WireMessageRef::from_raw(&json!({"clientId":invalid}));
        let mut server = message("server-a", "");
        server.wire_ref = WireMessageRef::from_raw(&json!({"cliMsgId":invalid}));
        let mut target = thread(vec![local]);
        merge_thread_into(&mut target, thread(vec![server]), false);
        assert_eq!(target.messages.len(), 2);
    }
}

#[test]
fn legacy_stored_client_id_reconciles_without_changing_canonical_id() {
    use zalo_rust::store::{persist_threads, MessageStore};
    let mut store = MessageStore::open(":memory:").unwrap();
    persist_threads(&mut store, &[thread(vec![message("client-legacy", "")])]).unwrap();
    let mut threads = vec![thread(Vec::new())];
    store.hydrate_threads(&mut threads, 100).unwrap();
    assert_eq!(threads[0].messages.len(), 1);
    assert!(threads[0].messages[0].client_message_id.is_empty());
    assert!(threads[0].messages[0].wire_ref.fields.is_empty());
    let response = json!({"error_code":0,"data":[{"messages":[
        {"globalMsgId":"server-a","cliMsgId":"client-legacy","idTo":"contact-1","uidFrom":"0","content":"Repeated text"}
    ]}]});
    zalo_rust::chat::merge_previews(&mut threads, &response);
    assert_eq!(threads[0].messages.len(), 1);
    assert_eq!(threads[0].messages[0].id, "client-legacy");
    assert_eq!(
        threads[0].messages[0].wire_ref.server_id,
        Some(json!("server-a"))
    );
    zalo_rust::chat::merge_previews(&mut threads, &response);
    assert_eq!(threads[0].messages.len(), 1);
}

#[test]
fn distinct_wire_server_ids_survive_when_legacy_client_alias_collides() {
    let mut threads = vec![thread(Vec::new())];
    let response = json!({"error_code":0,"data":[{"messages":[
        {"globalMsgId":"server-a","cliMsgId":"same-client","idTo":"contact-1","uidFrom":"0","content":"Repeated text"},
        {"globalMsgId":"server-b","cliMsgId":"same-client","idTo":"contact-1","uidFrom":"0","content":"Repeated text"}
    ]}]});
    for _ in 0..2 {
        zalo_rust::chat::merge_previews(&mut threads, &response);
        assert_eq!(threads[0].messages.len(), 2);
    }
}
