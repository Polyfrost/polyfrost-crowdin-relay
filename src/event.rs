use std::collections::VecDeque;

use serde_json::Value;

pub struct Event {
    pub kind: String,
    pub raw: Value,
}

pub struct Project {
    pub id: String,
    pub name: String,
    pub url: Option<String>,
}

pub struct Language {
    pub id: String,
    pub name: String,
}

pub struct User {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
}

pub struct SourceString {
    pub key: String,
    pub text: String,
    pub url: Option<String>,
}

pub fn parse_body(body: &Value) -> Vec<Event> {
    match body.get("events").and_then(Value::as_array) {
        Some(list) => list.iter().filter_map(Event::from_value).collect(),
        None => Event::from_value(body).into_iter().collect(),
    }
}

impl Event {
    fn from_value(value: &Value) -> Option<Event> {
        let kind = value.get("event")?.as_str()?.to_owned();
        Some(Event { kind, raw: value.clone() })
    }

    pub fn project(&self) -> Option<Project> {
        let p = find(&self.raw, "project")?;
        Some(Project {
            id: text(p, "id").unwrap_or_default(),
            name: text(p, "name").or_else(|| text(p, "identifier"))?,
            url: text(p, "url"),
        })
    }

    pub fn language(&self) -> Option<Language> {
        let l = find(&self.raw, "targetLanguage")?;
        let id = text(l, "id")?;
        Some(Language { name: text(l, "name").unwrap_or_else(|| id.clone()), id })
    }

    pub fn user(&self) -> Option<User> {
        let u = find(&self.raw, "user").or_else(|| find(&self.raw, "taskCreator"))?;
        user_from(u)
    }

    pub fn string(&self) -> Option<SourceString> {
        let s = find(&self.raw, "string")?;
        let key = text(s, "key").or_else(|| text(s, "identifier")).or_else(|| text(s, "id"))?;
        let body = match s.get("text") {
            Some(Value::String(t)) => t.clone(),
            Some(Value::Object(forms)) => forms
                .get("other")
                .or_else(|| forms.values().next())
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            _ => String::new(),
        };
        Some(SourceString { key, text: body, url: text(s, "url") })
    }

    pub fn translation(&self) -> Option<String> {
        ["translation", "newTranslation"]
            .iter()
            .find_map(|k| self.raw.get(*k))
            .and_then(|t| text(t, "text"))
    }

    pub fn path(&self, path: &[&str]) -> Option<&Value> {
        path.iter().try_fold(&self.raw, |v, k| v.get(*k))
    }

    pub fn field(&self, path: &[&str], key: &str) -> Option<String> {
        self.path(path).and_then(|v| text(v, key))
    }
}

pub fn user_from(u: &Value) -> Option<User> {
    Some(User {
        id: text(u, "id").unwrap_or_default(),
        name: text(u, "fullName").or_else(|| text(u, "username"))?,
        avatar: text(u, "avatarUrl").filter(|a| a.starts_with("https://")),
    })
}

pub fn text(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(s) if !s.trim().is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn find<'a>(root: &'a Value, key: &str) -> Option<&'a Value> {
    let mut queue = VecDeque::from([root]);
    while let Some(value) = queue.pop_front() {
        if let Value::Object(map) = value {
            if let Some(found @ Value::Object(_)) = map.get(key) {
                return Some(found);
            }
            queue.extend(map.values().filter(|child| child.is_object()));
        }
    }
    None
}
