use std::collections::HashMap;

use crate::json::Value;
use crate::time::parse_iso8601_utc;

pub struct StaleCard {
    pub card_name: String,
    pub list_name: String,
    pub entered_at: u64,
    pub stale_seconds: i64,
}

/// Walks a parsed Trello export and, for every open card, works out when it
/// most recently landed in the list it's currently sitting in.
///
/// The precise answer comes from the "actions" array (a `updateCard` action
/// whose `listAfter` matches the card's current list). Simplified exports
/// don't include actions at all, so we fall back to `dateLastActivity` on the
/// card itself, which is a looser approximation but better than nothing.
pub fn find_stale_cards(root: &Value, now: u64) -> Result<Vec<StaleCard>, String> {
    let lists_by_id = collect_lists(root)?;
    let cards = root
        .get("cards")
        .and_then(Value::as_array)
        .ok_or("export is missing a top-level \"cards\" array")?;

    let mut current_list: HashMap<String, String> = HashMap::new();
    let mut last_activity: HashMap<String, u64> = HashMap::new();
    let mut names: HashMap<String, String> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for card in cards {
        if card.get("closed").and_then(Value::as_bool).unwrap_or(false) {
            continue; // archived card, not something anyone needs to act on
        }
        let id = match card.get("id").and_then(Value::as_str) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let list_id = match card.get("idList").and_then(Value::as_str) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let name = card
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("(untitled card)")
            .to_string();
        if let Some(ts) = card
            .get("dateLastActivity")
            .and_then(Value::as_str)
            .and_then(parse_iso8601_utc)
        {
            last_activity.insert(id.clone(), ts);
        }
        order.push(id.clone());
        names.insert(id.clone(), name);
        current_list.insert(id, list_id);
    }

    let mut entered_at: HashMap<String, u64> = HashMap::new();
    if let Some(actions) = root.get("actions").and_then(Value::as_array) {
        for action in actions {
            if action.get("type").and_then(Value::as_str) != Some("updateCard") {
                continue;
            }
            let data = match action.get("data") {
                Some(d) => d,
                None => continue,
            };
            let card_id = match data.get("card").and_then(|c| c.get("id")).and_then(Value::as_str) {
                Some(s) => s,
                None => continue,
            };
            let list_after_id = match data.get("listAfter").and_then(|l| l.get("id")).and_then(Value::as_str) {
                Some(s) => s,
                None => continue,
            };
            // only care about the move into the list the card is *currently* in
            match current_list.get(card_id) {
                Some(target) if target == list_after_id => {}
                _ => continue,
            }
            let date = match action.get("date").and_then(Value::as_str).and_then(parse_iso8601_utc) {
                Some(d) => d,
                None => continue,
            };
            entered_at
                .entry(card_id.to_string())
                .and_modify(|existing| {
                    if date > *existing {
                        *existing = date;
                    }
                })
                .or_insert(date);
        }
    }

    let mut result = Vec::new();
    for id in order {
        let list_id = &current_list[&id];
        let list_name = lists_by_id
            .get(list_id)
            .cloned()
            .unwrap_or_else(|| "(unknown list)".to_string());
        let when = match entered_at.get(&id).copied().or_else(|| last_activity.get(&id).copied()) {
            Some(w) => w,
            None => continue, // no timestamp evidence at all, nothing to report
        };
        result.push(StaleCard {
            card_name: names.remove(&id).unwrap_or_default(),
            list_name,
            entered_at: when,
            stale_seconds: now as i64 - when as i64,
        });
    }

    Ok(result)
}

fn collect_lists(root: &Value) -> Result<HashMap<String, String>, String> {
    let lists = root
        .get("lists")
        .and_then(Value::as_array)
        .ok_or("export is missing a top-level \"lists\" array")?;
    let mut map = HashMap::new();
    for list in lists {
        let id = list.get("id").and_then(Value::as_str);
        let name = list.get("name").and_then(Value::as_str);
        if let (Some(id), Some(name)) = (id, name) {
            map.insert(id.to_string(), name.to_string());
        }
    }
    Ok(map)
}
