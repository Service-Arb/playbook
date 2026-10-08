//! The member surface: the playbook's `skill/`, `structured/` and `ref/` text, served as slices over
//! MCP to the Service-Arb members holding `sa:playbook:mcp:use`, behind the panel. See `README.md`.

mod auth;
mod corpus;
mod tools;

use std::{path::PathBuf, sync::{Arc, LazyLock, Mutex}};

use axum::{Router, middleware, routing::get};
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager};

pub struct Config {
	/// where members reach the server, e.g. `https://<host>/playbook_mcp`; the OAuth issuer, the resource, and the path it is served under
	pub public_url: String,
	/// the panel's public keys, for the assertion it puts on `/authorize`
	pub panel_keys: sa_auth::Keys,
	pub db: PathBuf,
	pub daily_bytes: i64,
}

struct State {
	config: Config,
	resource_metadata: String, // RFC 9728's path-inserted location
	db: Mutex<rusqlite::Connection>, // ponytail: one lock for every query; a pool if calls ever queue on it
	http: reqwest::Client,
}

/// the member the guard admitted, carried on the request into the tools
#[derive(Clone)]
struct Member {
	sub: String,
	email: String,
}

pub fn app(config: Config) -> Router {
	LazyLock::force(&corpus::CORPUS); // a malformed capture fails the start, not a member's call
	let db = rusqlite::Connection::open(&config.db).unwrap_or_else(|e| panic!("{}: {e}", config.db.display()));
	db.pragma_update(None, "journal_mode", "WAL").unwrap();
	db.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
	db.execute_batch("BEGIN IMMEDIATE").unwrap(); // a start that dies mid-migration leaves the old schema whole
	match db.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap() {
		0 => {
			let va_sso: bool = db.query_row("SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE name = 'clients')", [], |r| r.get(0)).unwrap();
			if va_sso {
				db.execute_batch(include_str!("va_sso_to_sub.sql")).unwrap();
			}
		}
		1 => {}
		v => panic!("{} is at schema version {v}, newer than this server", config.db.display()),
	}
	db.execute_batch(include_str!("schema.sql")).unwrap();
	db.execute_batch("COMMIT").unwrap();
	let url = reqwest::Url::parse(&config.public_url).expect("PUBLIC_URL is a url");
	let base = url.path().to_owned();
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
			StreamableHttpServerConfig::default().disable_allowed_hosts(), // the panel forwards with the in-cluster Host; every request needs a bearer token, which DNS rebinding cannot carry
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
