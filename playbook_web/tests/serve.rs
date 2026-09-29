//! Drives the server over streamable HTTP with a member's token, the way a member's client does.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rmcp::{
	RoleClient, ServiceExt,
	model::CallToolRequestParams,
	service::RunningService,
	transport::{StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const MEMBER: &str = "member@example.com";
const TOKEN: &str = "test-token";
const BASE: &str = "/playbook_mcp";
const REFRESH: &str = "https://site.test/auth/refresh";
const SSO_PRIVATE: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIA3bBKSXvm87i5bc706Y1QG1uj5EmbgUZygHJGfO1XYj\n-----END PRIVATE KEY-----\n";
const SSO_PUBLIC: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAws8sYuYGZt4/OjCm05rzUQYOTAWBxVHPL1Fdg74KyV4=\n-----END PUBLIC KEY-----\n";

#[derive(Deserialize)]
struct Golden {
	questions: Vec<Question>,
}

#[derive(Deserialize)]
struct Question {
	id: String,
	answerable: bool,
	essential: Vec<Evidence>,
	calls: Vec<Call>,
}

#[derive(Deserialize)]
struct Evidence {
	path: String,
	quote: String,
}

#[derive(Deserialize)]
struct Call {
	tool: String,
	args: Value,
}

/// Every answerable question's essential quotes come back within three calls; an unanswerable one
/// finds nothing.
#[tokio::test]
async fn golden() {
	let golden: Golden = serde_json::from_str(include_str!("golden.json")).unwrap();
	serve("golden", i64::MAX, async |url| {
		let client = connect(&url, TOKEN).await;
		let mut failures = Vec::new();
		for q in &golden.questions {
			assert!(q.calls.len() <= 3, "{}: at most three calls", q.id);
			let mut seen = String::new();
			for c in &q.calls {
				let (text, error) = call(&client, &c.tool, c.args.clone()).await;
				assert!(!error, "{} {}: {text}", q.id, c.tool);
				seen.push_str(&text);
			}
			let seen = squash(&seen);
			match q.answerable {
				true => failures.extend(q.essential.iter().filter(|e| !seen.contains(&squash(&e.quote))).map(|e| format!("{}: {} — {}", q.id, e.path, e.quote))),
				false if !seen.starts_with("no match") => failures.push(format!("{}: expected no match, got {seen}", q.id)),
				false => {}
			}
		}
		assert!(failures.is_empty(), "missing evidence:\n{}", failures.join("\n"));
	})
	.await;
}

#[tokio::test]
async fn read_returns_one_chapter() {
	serve("read", i64::MAX, async |url| {
		let client = connect(&url, TOKEN).await;
		for (path, header) in [("ref/loom/2026-09-18-group-call.md", "[03:34]"), ("structured/suggested/infra.md", "## facts")] {
			let (text, error) = call(&client, "read", json!({ "path": path, "header": header })).await;
			assert!(!error, "{text}");
			let headers = text.lines().filter(|l| l.split_once(" | ").is_some_and(|(_, l)| l.starts_with('#'))).count();
			assert_eq!(headers, 1, "{text}");
		}
	})
	.await;
}

/// The call that crosses the budget is answered; the request after it is refused until midnight UTC.
#[tokio::test]
async fn quota_overrun_is_429() {
	serve("quota", 1, async |url| {
		let client = connect(&url, TOKEN).await;
		call(&client, "guide", json!({ "section": "reviews" })).await;
		let res = reqwest::Client::new()
			.post(format!("{url}{BASE}"))
			.bearer_auth(TOKEN)
			.header("accept", "application/json, text/event-stream")
			.json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }))
			.send()
			.await
			.unwrap();
		assert_eq!(res.status(), 429);
		let retry: i64 = res.headers()["retry-after"].to_str().unwrap().parse().unwrap();
		assert!((1..=86_400).contains(&retry), "{retry}");
	})
	.await;
}

/// A client with nothing but the MCP url finds the authorization server through the path-inserted
/// metadata of RFC 9728 and RFC 8414.
#[tokio::test]
async fn no_token_leads_to_the_authorization_server() {
	serve("unauthorized", i64::MAX, async |url| {
		let http = reqwest::Client::new();
		let res = http.post(format!("{url}{BASE}")).json(&json!({})).send().await.unwrap();
		assert_eq!(res.status(), 401);
		let challenge = res.headers()["www-authenticate"].to_str().unwrap();
		let metadata = format!("{url}/.well-known/oauth-protected-resource{BASE}");
		assert_eq!(challenge, format!("Bearer resource_metadata=\"{metadata}\""));
		let resource: Value = http.get(metadata).send().await.unwrap().json().await.unwrap();
		assert_eq!(resource["resource"], format!("{url}{BASE}"));
		let issuer = resource["authorization_servers"][0].as_str().unwrap();
		let server: Value = http.get(format!("{url}/.well-known/oauth-authorization-server{BASE}")).send().await.unwrap().json().await.unwrap();
		assert_eq!(server["issuer"], issuer);
		assert_eq!(server["token_endpoint"], format!("{url}{BASE}/token"));
	})
	.await;
}

