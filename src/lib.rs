mod discord;
mod event;
mod render;

use discord::Delivery;
use event::Event;
use serde_json::Value;
use worker::{Context, Env, Method, Request, Response, Result, console_log, event};

#[event(fetch)]
async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    match (req.method(), req.path().as_str()) {
        (Method::Get, "/") => Response::ok("ok"),
        (Method::Post, "/crowdin") => handle(req, env, ctx).await,
        _ => Response::error("Not found", 404),
    }
}

async fn handle(mut req: Request, env: Env, ctx: Context) -> Result<Response> {
    let Ok(token) = env.secret("RELAY_TOKEN").map(|s| s.to_string()) else {
        return Response::error("RELAY_TOKEN is not set", 500);
    };
    if token.is_empty() || !authorized(&req, &token) {
        return Response::error("Unauthorized", 401);
    }
    let Ok(body) = req.json::<Value>().await else {
        return Response::error("Expected a JSON body", 400);
    };

    let events = event::parse_body(&body);
    if events.is_empty() {
        return Response::ok("no events");
    }
    let deliveries = plan(&env, &events);
    console_log!(
        "{} event(s) -> {} channel(s)",
        events.len(),
        deliveries.len()
    );
    ctx.wait_until(discord::deliver(deliveries));
    Response::ok("accepted")
}

fn plan(env: &Env, events: &[Event]) -> Vec<Delivery> {
    let mut deliveries = Vec::new();
    let all: Vec<&Event> = events.iter().collect();
    if let Some(url) = webhook(env, "GLOBAL") {
        deliveries.push(Delivery {
            label: "global".into(),
            url,
            messages: render::render(&all, true),
        });
    }

    let mut languages: Vec<String> = Vec::new();
    for id in events.iter().filter_map(|e| e.language().map(|l| l.id)) {
        if !languages.contains(&id) {
            languages.push(id);
        }
    }
    for id in languages {
        let Some(url) = webhook(env, &id.to_uppercase().replace('-', "_")) else {
            continue;
        };
        let subset: Vec<&Event> = events
            .iter()
            .filter(|e| e.language().is_some_and(|l| l.id == id))
            .collect();
        deliveries.push(Delivery {
            label: id,
            url,
            messages: render::render(&subset, false),
        });
    }
    deliveries
}

fn webhook(env: &Env, suffix: &str) -> Option<String> {
    env.secret(&format!("DISCORD_WEBHOOK_{suffix}"))
        .ok()
        .map(|s| s.to_string())
        .filter(|url| url.starts_with("https://"))
}

fn authorized(req: &Request, token: &str) -> bool {
    let headers = req.headers();
    let given = headers.get("X-Relay-Token").ok().flatten().or_else(|| {
        headers
            .get("Authorization")
            .ok()
            .flatten()
            .and_then(|h| h.strip_prefix("Bearer ").map(str::to_owned))
    });
    given.is_some_and(|g| constant_eq(g.as_bytes(), token.as_bytes()))
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
