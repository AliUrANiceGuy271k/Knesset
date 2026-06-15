use axum::{
    extract::Query,
    routing::get,
    Json, Router,
};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// --- Datenstrukturen ---

#[derive(Deserialize)]
struct OppsQuery {
    user: String,
    max_games: Option<u32>,
    min_games: Option<u32>,
}

#[derive(Serialize, Clone)]
struct Opponent {
    name: String,
    games: u32,
    color: String,
}

// --- Logik: Gegner laden ---

async fn load_opps(user: &str, max_games: Option<u32>) -> Vec<Opponent> {
    let client = Client::new();
    let url = format!(
        "https://lichess.org/api/games/user/{}",
        user.to_lowercase()
    );

    let mut req = client.get(&url);
    if let Some(max) = max_games {
        req = req.query(&[("max", max.to_string())]);
    }

    let response = match req.send().await {
        Ok(r) => r,
        Err(_) => return vec![],
    };

    if !response.status().is_success() {
        return vec![];
    }

    let mut opps: HashMap<String, (u32, String, Option<String>)> = HashMap::new();
    let mut opp_order: Vec<String> = vec![];

    let re = Regex::new(r#""(.*?)""#).unwrap();

    let bytes = response.bytes().await.unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes);

    let mut last_name: Option<String> = None;

    for line in text.lines() {
        if line.starts_with("[White ") || line.starts_with("[Black ") {
            if let Some(cap) = re.captures(line) {
                let name2 = cap[1].to_string();
                if name2.to_lowercase() != user.to_lowercase() {
                    last_name = Some(name2.clone());

                    if let Some(entry) = opps.get_mut(&name2) {
                        entry.0 += 1;
                    } else {
                        opp_order.push(name2.clone());
                        opps.insert(name2, (1, "blue".to_string(), None));
                    }
                }
            }
        } else if line.starts_with("[UTCDate ") {
            if let Some(ref name) = last_name {
                if let Some(cap) = re.captures(line) {
                    let date = cap[1].to_string();
                    if let Some(entry) = opps.get_mut(name) {
                        match &entry.2 {
                            None => entry.2 = Some(date),
                            Some(existing_date) => {
                                if *existing_date != date {
                                    entry.1 = "red".to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut opps_list: Vec<Opponent> = opp_order
        .iter()
        .map(|name| {
            let (games, mut color, _) = opps[name].clone();
            if name.starts_with("lichess AI level ") {
                color = "blue".to_string();
            }
            Opponent {
                name: name.clone(),
                games,
                color,
            }
        })
        .collect();

    let n = opps_list.len();
    for _ in 0..n {
        for j in 0..n.saturating_sub(1) {
            if opps_list[j].games < opps_list[j + 1].games {
                opps_list.swap(j, j + 1);
            }
        }
    }

    opps_list
}

// --- Handler ---

async fn get_opps(Query(params): Query<OppsQuery>) -> Json<Vec<Opponent>> {
    let mut opps = load_opps(&params.user, params.max_games).await;

    if let Some(min) = params.min_games {
        opps.retain(|o| o.games >= min);
    }

    Json(opps)
}

// --- Main ---

#[tokio::main]
async fn main() {
    let app = Router::new().route("/opps", get(get_opps));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server läuft auf http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}