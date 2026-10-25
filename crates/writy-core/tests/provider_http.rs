use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};
use writy_core::{
    config::Config,
    provider::ModelService,
    types::{Capability, Confidence, Language, TextSnapshot},
};

struct Server {
    url: String,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    task: JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start(chat: String, remote: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let end;
                loop {
                    let mut buffer = [0; 1024];
                    let n = socket.read(&mut buffer).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&buffer[..n]);
                    assert!(bytes.len() < 65536);
                    if let Some(p) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        end = p + 4;
                        break;
                    }
                }
                let header = String::from_utf8_lossy(&bytes[..end]);
                let path = header
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .to_owned();
                let size = header
                    .lines()
                    .filter_map(|s| s.split_once(':'))
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                    .unwrap_or(0);
                while bytes.len() < end + size {
                    let mut buffer = [0; 1024];
                    let n = socket.read(&mut buffer).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..n]);
                }
                let request = if size > 0 {
                    serde_json::from_slice(&bytes[end..end + size]).unwrap()
                } else {
                    Value::Null
                };
                captured.lock().unwrap().push((path.clone(), request));
                let metadata = if remote {
                    json!({"name":"qwen3:4b","remote_host":"https://ollama.com","details":{"format":""}})
                } else {
                    json!({"name":"qwen3:4b","details":{"format":"gguf"},"capabilities":["completion","thinking"]})
                };
                let body = match path.as_str() {
                    "/api/tags" => json!({"models":[metadata]}).to_string(),
                    "/api/show" => metadata.to_string(),
                    _ => chat.clone(),
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/x-ndjson\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        Self {
            url,
            requests,
            task,
        }
    }
    fn config(&self) -> Config {
        let mut c = Config {
            offline: true,
            ..Config::default()
        };
        c.model.ollama_url = self.url.clone();
        c.model.translation_model = "qwen3:4b".into();
        c.model.rewrite_model = "qwen3:4b".into();
        c
    }
}
fn snapshot() -> TextSnapshot {
    TextSnapshot {
        source: "synthetic-test".into(),
        focus_epoch: 1,
        revision: 1,
        language: Language::English,
        text: "A small synthetic sentence.".into(),
        confidence: Confidence::Verified,
    }
}
fn complete() -> String {
    "{\"message\":{\"content\":\"جملة قصيرة\"},\"done\":false}\n{\"message\":{\"content\":\"\"},\"done\":true,\"done_reason\":\"stop\"}\n".into()
}

#[tokio::test]
async fn offline_metadata_preflight_sends_no_prose_to_remote_stub() {
    let server = Server::start(complete(), true).await;
    let service = ModelService::new(server.config()).unwrap();
    assert!(
        service
            .generate(snapshot(), Capability::Rewrite)
            .await
            .is_err()
    );
    let requests = server.requests.lock().unwrap();
    assert!(!requests.iter().any(|(path, _)| path == "/api/chat"));
    assert!(
        requests
            .iter()
            .all(|(_, body)| !body.to_string().contains("synthetic sentence"))
    );
}

#[tokio::test]
async fn completed_arabic_stream_keeps_language_and_model_attribution() {
    let server = Server::start(complete(), false).await;
    let result = ModelService::new(server.config())
        .unwrap()
        .generate(snapshot(), Capability::Translate)
        .await
        .unwrap();
    assert_eq!(result.text, "جملة قصيرة");
    assert!(result.provider.contains("qwen3:4b"));
}

#[tokio::test]
async fn interrupted_and_truncated_outputs_are_never_returned() {
    for body in [
        "{\"message\":{\"content\":\"partial\"},\"done\":false}\n",
        "{\"message\":{\"content\":\"partial\"},\"done\":true,\"done_reason\":\"length\"}\n",
    ] {
        let server = Server::start(body.into(), false).await;
        assert!(
            ModelService::new(server.config())
                .unwrap()
                .generate(snapshot(), Capability::Rewrite)
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn missing_cloud_auth_is_reported_when_local_fallback_also_fails() {
    let server = Server::start(complete(), true).await;
    let mut config = server.config();
    config.offline = false;
    config.model.cloud_url = "https://example.invalid/v1".into();
    config.model.cloud_model = "synthetic-cloud-model".into();
    config.model.api_key_env = format!(
        "WRITY_TEST_MISSING_KEY_{}",
        server.url.rsplit(':').next().unwrap()
    );
    let result = ModelService::new(config)
        .unwrap()
        .generate(snapshot(), Capability::Rewrite)
        .await;
    let message = format!("{:#}", result.err().unwrap());
    assert!(message.contains("API key environment variable is missing"));
    assert!(message.contains("local"));
}

#[tokio::test]
async fn successful_fallback_keeps_cloud_failure_notice() {
    let server = Server::start(complete(), false).await;
    let mut config = server.config();
    config.offline = false;
    config.model.cloud_url = "https://example.invalid/v1".into();
    config.model.cloud_model = "synthetic".into();
    config.model.api_key_env = "WRITY_TEST_MISSING_SECRET_6f2b0e17".into();
    let result = ModelService::new(config)
        .unwrap()
        .generate(snapshot(), Capability::Rewrite)
        .await
        .unwrap();
    assert!(
        result
            .notice
            .unwrap()
            .contains("API key environment variable is missing")
    );
}

#[tokio::test]
async fn oversize_output_is_rejected() {
    let body = json!({"message":{"content":"x".repeat(8193)},"done":true}).to_string();
    let server = Server::start(body, false).await;
    assert!(
        ModelService::new(server.config())
            .unwrap()
            .generate(snapshot(), Capability::Rewrite)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn source_echo_is_not_returned_as_a_translation() {
    let body = format!(
        "{}\n",
        json!({"message":{"content":snapshot().text},"done":true,"done_reason":"stop"})
    );
    let server = Server::start(body, false).await;
    let error = ModelService::new(server.config())
        .unwrap()
        .generate(snapshot(), Capability::Translate)
        .await
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("repeated the source"));
}
