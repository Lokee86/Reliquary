use crate::openai_codex_device_auth::test_device_auth;
use crate::{Credential, CredentialId, CredentialsConfig, OPENAI_CODEX_OAUTH_CLIENT_ID};
use base64::Engine;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
struct RequestCapture {
    path: String,
    body: String,
}

#[test]
fn codex_client_id_matches_current_public_client() {
    assert_eq!(OPENAI_CODEX_OAUTH_CLIENT_ID, "app_EMoamEEZ73f0CkXaXp7hrann");
}

#[test]
fn device_code_flow_requests_polls_exchanges_and_stores_chatgpt_oauth() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::<RequestCapture>::new()));
    let server_requests = Arc::clone(&requests);
    let id_token = jwt_with_account("acct-test");
    let server = thread::spawn(move || {
        serve_once(
            &listener,
            &server_requests,
            json!({
                "device_auth_id": "device-1",
                "user_code": "ABCD-1234",
                "interval": "1"
            }),
        );
        serve_once(
            &listener,
            &server_requests,
            json!({
                "authorization_code": "authorization-1",
                "code_challenge": "unused-challenge",
                "code_verifier": "verifier-1"
            }),
        );
        serve_once(
            &listener,
            &server_requests,
            json!({
                "id_token": id_token,
                "access_token": "access-1",
                "refresh_token": "refresh-1"
            }),
        );
    });

    let auth = test_device_auth(&issuer, "test-client", Duration::from_secs(5)).unwrap();
    let code = auth.request_device_code().unwrap();
    assert_eq!(code.verification_url, format!("{issuer}/codex/device"));
    assert_eq!(code.user_code, "ABCD-1234");

    let mut credentials = CredentialsConfig::default();
    let id = CredentialId::new("codex").unwrap();
    auth.complete_device_code(code, &mut credentials, id.clone())
        .unwrap();
    server.join().unwrap();

    let credential = credentials.get(&id).unwrap();
    let Credential::ChatGptOAuth {
        id_token,
        access_token,
        refresh_token,
        account_id,
    } = credential
    else {
        panic!("expected ChatGPT OAuth credential");
    };
    assert!(!id_token.expose().is_empty());
    assert_eq!(access_token.expose(), "access-1");
    assert_eq!(refresh_token.expose(), "refresh-1");
    assert_eq!(account_id.as_deref(), Some("acct-test"));

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[0].path, "/api/accounts/deviceauth/usercode");
    let request: Value = serde_json::from_str(&requests[0].body).unwrap();
    assert_eq!(request["client_id"], "test-client");
    assert_eq!(requests[1].path, "/api/accounts/deviceauth/token");
    let poll: Value = serde_json::from_str(&requests[1].body).unwrap();
    assert_eq!(poll["device_auth_id"], "device-1");
    assert_eq!(poll["user_code"], "ABCD-1234");
    assert_eq!(requests[2].path, "/oauth/token");
    assert!(requests[2].body.contains("grant_type=authorization_code"));
    assert!(requests[2].body.contains("code=authorization-1"));
    assert!(requests[2].body.contains("client_id=test-client"));
    assert!(requests[2].body.contains("code_verifier=verifier-1"));
}

#[test]
fn refresh_exchanges_stored_refresh_token_and_preserves_identity_binding() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::<RequestCapture>::new()));
    let server_requests = Arc::clone(&requests);
    let server = thread::spawn(move || {
        serve_once(
            &listener,
            &server_requests,
            json!({
                "access_token": "access-2",
                "refresh_token": "refresh-2"
            }),
        );
    });

    let auth = test_device_auth(&issuer, "test-client", Duration::from_secs(5)).unwrap();
    let mut credentials = CredentialsConfig::default();
    let id = CredentialId::new("codex").unwrap();
    credentials
        .insert_chatgpt_oauth(
            id.clone(),
            "id-token-1",
            "expired-access",
            "refresh-1",
            Some("acct-test".into()),
        )
        .unwrap();

    auth.refresh_credential(&mut credentials, &id).unwrap();
    server.join().unwrap();

    let Credential::ChatGptOAuth {
        id_token,
        access_token,
        refresh_token,
        account_id,
    } = credentials.get(&id).unwrap()
    else {
        panic!("expected ChatGPT OAuth credential");
    };
    assert_eq!(id_token.expose(), "id-token-1");
    assert_eq!(access_token.expose(), "access-2");
    assert_eq!(refresh_token.expose(), "refresh-2");
    assert_eq!(account_id.as_deref(), Some("acct-test"));

    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/oauth/token");
    let body: Value = serde_json::from_str(&requests[0].body).unwrap();
    assert_eq!(
        body,
        json!({
            "client_id": "test-client",
            "grant_type": "refresh_token",
            "refresh_token": "refresh-1"
        })
    );
}

fn serve_once(listener: &TcpListener, requests: &Arc<Mutex<Vec<RequestCapture>>>, response: Value) {
    let (mut stream, _) = listener.accept().unwrap();
    let request = read_request(&mut stream);
    requests.lock().unwrap().push(request);
    let body = serde_json::to_string(&response).unwrap();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .unwrap();
    stream.flush().unwrap();
}

fn read_request(stream: &mut TcpStream) -> RequestCapture {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    let path = first.split_whitespace().nth(1).unwrap().to_owned();
    let mut content_length = 0_usize;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" || line.is_empty() {
            break;
        }
        if let Some(value) = line
            .strip_prefix("content-length:")
            .or_else(|| line.strip_prefix("Content-Length:"))
        {
            content_length = value.trim().parse().unwrap();
        }
    }
    let mut body = vec![0_u8; content_length];
    reader.read_exact(&mut body).unwrap();
    RequestCapture {
        path,
        body: String::from_utf8(body).unwrap(),
    }
}

fn jwt_with_account(account_id: &str) -> String {
    let encode = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let header = encode(br#"{"alg":"none","typ":"JWT"}"#);
    let payload = encode(
        serde_json::to_string(&json!({
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account_id
            }
        }))
        .unwrap()
        .as_bytes(),
    );
    format!("{header}.{payload}.sig")
}
