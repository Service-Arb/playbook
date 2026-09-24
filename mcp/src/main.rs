use std::collections::HashSet;

use serde::Deserialize;

const DAILY_BYTES: i64 = 2_000_000; // ~500k tokens a member a day

#[derive(Deserialize)]
struct Members {
	members: Vec<String>,
}

#[tokio::main]
async fn main() {
	let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is unset"));
	let Members { members } = toml::from_str(include_str!("../members.toml")).expect("members.toml is `members = [\"<email>\", …]`");
	let members: HashSet<String> = members.into_iter().map(|m| m.to_lowercase()).collect();
	let config = mcp::Config {
		public_url: env("PUBLIC_URL").trim_end_matches('/').to_owned(),
		google_client_id: env("GOOGLE_CLIENT_ID"),
		google_client_secret: env("GOOGLE_CLIENT_SECRET"),
		members,
		db: "mcp.db".into(),
		daily_bytes: DAILY_BYTES,
	};
	let addr = format!("0.0.0.0:{}", env("PORT"));
	let listener = tokio::net::TcpListener::bind(&addr).await.unwrap_or_else(|e| panic!("{addr}: {e}"));
	println!("serving on {addr}");
	axum::serve(listener, mcp::app(config)).with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() }).await.unwrap();
}
