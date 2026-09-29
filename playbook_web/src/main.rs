const DAILY_BYTES: i64 = 2_000_000; // ~500k tokens a member a day

#[tokio::main]
async fn main() {
	let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is unset"));
	let config = playbook_web::Config {
		public_url: env("PUBLIC_URL").trim_end_matches('/').to_owned(),
		sso: va_sso::Verifier::try_new(&env("SSO_PUBLIC_KEY")).expect("SSO_PUBLIC_KEY is an Ed25519 public key PEM"),
		sso_refresh_url: env("SSO_REFRESH_URL"),
		db: "mcp.db".into(),
		daily_bytes: DAILY_BYTES,
	};
	let addr = format!("0.0.0.0:{}", env("PORT"));
	let listener = tokio::net::TcpListener::bind(&addr).await.unwrap_or_else(|e| panic!("{addr}: {e}"));
	println!("serving on {addr}");
	axum::serve(listener, playbook_web::app(config)).with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() }).await.unwrap();
}
