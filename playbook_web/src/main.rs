const DAILY_BYTES: i64 = 2_000_000; // ~500k tokens a member a day

#[tokio::main]
async fn main() {
	let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("{k} is unset"));
	let config = playbook_web::Config {
		public_url: env("PUBLIC_URL").trim_end_matches('/').to_owned(),
		panel_keys: env("PANEL_ASSERTION_KEYS").parse().unwrap_or_else(|e| panic!("PANEL_ASSERTION_KEYS: {e}")),
		db: "mcp.db".into(),
		daily_bytes: DAILY_BYTES,
	};
	let addr = format!("0.0.0.0:{}", env("PORT"));
	let listener = tokio::net::TcpListener::bind(&addr).await.unwrap_or_else(|e| panic!("{addr}: {e}"));
	println!("serving on {addr}");
	axum::serve(listener, playbook_web::app(config)).with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() }).await.unwrap();
}
