mod board;
mod json;
mod time;

use std::env;
use std::fs;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

struct Args {
    path: String,
    json_output: bool,
    limit: Option<usize>,
    list_filter: Option<String>,
}

fn parse_args() -> Result<Args, String> {
    let mut path = None;
    let mut json_output = false;
    let mut limit = None;
    let mut list_filter = None;

    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--json" => json_output = true,
            "--limit" => {
                let n = iter.next().ok_or("--limit requires a number")?;
                limit = Some(n.parse::<usize>().map_err(|_| "--limit expects a positive integer")?);
            }
            "--list" => {
                let name = iter.next().ok_or("--list requires a list name")?;
                list_filter = Some(name);
            }
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            other if path.is_none() => path = Some(other.to_string()),
            other => return Err(format!("unexpected argument '{}'", other)),
        }
    }

    let path = path.ok_or("missing path to a board export JSON file")?;
    Ok(Args { path, json_output, limit, list_filter })
}

fn print_usage() {
    println!("kanban-stale-cards - find the cards that have been sitting longest in their list\n");
    println!("USAGE:");
    println!("    kanban-stale-cards <export.json> [--json] [--limit N] [--list NAME]\n");
    println!("OPTIONS:");
    println!("    --json         emit machine-readable JSON instead of a table");
    println!("    --limit N      only show the N stalest cards");
    println!("    --list NAME    only show cards currently in the list NAME (case-insensitive)");
    println!("    -h, --help     show this message");
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {}", e);
            print_usage();
            process::exit(2);
        }
    };

    let contents = match fs::read_to_string(&args.path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: could not read '{}': {}", args.path, e);
            process::exit(1);
        }
    };

    let root = match json::parse(&contents) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: '{}' is not valid JSON: {}", args.path, e);
            process::exit(1);
        }
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut cards = match board::find_stale_cards(&root, now) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {}", e);
            process::exit(1);
        }
    };

    if let Some(list_name) = &args.list_filter {
        cards.retain(|c| c.list_name.eq_ignore_ascii_case(list_name));
    }

    cards.sort_by(|a, b| b.stale_seconds.cmp(&a.stale_seconds));
    if let Some(limit) = args.limit {
        cards.truncate(limit);
    }

    if args.json_output {
        print_json(&cards);
    } else {
        print_table(&cards);
    }
}

fn print_table(cards: &[board::StaleCard]) {
    if cards.is_empty() {
        println!("no open cards found in this export");
        return;
    }

    let name_width = cards.iter().map(|c| c.card_name.chars().count()).max().unwrap_or(4).max(4);
    let list_width = cards.iter().map(|c| c.list_name.chars().count()).max().unwrap_or(4).max(4);

    println!(
        "{:<name_width$}  {:<list_width$}  STALE",
        "CARD",
        "LIST",
        name_width = name_width,
        list_width = list_width
    );
    for card in cards {
        println!(
            "{:<name_width$}  {:<list_width$}  {}",
            card.card_name,
            card.list_name,
            time::format_duration(card.stale_seconds),
            name_width = name_width,
            list_width = list_width
        );
    }
}

fn print_json(cards: &[board::StaleCard]) {
    let mut out = String::from("[\n");
    for (i, card) in cards.iter().enumerate() {
        out.push_str("  {\"card\": \"");
        out.push_str(&json_escape(&card.card_name));
        out.push_str("\", \"list\": \"");
        out.push_str(&json_escape(&card.list_name));
        out.push_str("\", \"entered_at_unix\": ");
        out.push_str(&card.entered_at.to_string());
        out.push_str(", \"stale_seconds\": ");
        out.push_str(&card.stale_seconds.to_string());
        out.push_str(", \"stale_human\": \"");
        out.push_str(&json_escape(&time::format_duration(card.stale_seconds)));
        out.push_str("\"}");
        if i + 1 < cards.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push(']');
    println!("{}", out);
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
