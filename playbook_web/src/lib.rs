//! The member surface: the playbook's `skill/`, `structured/` and `ref/` text, served as slices over
//! MCP to the Google accounts on `members.toml`. See `README.md`.

mod auth;
mod corpus;
mod tools;

use std::{collections::HashSet, path::PathBuf, sync::{Arc, LazyLock, Mutex}};

use axum::{Router, middleware, routing::get};
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager};

pub struct Config {
	/// where members reach the server, e.g. `https://<host>/playbook_mcp`; the OAuth issuer, the resource, and the path it is served under
	pub public_url: String,
	pub google_client_id: String,
	pub google_client_secret: String,
	pub members: HashSet<String>,
	pub db: PathBuf,
	pub daily_bytes: i64,
}

struct State {
	config: Config,
	resource_metadata: String, // RFC 9728's path-inserted location
	db: Mutex<rusqlite::Connection>, // ponytail: one lock for every query; a pool if calls ever queue on it
	http: reqwest::Client,
}

/// the email the guard admitted, carried on the request into the tools
#[derive(Clone)]
struct Member(String);

pub fn app(config: Config) -> Router {
	LazyLock::force(&corpus::CORPUS); // a malformed capture fails the start, not a member's call
	let db = rusqlite::Connection::open(&config.db).unwrap_or_else(|e| panic!("{}: {e}", config.db.display()));
	db.execute_batch(include_str!("schema.sql")).unwrap();
	let url = reqwest::Url::parse(&config.public_url).expect("PUBLIC_URL is a url");
	let (host, base) = (url.authority().to_owned(), url.path().to_owned());
	assert!(base.len() > 1, "PUBLIC_URL names the path the server is served under: {}", config.public_url);
	let state = Arc::new(State {
		resource_metadata: format!("{}/.well-known/oauth-protected-resource{base}", url.origin().ascii_serialization()),
		config,
		db: Mutex::new(db),
		http: reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().unwrap(),
	});

	let mcp = {
		let state = state.clone();
		StreamableHttpService::new(
			move || Ok(tools::Playbook::new(state.clone())),
			LocalSessionManager::default().into(),
			StreamableHttpServerConfig::default().with_allowed_hosts([host, "localhost".into(), "127.0.0.1".into()]),
		)
	};
	Router::new()
		.route_service(&base, mcp)
		.layer(middleware::from_fn_with_state(state.clone(), auth::guard))
		.merge(auth::routes(&base))
		.route("/health", get(|| async { "ok" }))
		.with_state(state)
}

fn now() -> i64 {
	std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}
