use serde_json::{Value, json};

use crate::event::{Event, SourceString, text, user_from};

const MAX_COMPONENTS: usize = 40;
const MAX_TEXT: usize = 3800;
const MAX_DETAILS: usize = 5;
const COMPONENTS_V2: u64 = 1 << 15;

const BLUE: u32 = 0x3B82F6;
const INDIGO: u32 = 0x6366F1;
const GREEN: u32 = 0x22C55E;
const RED: u32 = 0xEF4444;
const GREY: u32 = 0x6B7280;
const TEAL: u32 = 0x14B8A6;
const SLATE: u32 = 0x64748B;
const GOLD: u32 = 0xF59E0B;
const PURPLE: u32 = 0x8B5CF6;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bucket {
    Suggested,
    Changed,
    Approved,
    Rejected,
    Removed,
    Source,
    Files,
    Single,
}

fn bucket(kind: &str) -> Bucket {
    match kind {
        "suggestion.added" | "suggestion.updated" => Bucket::Suggested,
        "translation.updated" => Bucket::Changed,
        "suggestion.approved" => Bucket::Approved,
        "suggestion.disapproved" => Bucket::Rejected,
        "suggestion.deleted" => Bucket::Removed,
        "string.added" | "string.updated" | "string.deleted" => Bucket::Source,
        "file.added" | "file.updated" | "file.reverted" | "file.deleted" => Bucket::Files,
        _ => Bucket::Single,
    }
}

struct Card {
    accent: u32,
    avatar: Option<String>,
    title: String,
    summary: String,
    details: Vec<String>,
    footer: String,
    link: Option<String>,
}

pub fn render(events: &[&Event], show_language: bool) -> Vec<Value> {
    let cards = group(events)
        .into_iter()
        .map(|(b, evs)| card(b, &evs, show_language))
        .collect();
    pack(cards)
}

