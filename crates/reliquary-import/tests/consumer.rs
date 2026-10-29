use reliquary_import::{normalize_agent, prepare};

#[test]
fn external_consumer_normalizes_and_prepares_source_and_attachment() {
    let input = br#"{"schema_version":1,"source_namespace":"acct","source_format":"agent-v1","conversations":[{"id":"c","title":null,"messages":[{"id":"m","revision":null,"parent_id":null,"replaces":null,"deleted":false,"role":"user","timestamp_ns":1,"content":"hello","artifacts":[{"id":"a","source_ref":"ref","filename":"a.txt","mime_type":"text/plain","bytes":[104,105]}]}],"branches":[{"id":"b","leaf_message_id":"m","canonical":true}]}]}"#;
    let normalized = normalize_agent(input).unwrap();
    let prepared = prepare(&normalized).unwrap();
    assert_eq!(prepared.receipt.source_namespace, "acct");
    assert_eq!(prepared.receipt.conversations[0].messages[0].source_id, "m");
    assert_eq!(
        prepared.receipt.conversations[0].messages[0].artifacts[0].source_ref,
        "ref"
    );
    assert_eq!(prepared.turns[0].content, "hello");
    assert_eq!(prepared.turns[0].attachments[0].bytes, b"hi");
    assert_eq!(prepared.receipt, prepare(&normalized).unwrap().receipt);
}
