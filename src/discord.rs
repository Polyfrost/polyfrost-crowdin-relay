use std::time::Duration;

use futures_util::future::join_all;
use serde_json::Value;
use worker::{Delay, Fetch, Headers, Method, Request, RequestInit, console_error, wasm_bindgen::JsValue};

const MAX_ATTEMPTS: u32 = 4;

pub struct Delivery {
    pub label: String,
    pub url: String,
    pub messages: Vec<Value>,
}

pub async fn deliver(deliveries: Vec<Delivery>) {
    join_all(deliveries.into_iter().map(|d| async move {
        for message in &d.messages {
            send(&d.label, &d.url, message).await;
        }
    }))
    .await;
}

async fn send(label: &str, url: &str, body: &Value) {
    let separator = if url.contains('?') { '&' } else { '?' };
    let endpoint = format!("{url}{separator}with_components=true");
    let payload = body.to_string();

    for _ in 0..MAX_ATTEMPTS {
        let request = match build(&endpoint, &payload) {
            Ok(r) => r,
            Err(e) => return console_error!("[{label}] could not build request: {e}"),
        };
        match Fetch::Request(request).send().await {
            Ok(mut response) => {
                let status = response.status_code();
                if (200..300).contains(&status) {
                    return;
                }
                if status == 429 {
                    let wait = response
                        .json::<Value>()
                        .await
                        .ok()
                        .and_then(|v| v.get("retry_after")?.as_f64())
                        .unwrap_or(1.0);
                    Delay::from(Duration::from_millis((wait * 1000.0) as u64 + 100)).await;
                    continue;
                }
                let detail = response.text().await.unwrap_or_default();
                return console_error!("[{label}] Discord returned {status}: {detail}");
            }
            Err(e) => {
                console_error!("[{label}] request failed: {e}");
                Delay::from(Duration::from_millis(500)).await;
            }
        }
    }
    console_error!("[{label}] gave up after {MAX_ATTEMPTS} attempts");
}

fn build(endpoint: &str, payload: &str) -> worker::Result<Request> {
    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(payload)));
    Request::new_with_init(endpoint, &init)
}