fn group<'a>(events: &[&'a Event]) -> Vec<(Bucket, Vec<&'a Event>)> {
    let mut groups: Vec<(Bucket, String, Vec<&'a Event>)> = Vec::new();
    for &e in events {
        let b = bucket(&e.kind);
        if b == Bucket::Single {
            groups.push((b, String::new(), vec![e]));
            continue;
        }
        let key = format!(
            "{}|{}|{}",
            e.project().map(|p| p.id).unwrap_or_default(),
            e.language().map(|l| l.id).unwrap_or_default(),
            e.user().map(|u| u.id).unwrap_or_default(),
        );
        match groups.iter_mut().find(|(gb, gk, _)| *gb == b && *gk == key) {
            Some(g) => g.2.push(e),
            None => groups.push((b, key, vec![e])),
        }
    }
    groups.into_iter().map(|(b, _, e)| (b, e)).collect()
}

fn card(b: Bucket, events: &[&Event], show_language: bool) -> Card {
    let first = events[0];
    let n = events.len();
    let user = first.user();
    let who = user.as_ref().map(|u| bold(&u.name)).unwrap_or_else(|| "Someone".into());
    let in_lang = match first.language() {
        Some(l) if show_language => format!(" in {}", bold(&l.name)),
        _ => String::new(),
    };
    let mut card = Card {
        accent: SLATE,
        avatar: user.and_then(|u| u.avatar),
        title: String::new(),
        summary: String::new(),
        details: Vec::new(),
        footer: footer(first, show_language),
        link: None,
    };
    let lines = |f: fn(&Event) -> Option<String>| events.iter().filter_map(|e| f(e)).collect::<Vec<_>>();

    match b {
        Bucket::Suggested => {
            card.accent = BLUE;
            card.title = "💡 New suggestions".into();
            card.summary = format!("{who} suggested {}{in_lang}.", count(n, "translation"));
            card.details = lines(translation_line);
        }
        Bucket::Changed => {
            card.accent = INDIGO;
            card.title = "✏️ Translations changed".into();
            card.summary = format!("{who} changed {}{in_lang}.", count(n, "translation"));
            card.details = lines(translation_line);
        }
        Bucket::Approved => {
            card.accent = GREEN;
            card.title = "✅ Translations approved".into();
            card.summary = format!("{who} approved {}{in_lang}.", count(n, "translation"));
            card.details = lines(translation_line);
        }
        Bucket::Rejected => {
            card.accent = RED;
            card.title = "👎 Suggestions rejected".into();
            card.summary = format!("{who} rejected {}{in_lang}.", count(n, "suggestion"));
            card.details = lines(translation_line);
        }
        Bucket::Removed => {
            card.accent = GREY;
            card.title = "🗑️ Suggestions removed".into();
            card.summary = format!("{who} removed {}{in_lang}.", count(n, "suggestion"));
            card.details = lines(translation_line);
        }
        Bucket::Source => {
            card.accent = TEAL;
            card.title = "📝 Source text changed".into();
            card.summary = source_summary(&who, events);
            card.details = lines(source_line);
        }
        Bucket::Files => {
            card.accent = SLATE;
            card.title = "📁 Files changed".into();
            card.summary = if n == 1 {
                format!("{who} {} {}.", file_verb(&first.kind), file_name(first))
            } else {
                format!("{who} changed {n} files.")
            };
            if n > 1 {
                card.details = events
                    .iter()
                    .map(|e| format!("{} {}", capitalize(file_verb(&e.kind)), file_name(e)))
                    .collect();
            }
        }
        Bucket::Single => single(&mut card, first, &who, &in_lang),
    }

    if card.details.len() > MAX_DETAILS {
        let more = card.details.len() - MAX_DETAILS;
        card.details.truncate(MAX_DETAILS);
        card.details.push(format!("-# …and {more} more"));
    }
    card
}

fn single(card: &mut Card, e: &Event, who: &str, in_lang: &str) {
    let kind = e.kind.as_str();
    match kind {
        "file.translated" | "file.approved" | "project.translated" | "project.approved"
        | "branch.translated" | "branch.approved" => {
            let approved = kind.ends_with("approved");
            let language = e.language().map(|l| md(&l.name)).unwrap_or_else(|| "A language".into());
            card.avatar = None;
            card.accent = if approved { GREEN } else { GOLD };
            card.title = if approved {
                format!("🏆 {language} is fully approved")
            } else {
                format!("🎉 {language} is fully translated")
            };
            let scope = match kind.split('.').next() {
                Some("file") => format!("Every string in {}", file_name(e)),
                Some("branch") => format!(
                    "Every string on the {} branch",
                    code(&e.field(&["branch"], "name").unwrap_or_default())
                ),
                _ => "Every string in the project".into(),
            };
            card.summary = format!("{scope} is {}.", if approved { "approved" } else { "translated" });
        }
        _ if kind.starts_with("stringComment.") => comment(card, e, in_lang),
        _ if kind.starts_with("task.") => task(card, e),
        "project.built" => {
            card.avatar = None;
            card.accent = TEAL;
            card.title = "📦 Build finished".into();
            card.summary = "A fresh translation build is ready.".into();
        }
        "preTranslation.completed" => {
            card.accent = PURPLE;
            card.title = "🤖 Pre-translation finished".into();
            let langs = e
                .path(&["preTranslation", "languages"])
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            card.summary = format!("{who} pre-translated {}.", count(langs.len().max(1), "language"));
            card.details = langs
                .iter()
                .map(|l| {
                    let id = text(l, "id").unwrap_or_default();
                    let stats = l.get("statistics");
                    let phrases = stats.and_then(|s| text(s, "phrases")).unwrap_or_else(|| "0".into());
                    let words = stats.and_then(|s| text(s, "words")).unwrap_or_else(|| "0".into());
                    format!("{} · {phrases} strings, {words} words", code(&id))
                })
                .collect();
            if let Some(err) = e.field(&["preTranslation"], "errorMessage") {
                card.accent = RED;
                card.details.insert(0, format!("Failed: {}", md(&oneline(&err, 200))));
            }
        }
        _ => {
            card.accent = GREY;
            card.title = format!("🔔 {}", code(kind));
            card.summary = format!("{who} triggered {}{in_lang}.", code(kind));
        }
    }
}

fn comment(card: &mut Card, e: &Event, in_lang: &str) {
    let c = e.path(&["comment"]).cloned().unwrap_or(Value::Null);
    let is_issue = text(&c, "type").as_deref() == Some("issue");
    let resolved = text(&c, "issueStatus").as_deref() == Some("resolved");
    let action = e.kind.rsplit('.').next().unwrap_or_default();
    let author = e.user().map(|u| bold(&u.name)).unwrap_or_else(|| "Someone".into());
    let target = e.string().map(|s| key_link(&s)).unwrap_or_else(|| "a string".into());

    let (accent, title, summary) = match (action, is_issue) {
        ("updated", true) if resolved => {
            let resolver = c.get("commentResolver").and_then(user_from);
            if let Some(r) = &resolver {
                card.avatar = r.avatar.clone();
            }
            let who = resolver.map(|r| bold(&r.name)).unwrap_or(author);
            (GREEN, "✔️ Issue resolved", format!("{who} resolved an issue on {target}{in_lang}."))
        }
        ("created", true) => {
            let verb = match text(&c, "issueType").as_deref() {
                Some("source_mistake") => "reported a source mistake on",
                Some("translation_mistake") => "reported a translation mistake on",
                Some("context_request") => "asked for context on",
                Some("general_question") => "asked a question about",
                _ => "opened an issue on",
            };
            (RED, "🚩 New issue", format!("{author} {verb} {target}{in_lang}."))
        }
        ("created", false) => (PURPLE, "💬 New comment", format!("{author} commented on {target}{in_lang}.")),
        ("updated", _) => (PURPLE, "✏️ Comment edited", format!("{author} edited a comment on {target}{in_lang}.")),
        ("deleted", _) => (GREY, "🗑️ Comment deleted", format!("{author}'s comment on {target} was deleted.")),
        ("restored", _) => (PURPLE, "♻️ Comment restored", format!("{author}'s comment on {target} was restored.")),
        _ => (PURPLE, "💬 Comment", format!("{author} on {target}{in_lang}.")),
    };
    card.accent = accent;
    card.title = title.into();
    card.summary = summary;

    if let Some(body) = text(&c, "text") {
        card.details.push(quote(&body, 600));
    }
    if let Some(s) = e.string().filter(|s| !s.text.is_empty()) {
        card.details.push(format!("-# Source: {}", md(&oneline(&s.text, 160))));
    }
    card.link = e.string().and_then(|s| s.url).filter(|u| u.starts_with("https://"));
}

fn task(card: &mut Card, e: &Event) {
    card.accent = BLUE;
    card.title = match e.kind.as_str() {
        "task.added" => "📋 New task",
        "task.statusChanged" => "📋 Task status changed",
        "task.updated" => "📋 Task updated",
        "task.deleted" => "📋 Task deleted",
        _ => "📋 Task",
    }
    .into();
    card.summary = bold(&e.field(&["task"], "title").unwrap_or_else(|| "Untitled task".into()));
    let status = e.field(&[], "newStatus").or_else(|| e.field(&["task"], "status"));
    if let Some(status) = status {
        card.details.push(format!("Status: {}", md(&capitalize(&status.replace('_', " ")))));
    }
    if let Some(percent) = e.field(&["task", "progress"], "percent") {
        card.details.push(format!("Progress: {percent}%"));
    }
    if let Some(deadline) = e.field(&["task"], "deadline") {
        card.details.push(format!("Due: {}", md(deadline.get(..10).unwrap_or(&deadline))));
    }
}

fn translation_line(e: &Event) -> Option<String> {
    let s = e.string()?;
    let mut line = key_link(&s);
    if let Some(t) = e.translation() {
        line.push('\n');
        line.push_str(&md(&oneline(&t, 160)));
    }
    if let Some(old) = e.field(&["oldTranslation"], "text") {
        line.push_str(&format!("\n-# Was: {}", md(&oneline(&old, 120))));
    } else if !s.text.is_empty() {
        line.push_str(&format!("\n-# {}", md(&oneline(&s.text, 120))));
    }
    Some(line)
}

fn source_line(e: &Event) -> Option<String> {
    let s = e.string()?;
    let label = match e.kind.as_str() {
        "string.added" => "New",
        "string.updated" => "Edited",
        _ => "Removed",
    };
    let mut line = format!("{label} {}", key_link(&s));
    if !s.text.is_empty() {
        line.push_str(&format!("\n-# {}", md(&oneline(&s.text, 140))));
    }
    Some(line)
}

fn source_summary(who: &str, events: &[&Event]) -> String {
    let tally = |kind: &str| events.iter().filter(|e| e.kind == kind).count();
    let parts: Vec<(usize, &str)> = [
        (tally("string.added"), "added"),
        (tally("string.updated"), "updated"),
        (tally("string.deleted"), "removed"),
    ]
    .into_iter()
    .filter(|(c, _)| *c > 0)
    .collect();
    match parts.as_slice() {
        [(c, verb)] => format!("{who} {verb} {}.", count(*c, "source string")),
        _ => {
            let list: Vec<String> = parts.iter().map(|(c, v)| format!("{c} {v}")).collect();
            format!("{who} changed {} ({}).", count(events.len(), "source string"), list.join(", "))
        }
    }
}

fn footer(e: &Event, show_language: bool) -> String {
    let mut parts = Vec::new();
    if let Some(p) = e.project() {
        parts.push(match p.url {
            Some(url) if url.starts_with("https://") => format!("[{}](<{url}>)", md(&p.name)),
            _ => md(&p.name),
        });
    }
    if show_language && let Some(l) = e.language() {
        parts.push(md(&l.name));
    }
    parts.join(" · ")
}

impl Card {
    fn build(self) -> (Value, usize, usize) {
        let text_display = |content: String| json!({ "type": 10, "content": content });
        let mut parts: Vec<Value> = Vec::new();

        let header = format!("### {}\n{}", self.title, self.summary);
        parts.push(match self.avatar {
            Some(url) => json!({
                "type": 9,
                "components": [text_display(header)],
                "accessory": { "type": 11, "media": { "url": url } },
            }),
            None => text_display(header),
        });
        if !self.details.is_empty() {
            parts.push(json!({ "type": 14, "divider": true, "spacing": 1 }));
            parts.push(text_display(self.details.join("\n\n")));
        }
        if !self.footer.is_empty() {
            parts.push(text_display(format!("-# {}", self.footer)));
        }
        if let Some(url) = self.link {
            parts.push(json!({
                "type": 1,
                "components": [{ "type": 2, "style": 5, "label": "Open in Crowdin", "url": url }],
            }));
        }
        let container = json!({ "type": 17, "accent_color": self.accent, "components": parts });
        let (count, chars) = weigh(&container);
        (container, count, chars)
    }
}

fn weigh(component: &Value) -> (usize, usize) {
    let mut count = 1;
    let mut chars = component.get("content").and_then(Value::as_str).map_or(0, |c| c.chars().count());
    let children = component
        .get("components")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(component.get("accessory"));
    for child in children {
        let (n, c) = weigh(child);
        count += n;
        chars += c;
    }
    (count, chars)
}

fn pack(cards: Vec<Card>) -> Vec<Value> {
    let mut messages = Vec::new();
    let mut current = Vec::new();
    let (mut used, mut chars) = (0, 0);
    for card in cards {
        let (container, n, len) = card.build();
        if !current.is_empty() && (used + n > MAX_COMPONENTS || chars + len > MAX_TEXT) {
            messages.push(message(std::mem::take(&mut current)));
            used = 0;
            chars = 0;
        }
        current.push(container);
        used += n;
        chars += len;
    }
    if !current.is_empty() {
        messages.push(message(current));
    }
    messages
}

fn message(components: Vec<Value>) -> Value {
    json!({
        "flags": COMPONENTS_V2,
        "components": components,
        "allowed_mentions": { "parse": [] },
    })
}

fn key_link(s: &SourceString) -> String {
    match &s.url {
        Some(url) if url.starts_with("https://") => format!("[{}](<{url}>)", code(&s.key)),
        _ => code(&s.key),
    }
}

fn file_name(e: &Event) -> String {
    let name = e.field(&["file"], "path").or_else(|| e.field(&["file"], "name")).unwrap_or_else(|| "a file".into());
    code(&name)
}

fn file_verb(kind: &str) -> &'static str {
    match kind {
        "file.added" => "added",
        "file.reverted" => "reverted",
        "file.deleted" => "deleted",
        _ => "updated",
    }
}

fn count(n: usize, noun: &str) -> String {
    match n {
        1 => format!("a {noun}"),
        _ => format!("{n} {noun}s"),
    }
}

fn bold(s: &str) -> String {
    format!("**{}**", md(s))
}

fn code(s: &str) -> String {
    format!("`{}`", oneline(s, 80).replace('`', "'"))
}

fn quote(s: &str, max: usize) -> String {
    clip(s, max).lines().map(|l| format!("> {}", md(l))).collect::<Vec<_>>().join("\n")
}

fn md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(ch, '\\' | '*' | '_' | '~' | '`' | '|' | '[' | ']' | '<' | '>' | '#' | '-' | '@') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn oneline(s: &str, max: usize) -> String {
    clip(&s.split_whitespace().collect::<Vec<_>>().join(" "), max)
}

fn clip(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.truncate(out.trim_end().len());
    out.push('…');
    out
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