/// A browser signed in on the site as a member is handed its code at once; one that is not
/// signed in goes to the site and comes back to this very request; someone outside the group
/// is refused. A native client's loopback redirect is registered without a port and matches
/// whichever it listens on (RFC 8252 §7.3), as Claude Code's client metadata does.
#[tokio::test]
async fn authorize_follows_the_sites_sign_in() {
	serve("authorize", i64::MAX, async |url| {
		let http = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
		let reg: Value = http
			.post(format!("{url}{BASE}/register"))
			.json(&json!({ "redirect_uris": ["http://localhost/callback"] }))
			.send()
			.await
			.unwrap()
			.json()
			.await
			.unwrap();
		let verifier = "a-verifier-long-enough-to-be-one-0123456789";
		let authorize = reqwest::Url::parse_with_params(&format!("{url}{BASE}/authorize"), [
			("response_type", "code"),
			("client_id", reg["client_id"].as_str().unwrap()),
			("redirect_uri", "http://localhost:64461/callback"),
			("code_challenge", &URL_SAFE_NO_PAD.encode(Sha256::digest(verifier))),
			("code_challenge_method", "S256"),
			("state", "s1"),
		])
		.unwrap();
		let cookie = |groups: &[&str]| {
			let claims = va_sso::Claims {
				sub: "u1".into(),
				email: MEMBER.into(),
				username: "m".into(),
				admin: false,
				groups: groups.iter().map(|g| (*g).to_owned()).collect(),
				exp: i64::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()).unwrap() + 900,
			};
			format!("{}={}", va_sso::COOKIE, va_sso::mint(SSO_PRIVATE, claims).unwrap())
		};

		let res = http.get(authorize.clone()).send().await.unwrap();
		assert!(res.status().is_redirection(), "{}", res.status());
		let to = reqwest::Url::parse(res.headers()["location"].to_str().unwrap()).unwrap();
		assert_eq!(to.as_str().split('?').next(), Some(REFRESH));
		assert_eq!(to.query_pairs().find(|(k, _)| k == "return_to").unwrap().1, authorize.as_str());

		let res = http.get(authorize.clone()).header("cookie", cookie(&["other"])).send().await.unwrap();
		assert_eq!(res.status(), 403);

		let res = http.get(authorize.clone()).header("cookie", cookie(&["service-arb"])).send().await.unwrap();
		let back = reqwest::Url::parse(res.headers()["location"].to_str().unwrap()).unwrap();
		assert_eq!(back.as_str().split('?').next(), Some("http://localhost:64461/callback"));
		assert_eq!(back.query_pairs().find(|(k, _)| k == "state").unwrap().1, "s1");
		let code = back.query_pairs().find(|(k, _)| k == "code").unwrap().1.into_owned();
		let tokens: Value = http
			.post(format!("{url}{BASE}/token"))
			.form(&[
				("grant_type", "authorization_code"),
				("client_id", reg["client_id"].as_str().unwrap()),
				("code", &code),
				("code_verifier", verifier),
				("redirect_uri", "http://localhost:64461/callback"),
			])
			.send()
			.await
			.unwrap()
			.json()
			.await
			.unwrap();
		let client = connect(&url, tokens["access_token"].as_str().unwrap()).await;
		let (text, error) = call(&client, "guide", json!({ "section": "reviews" })).await;
		assert!(!error, "{text}");
	})
	.await;
}

/// Runs `body` against a fresh server, served under `BASE`, that knows one member, holding the access
/// token `TOKEN`. `body` gets the origin.
async fn serve(name: &str, daily_bytes: i64, body: impl AsyncFnOnce(String) -> ()) {
	let dir = std::env::temp_dir().join(format!("playbook_web-test-{}", std::process::id()));
	std::fs::create_dir_all(&dir).unwrap();
	let db = dir.join(format!("{name}.db"));
	if db.exists() {
		std::fs::remove_file(&db).unwrap();
	}
	let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
	let url = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
	let app = playbook_web::app(playbook_web::Config {
		public_url: format!("{url}{BASE}"),
		sso: va_sso::Verifier::try_new(SSO_PUBLIC).unwrap(),
		sso_refresh_url: REFRESH.into(),
		db: db.clone(),
		daily_bytes,
	});
	rusqlite::Connection::open(&db)
		.unwrap()
		.execute(
			"INSERT INTO tokens (hash, kind, email, client_id, expires) VALUES (?1, 'access', ?2, 'test', ?3)",
			(URL_SAFE_NO_PAD.encode(Sha256::digest(TOKEN)), MEMBER, i64::MAX),
		)
		.unwrap();
	tokio::select! {
		r = async { axum::serve(listener, app).await } => panic!("the server stopped: {r:?}"),
		() = body(url) => {}
	}
}

async fn connect(url: &str, token: &str) -> RunningService<RoleClient, ()> {
	let transport = StreamableHttpClientTransport::from_config(StreamableHttpClientTransportConfig::with_uri(format!("{url}{BASE}")).auth_header(token));
	().serve(transport).await.unwrap()
}

async fn call(client: &RunningService<RoleClient, ()>, tool: &str, args: Value) -> (String, bool) {
	let res = client
		.call_tool(CallToolRequestParams::new(tool.to_owned()).with_arguments(args.as_object().unwrap().clone()))
		.await
		.unwrap();
	let text = res.content.iter().map(|c| c.as_text().unwrap().text.as_str()).collect::<Vec<_>>().join("\n");
	(text, res.is_error == Some(true))
}

/// the text as a reader sees it: without the line-number gutter, whitespace collapsed
fn squash(s: &str) -> String {
	let text = s.lines().map(|l| l.split_once(" | ").filter(|(n, _)| n.trim().parse::<usize>().is_ok()).map_or(l, |(_, t)| t));
	text.flat_map(str::split_whitespace).collect::<Vec<_>>().join(" ").to_lowercase()
}
