//! OAuth 2.1 authorization server for the MCP clients. Who the member is comes from the
//! panel's assertion on `/authorize` ([`sa_auth`]). The flow and the tables are in `README.md`.

use std::sync::Arc;

use axum::{
	Form, Json, Router,
	extract::{Query, Request, State as Axum},
	http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header},
	middleware::Next,
	response::{Html, IntoResponse, Redirect, Response},
	routing::{get, post},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{Member, State, now};

type S = Axum<Arc<State>>;

const CODE_TTL: i64 = 60;
const ACCESS_TTL: i64 = 60 * 60;
const REFRESH_TTL: i64 = 7 * 24 * 60 * 60; // from authorize: rotation does not extend it, so the permission is re-checked weekly
const CONSENT_TTL: i64 = 10 * 60;
const DAY: i64 = 24 * 60 * 60;
const CIMD_MAX_BYTES: usize = 64 * 1024;

/// `base` is `PUBLIC_URL`'s path; the metadata sits where RFC 8414 and 9728 insert it
pub(crate) fn routes(base: &str) -> Router<Arc<State>> {
	Router::new()
		.route(&format!("/.well-known/oauth-protected-resource{base}"), get(resource_metadata))
		.route(&format!("/.well-known/oauth-authorization-server{base}"), get(server_metadata))
		.route(&format!("{base}/register"), post(register))
		.route(&format!("{base}/authorize"), get(authorize).post(consent))
		.route(&format!("{base}/token"), post(token))
}

/// Admits a request to the MCP endpoint only with a live access token of a member under their daily budget.
pub(crate) async fn guard(Axum(state): S, mut req: Request, next: Next) -> Response {
	let unauthorized = || {
		let challenge = format!("Bearer resource_metadata=\"{}\"", state.resource_metadata);
		(StatusCode::UNAUTHORIZED, [(header::WWW_AUTHENTICATE, challenge)]).into_response()
	};
	let Some(token) = req.headers().get(header::AUTHORIZATION).and_then(|h| h.to_str().ok()).and_then(|h| h.strip_prefix("Bearer ")) else {
		return unauthorized();
	};
	let member: Option<(String, String)> = state
		.db
		.lock()
		.unwrap()
		.query_row("SELECT sub, email FROM tokens WHERE hash = ?1 AND kind = 'access' AND expires > ?2", (hash(token), now()), |r| {
			Ok((r.get(0)?, r.get(1)?))
		})
		.optional()
		.unwrap();
	let Some((sub, email)) = member else { return unauthorized() };
	let today = now() - now() % DAY;
	let spent: i64 = state
		.db
		.lock()
		.unwrap()
		.query_row("SELECT COALESCE(SUM(bytes), 0) FROM calls WHERE sub = ?1 AND at >= ?2", (&sub, today), |r| r.get(0))
		.unwrap();
	if spent >= state.config.daily_bytes {
		let retry = (today + DAY - now()).to_string();
		return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, retry)], format!("{email} has used today's {} bytes; the budget resets at 00:00 UTC", state.config.daily_bytes)).into_response();
	}
	req.extensions_mut().insert(Member { sub, email });
	next.run(req).await
}

async fn resource_metadata(Axum(state): S) -> Json<Value> {
	let base = &state.config.public_url;
	Json(json!({
		"resource": base,
		"authorization_servers": [base],
		"bearer_methods_supported": ["header"],
	}))
}

async fn server_metadata(Axum(state): S) -> Json<Value> {
	let base = &state.config.public_url;
	Json(json!({
		"issuer": base,
		"authorization_endpoint": format!("{base}/authorize"),
		"token_endpoint": format!("{base}/token"),
		"registration_endpoint": format!("{base}/register"),
		"response_types_supported": ["code"],
		"grant_types_supported": ["authorization_code", "refresh_token"],
		"code_challenge_methods_supported": ["S256"],
		"token_endpoint_auth_methods_supported": ["none"],
		"client_id_metadata_document_supported": true,
	}))
}

#[derive(Deserialize)]
struct Registration {
	redirect_uris: Vec<String>,
	client_name: Option<String>,
}

