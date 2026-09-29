//! OAuth 2.1 authorization server for the MCP clients. Who the member is comes from
//! valeratrades.com's `va_access` cookie ([`va_sso`]). The flow and the tables are in `README.md`.

use std::sync::Arc;

use axum::{
	Form, Json, Router,
	extract::{Query, RawQuery, Request, State as Axum},
	http::{HeaderMap, HeaderValue, StatusCode, header},
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
const REFRESH_TTL: i64 = 7 * 24 * 60 * 60; // from authorize: rotation does not extend it, so membership is re-checked weekly
const GROUP: &str = "service-arb";
const DAY: i64 = 24 * 60 * 60;
const CIMD_MAX_BYTES: usize = 64 * 1024;

/// `base` is `PUBLIC_URL`'s path; the metadata sits where RFC 8414 and 9728 insert it
pub(crate) fn routes(base: &str) -> Router<Arc<State>> {
	Router::new()
		.route(&format!("/.well-known/oauth-protected-resource{base}"), get(resource_metadata))
		.route(&format!("/.well-known/oauth-authorization-server{base}"), get(server_metadata))
		.route(&format!("{base}/register"), post(register))
		.route(&format!("{base}/authorize"), get(authorize))
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
	let email: Option<String> = state
		.db
		.lock()
		.unwrap()
		.query_row("SELECT email FROM tokens WHERE hash = ?1 AND kind = 'access' AND expires > ?2", (hash(token), now()), |r| r.get(0))
		.optional()
		.unwrap();
	let Some(email) = email else { return unauthorized() };
	let today = now() - now() % DAY;
	let spent: i64 = state
		.db
		.lock()
		.unwrap()
		.query_row("SELECT COALESCE(SUM(bytes), 0) FROM calls WHERE email = ?1 AND at >= ?2", (&email, today), |r| r.get(0))
		.unwrap();
	if spent >= state.config.daily_bytes {
		let retry = (today + DAY - now()).to_string();
		return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, retry)], format!("{email} has used today's {} bytes; the budget resets at 00:00 UTC", state.config.daily_bytes)).into_response();
	}
	req.extensions_mut().insert(Member(email));
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
		.execute("INSERT INTO clients (id, redirect_uris, created) VALUES (?1, ?2, ?3)", (&id, serde_json::to_string(&reg.redirect_uris).unwrap(), now()))
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

/// A browser signed in on valeratrades.com as a member gets its code at once; one that is not
/// goes to the site's `/auth/refresh`, which signs it in and sends it back here.
async fn authorize(Axum(state): S, headers: HeaderMap, RawQuery(raw): RawQuery, Query(q): Query<AuthorizeQuery>) -> Response {
	if q.response_type != "code" || q.code_challenge_method != "S256" {
		return (StatusCode::BAD_REQUEST, "response_type=code with code_challenge_method=S256 only").into_response();
	}
	if let Some(resource) = &q.resource
		&& resource.trim_end_matches('/') != state.config.public_url
	{
		return (StatusCode::BAD_REQUEST, format!("{resource} is not served here")).into_response();
	}
	if let Err(e) = client_redirects_to(&state, &q.client_id, &q.redirect_uri).await {
		return (StatusCode::BAD_REQUEST, e).into_response();
	}
	let cookie = headers
		.get_all(header::COOKIE)
		.iter()
		.filter_map(|v| v.to_str().ok())
		.flat_map(|v| v.split(';'))
		.find_map(|c| c.trim().strip_prefix(va_sso::COOKIE)?.strip_prefix('='));
	let Some(claims) = cookie.and_then(|c| state.config.sso.verify(c).ok()) else {
		let here = format!("{}/authorize?{}", state.config.public_url, raw.expect("parsed into AuthorizeQuery above"));
		let refresh = reqwest::Url::parse_with_params(&state.config.sso_refresh_url, [("return_to", &here)]).expect("SSO_REFRESH_URL is a url");
		return Redirect::to(refresh.as_str()).into_response();
	};
	if !claims.member_of(GROUP) {
		let page = format!("<!doctype html><title>Not a member</title><p><b>{}</b> is not a {GROUP} member. Ask Valera to add it, then connect again.</p>", claims.email);
		return (StatusCode::FORBIDDEN, Html(page)).into_response();
	}
	let code = random();
	state
		.db
		.lock()
		.unwrap()
		.execute(
			"INSERT INTO codes (hash, client_id, redirect_uri, challenge, email, expires) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
			(hash(&code), &q.client_id, &q.redirect_uri, &q.code_challenge, claims.email.to_lowercase(), now() + CODE_TTL),
		)
		.unwrap();
	let mut back = reqwest::Url::parse(&q.redirect_uri).expect("a registered redirect_uri");
	back.query_pairs_mut().append_pair("code", &code);
	if let Some(s) = &q.state {
		back.query_pairs_mut().append_pair("state", s);
	}
	Redirect::to(back.as_str()).into_response()
}

