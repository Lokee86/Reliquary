use super::*;
use crate::test_support::mock_master_key_store;
use crate::{ConfigFile, ConfiguredRuntimeError, CredentialId, CredentialKind};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

#[test]
fn device_login_and_explicit_refresh_use_the_expected_exchange() {
    let keys = mock_master_key_store();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let claims = URL_SAFE_NO_PAD
            .encode(br#"{"https://api.openai.com/auth":{"chatgpt_account_id":"mock-account"}}"#);
        let id = format!("header.{claims}.signature");
        for (path, expected, status, body) in [
            (
                "/api/accounts/deviceauth/usercode",
                "client_id",
                "200 OK",
                serde_json::json!({"device_auth_id":"mock-device","user_code":"ABCD","interval":"1"}),
            ),
            (
                "/api/accounts/deviceauth/token",
                "mock-device",
                "200 OK",
                serde_json::json!({"authorization_code":"mock-authorization","code_verifier":"mock-verifier"}),
            ),
            (
                "/oauth/token",
                "grant_type=authorization_code",
                "200 OK",
                serde_json::json!({"id_token":id,"access_token":"access-1","refresh_token":"refresh-1"}),
            ),
            (
                "/oauth/token",
                "refresh-1",
                "200 OK",
                serde_json::json!({"access_token":"access-2","refresh_token":"refresh-2"}),
            ),
            (
                "/oauth/token",
                "refresh-2",
                "401 Unauthorized",
                serde_json::json!({"error":"response-should-stay-private"}),
            ),
        ] {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(&mut socket);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            assert_eq!(line.trim_end(), format!("POST {path} HTTP/1.1"));
            let mut length = None;
            loop {
                line.clear();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = Some(value.trim().parse().unwrap());
                }
            }
            let mut request = vec![0; length.unwrap()];
            reader.read_exact(&mut request).unwrap();
            assert!(String::from_utf8(request).unwrap().contains(expected));
            let body = body.to_string();
            write!(
                socket,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    let auth = DeviceAuth::with_issuer(&issuer, Duration::from_secs(5)).unwrap();
    let parent =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/oc02-fixtures");
    std::fs::create_dir_all(&parent).unwrap();
    let dir = tempfile::tempdir_in(parent).unwrap();
    let path = dir.path().join("device.cfg");
    let legacy_key = dir.path().join("device.master-key.json");
    let id = CredentialId::new("device").unwrap();
    let mut config = ConfigFile::new(&path);
    config
        .login_codex_with(
            id.clone(),
            |code| {
                assert_eq!(code.verification_url, format!("{issuer}/codex/device"));
                assert_eq!(code.user_code, "ABCD");
                assert!(!format!("{code:?}").contains("mock-device"));
            },
            &auth,
        )
        .unwrap();
    assert!(!path.exists() && !legacy_key.exists());
    config.save().unwrap();
    let original = std::fs::read(&path).unwrap();
    let key = keys.key(&path).unwrap();
    let mut reopened = ConfigFile::open(&path).unwrap();
    assert_eq!(
        reopened.credentials().collect::<Vec<_>>(),
        [(&id, CredentialKind::ChatGptDeviceCode)]
    );
    reopened
        .refresh_codex_with(&id, |saved| auth.refresh(saved))
        .unwrap();
    reopened.save().unwrap();
    let refreshed = std::fs::read(&path).unwrap();
    assert_ne!(original, refreshed);
    let mut reopened = ConfigFile::open(&path).unwrap();
    let error = reopened
        .refresh_codex_with(&id, |saved| auth.refresh(saved))
        .unwrap_err();
    assert_eq!(
        error,
        ConfiguredRuntimeError::Authorization(DeviceAuthError::Status(401))
    );
    assert!(!format!("{error:?} {error}").contains("response-should-stay-private"));
    assert_eq!(std::fs::read(&path).unwrap(), refreshed);
    assert!(*keys.key(&path).unwrap() == *key);
    assert!(!legacy_key.exists());
    server.join().unwrap();
}