async fn register(Axum(state): S, Json(reg): Json<Registration>) -> Response {
	if reg.redirect_uris.is_empty() {
		return oauth_error("invalid_redirect_uri", "at least one redirect_uri");
	}
	if let Some(bad) = reg.redirect_uris.iter().find(|u| !redirect_allowed(u)) {
		return oauth_error("invalid_redirect_uri", &format!("{bad}: https, or http on loopback"));
	}
	let id = random();
	state
		.db
		.lock()
		.unwrap()
		.execute(
			"INSERT INTO clients (id, redirect_uris, created, name) VALUES (?1, ?2, ?3, ?4)",
			(&id, serde_json::to_string(&reg.redirect_uris).unwrap(), now(), &reg.client_name),
		)
		.unwrap();
	let body = json!({
		"client_id": id,
		"client_id_issued_at": now(),
		"redirect_uris": reg.redirect_uris,
		"token_endpoint_auth_method": "none",
		"grant_types": ["authorization_code", "refresh_token"],
		"response_types": ["code"],
	});
	(StatusCode::CREATED, Json(body)).into_response()
}

fn redirect_allowed(uri: &str) -> bool {
	match reqwest::Url::parse(uri) {
		Ok(u) => u.scheme() == "https" || (u.scheme() == "http" && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))),
		Err(_) => false,
	}
}

#[derive(Deserialize)]
struct AuthorizeQuery {
	response_type: String,
	client_id: String,
	redirect_uri: String,
	code_challenge: String,
	code_challenge_method: String,
	state: Option<String>,
	resource: Option<String>,
}

/// The panel's assertion on this request, admitting only holders of `sa:playbook:mcp:use`.
fn member(state: &State, headers: &HeaderMap, method: &Method, uri: &Uri) -> Result<sa_auth::Assertion, Response> {
	let token = headers.get(sa_auth::HEADER).ok_or_else(|| (StatusCode::UNAUTHORIZED, "reach the playbook through https://sa.evinvest.ltd").into_response())?;
	let token = token.to_str().map_err(|_| (StatusCode::UNAUTHORIZED, "the panel's assertion is not ascii").into_response())?;
	let assertion = sa_auth::verify(&state.config.panel_keys, token, sa_auth::Service::Playbook, method.as_str(), uri.path(), now())
		.map_err(|e| (StatusCode::UNAUTHORIZED, format!("the panel's assertion is refused: {e}")).into_response())?;
	if !assertion.permissions.may(sa_auth::Mcp::Use) {
		let page = format!(
			"<!doctype html><title>No access</title><p><b>{}</b> has no access to the playbook. Ask an admin to grant it <code>sa:playbook:mcp:use</code>, then connect again.</p>",
			escape(&assertion.email)
		);
		return Err((StatusCode::FORBIDDEN, Html(page)).into_response());
	}
	Ok(assertion)
}

/// Asks the member to confirm; the code is issued by the form's POST, never here.
async fn authorize(Axum(state): S, method: Method, uri: Uri, headers: HeaderMap, Query(q): Query<AuthorizeQuery>) -> Response {
	let member = match member(&state, &headers, &method, &uri) {
		Ok(m) => m,
		Err(r) => return r,
	};
	if q.response_type != "code" || q.code_challenge_method != "S256" {
		return (StatusCode::BAD_REQUEST, "response_type=code with code_challenge_method=S256 only").into_response();
	}
	if let Some(resource) = &q.resource
		&& resource.trim_end_matches('/') != state.config.public_url
	{
		return (StatusCode::BAD_REQUEST, format!("{resource} is not served here")).into_response();
	}
	let name = match client(&state, &q.client_id, &q.redirect_uri).await {
		Ok(name) => name,
		Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
	};
	let nonce = random();
	{
		let db = state.db.lock().unwrap();
		db.execute("DELETE FROM consents WHERE expires <= ?1", [now()]).unwrap();
		db.execute(
			"INSERT INTO consents (hash, sub, client_id, redirect_uri, challenge, state, expires) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
			(hash(&nonce), &member.sub, &q.client_id, &q.redirect_uri, &q.code_challenge, &q.state, now() + CONSENT_TTL),
		)
		.unwrap();
	}
	let host = reqwest::Url::parse(&q.redirect_uri).expect("matched a registered redirect_uri").host_str().expect("https or loopback http").to_owned();
	let page = format!(
		r#"<!doctype html><meta charset="utf-8"><title>Connect the playbook</title>
<p>Signed in as <b>{email}</b>.</p>
<p><b>{client}</b> asks to read the playbook as you, and will be sent back to <b>{host}</b>.</p>
<form method="post" action="{action}"><input type="hidden" name="nonce" value="{nonce}"><button>Allow</button></form>"#,
		email = escape(&member.email),
		client = escape(name.as_deref().unwrap_or(&q.client_id)), // client_name is optional in RFC 7591 and CIMD
		host = escape(&host),
		action = escape(&format!("{}/authorize", state.config.public_url)),
	);
	([(header::CONTENT_SECURITY_POLICY, "frame-ancestors 'none'")], Html(page)).into_response()
}