/// A `client_id` is either a CIMD document's https URL, or an id `/register` handed out.
async fn client_redirects_to(state: &State, client_id: &str, redirect_uri: &str) -> Result<(), String> {
	let uris: Vec<String> = if client_id.starts_with("https://") {
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
		serde_json::from_value(doc["redirect_uris"].clone()).map_err(|_| "the client metadata has no redirect_uris")?
	} else {
		let stored: Option<String> =
			state.db.lock().unwrap().query_row("SELECT redirect_uris FROM clients WHERE id = ?1", [client_id], |r| r.get(0)).optional().unwrap();
		serde_json::from_str(&stored.ok_or("unknown client_id; register first")?).expect("written by /register")
	};
	// RFC 8252 §7.3: a loopback redirect is registered without the port the client ends up listening on
	let portless = |u: &str| {
		let mut u = reqwest::Url::parse(u).ok()?;
		(u.scheme() == "http" && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))).then(|| u.set_port(None).expect("http takes a port"))?;
		Some(u)
	};
	match uris.iter().any(|u| u == redirect_uri || portless(u).is_some_and(|u| Some(u) == portless(redirect_uri))) {
		true => Ok(()),
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
	let (email, chain_ends) = match f.grant_type.as_str() {
		"authorization_code" => {
			let (Some(code), Some(verifier), Some(redirect_uri)) = (&f.code, &f.code_verifier, &f.redirect_uri) else {
				return oauth_error("invalid_request", "code, code_verifier and redirect_uri");
			};
			let db = state.db.lock().unwrap();
			let row: Option<(String, String, String, String)> = db
				.query_row("SELECT client_id, redirect_uri, challenge, email FROM codes WHERE hash = ?1 AND expires > ?2", (hash(code), now()), |r| {
					Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
				})
				.optional()
				.unwrap();
			db.execute("DELETE FROM codes WHERE hash = ?1 OR expires <= ?2", (hash(code), now())).unwrap();
			match row {
				Some((client, redirect, challenge, email))
					if client == f.client_id && &redirect == redirect_uri && URL_SAFE_NO_PAD.encode(Sha256::digest(verifier)) == challenge =>
					(email, now() + REFRESH_TTL),
				_ => return oauth_error("invalid_grant", "the code is spent, expired, or not this client's"),
			}
		}
		"refresh_token" => {
			let Some(refresh) = &f.refresh_token else { return oauth_error("invalid_request", "refresh_token") };
			let db = state.db.lock().unwrap();
			let row: Option<(String, i64)> = db
				.query_row(
					"DELETE FROM tokens WHERE hash = ?1 AND kind = 'refresh' AND client_id = ?2 AND expires > ?3 RETURNING email, expires",
					(hash(refresh), &f.client_id, now()),
					|r| Ok((r.get(0)?, r.get(1)?)),
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
		let insert = "INSERT INTO tokens (hash, kind, email, client_id, expires) VALUES (?1, ?2, ?3, ?4, ?5)";
		db.execute(insert, (hash(&access), "access", &email, &f.client_id, (now() + ACCESS_TTL).min(chain_ends))).unwrap();
		db.execute(insert, (hash(&refresh), "refresh", &email, &f.client_id, chain_ends)).unwrap();
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