#[derive(Deserialize)]
struct ConsentForm {
	nonce: String,
}

/// The consent page's POST: the same member, with a nonce `authorize` handed them and nobody spent yet.
async fn consent(Axum(state): S, method: Method, uri: Uri, headers: HeaderMap, Form(f): Form<ConsentForm>) -> Response {
	let member = match member(&state, &headers, &method, &uri) {
		Ok(m) => m,
		Err(r) => return r,
	};
	let db = state.db.lock().unwrap();
	let pending: Option<(String, String, String, Option<String>)> = db
		.query_row(
			"DELETE FROM consents WHERE hash = ?1 AND sub = ?2 AND expires > ?3 RETURNING client_id, redirect_uri, challenge, state",
			(hash(&f.nonce), &member.sub, now()),
			|r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
		)
		.optional()
		.unwrap();
	let Some((client_id, redirect_uri, challenge, oauth_state)) = pending else {
		return (StatusCode::BAD_REQUEST, "this consent form is spent, expired, or someone else's; connect again from Claude Code").into_response();
	};
	let code = random();
	db.execute(
		"INSERT INTO codes (hash, client_id, redirect_uri, challenge, sub, email, expires) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
		(hash(&code), &client_id, &redirect_uri, &challenge, &member.sub, member.email.to_lowercase(), now() + CODE_TTL),
	)
	.unwrap();
	let mut back = reqwest::Url::parse(&redirect_uri).expect("a registered redirect_uri");
	back.query_pairs_mut().append_pair("code", &code);
	if let Some(s) = &oauth_state {
		back.query_pairs_mut().append_pair("state", s);
	}
	Redirect::to(back.as_str()).into_response()
}

fn escape(s: &str) -> String {
	s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

/// A `client_id` is either a CIMD document's https URL, or an id `/register` handed out; its name, if it gave one.
async fn client(state: &State, client_id: &str, redirect_uri: &str) -> Result<Option<String>, String> {
	let (uris, name): (Vec<String>, Option<String>) = if client_id.starts_with("https://") {
		let mut res = state.http.get(client_id).send().await.map_err(|e| format!("fetching the client metadata: {e}"))?;
		let mut body = Vec::new();
		while let Some(chunk) = res.chunk().await.map_err(|e| format!("reading the client metadata: {e}"))? {
			body.extend_from_slice(&chunk);
			if body.len() > CIMD_MAX_BYTES {
				return Err("the client metadata document is over 64 KiB".into());
			}
		}
		let doc: Value = serde_json::from_slice(&body).map_err(|e| format!("the client metadata is not json: {e}"))?;
		if doc["client_id"] != client_id {
			return Err("the client metadata document names a different client_id".into());
		}
		let uris = serde_json::from_value(doc["redirect_uris"].clone()).map_err(|_| "the client metadata has no redirect_uris")?;
		let name = match &doc["client_name"] {
			Value::Null => None,
			Value::String(n) => Some(n.clone()),
			_ => return Err("the client metadata's client_name is not a string".into()),
		};
		(uris, name)
	} else {
		let stored: Option<(String, Option<String>)> = state
			.db
			.lock()
			.unwrap()
			.query_row("SELECT redirect_uris, name FROM clients WHERE id = ?1", [client_id], |r| Ok((r.get(0)?, r.get(1)?)))
			.optional()
			.unwrap();
		let (uris, name) = stored.ok_or("unknown client_id; register first")?;
		(serde_json::from_str(&uris).expect("written by /register"), name)
	};
	// RFC 8252 §7.3: a loopback redirect is registered without the port the client ends up listening on
	let portless = |u: &str| {
		let mut u = reqwest::Url::parse(u).ok()?;
		(u.scheme() == "http" && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))).then(|| u.set_port(None).expect("http takes a port"))?;
		Some(u)
	};
	match uris.iter().any(|u| u == redirect_uri || portless(u).is_some_and(|u| Some(u) == portless(redirect_uri))) {
		true => Ok(name),
		false => Err(format!("{redirect_uri} is not a redirect_uri of this client")),
	}
}

#[derive(Deserialize)]
struct TokenForm {
	grant_type: String,
	client_id: String,
	code: Option<String>,
	code_verifier: Option<String>,
	redirect_uri: Option<String>,
	refresh_token: Option<String>,
}

async fn token(Axum(state): S, Form(f): Form<TokenForm>) -> Response {
	let (sub, email, chain_ends) = match f.grant_type.as_str() {
		"authorization_code" => {
			let (Some(code), Some(verifier), Some(redirect_uri)) = (&f.code, &f.code_verifier, &f.redirect_uri) else {
				return oauth_error("invalid_request", "code, code_verifier and redirect_uri");
			};
			let db = state.db.lock().unwrap();
			let row: Option<(String, String, String, String, String)> = db
				.query_row("SELECT client_id, redirect_uri, challenge, sub, email FROM codes WHERE hash = ?1 AND expires > ?2", (hash(code), now()), |r| {
					Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
				})
				.optional()
				.unwrap();
			db.execute("DELETE FROM codes WHERE hash = ?1 OR expires <= ?2", (hash(code), now())).unwrap();
			match row {
				Some((client, redirect, challenge, sub, email))
					if client == f.client_id && &redirect == redirect_uri && URL_SAFE_NO_PAD.encode(Sha256::digest(verifier)) == challenge =>
					(sub, email, now() + REFRESH_TTL),
				_ => return oauth_error("invalid_grant", "the code is spent, expired, or not this client's"),
			}
		}
		"refresh_token" => {
			let Some(refresh) = &f.refresh_token else { return oauth_error("invalid_request", "refresh_token") };
			let db = state.db.lock().unwrap();
			let row: Option<(String, String, i64)> = db
				.query_row(
					"DELETE FROM tokens WHERE hash = ?1 AND kind = 'refresh' AND client_id = ?2 AND expires > ?3 RETURNING sub, email, expires",
					(hash(refresh), &f.client_id, now()),
					|r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
				)
				.optional()
				.unwrap();
			match row {
				Some(row) => row,
				None => return oauth_error("invalid_grant", "the refresh token is spent, expired, or not this client's"),
			}
		}
		_ => return oauth_error("unsupported_grant_type", "authorization_code or refresh_token"),
	};
	let (access, refresh) = (random(), random());
	{
		let db = state.db.lock().unwrap();
		db.execute("DELETE FROM tokens WHERE expires <= ?1", [now()]).unwrap();
		let insert = "INSERT INTO tokens (hash, kind, sub, email, client_id, expires) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
		db.execute(insert, (hash(&access), "access", &sub, &email, &f.client_id, (now() + ACCESS_TTL).min(chain_ends))).unwrap();
		db.execute(insert, (hash(&refresh), "refresh", &sub, &email, &f.client_id, chain_ends)).unwrap();
	}
	let body = json!({ "access_token": access, "token_type": "Bearer", "expires_in": ACCESS_TTL.min(chain_ends - now()), "refresh_token": refresh });
	([(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))], Json(body)).into_response()
}

fn oauth_error(error: &str, description: &str) -> Response {
	(StatusCode::BAD_REQUEST, Json(json!({ "error": error, "error_description": description }))).into_response()
}

fn random() -> String {
	let mut bytes = [0u8; 32];
	getrandom::fill(&mut bytes).expect("the OS has entropy");
	URL_SAFE_NO_PAD.encode(bytes)
}

fn hash(secret: &str) -> String {
	URL_SAFE_NO_PAD.encode(Sha256::digest(secret))
}
