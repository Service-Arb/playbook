//! What the browser sees of the connect flow, on the kit's class tables, so it reads as the panel
//! does. The design is Figma's "Connect flow"; screens 2 and 3 are the panel's `/access`.

use axum::{
	http::{StatusCode, header},
	response::{Html, IntoResponse, Response},
};
use ev_lib_classes::{AVATAR, AVATAR_FALLBACK, BUTTON_BASE, ButtonVariant, CARD, ITEM_BASE, ItemSize, ItemVariant, Size, button_size_class};
use tailwind_fuse::AsTailwindClass;

include!(concat!(env!("OUT_DIR"), "/stylesheet.rs"));

pub(crate) async fn stylesheet() -> Response {
	([(header::CONTENT_TYPE, "text/css"), (header::CACHE_CONTROL, "public, max-age=31536000, immutable")], CSS).into_response()
}

pub(crate) struct Consent<'a> {
	pub base: &'a str,
	pub client: &'a str,
	pub logo: Option<&'a str>,
	pub name: &'a str,
	pub email: &'a str,
	pub switch: &'a str,
	pub cancel: &'a str,
	pub nonce: &'a str,
	pub redirect: &'a reqwest::Url,
}

pub(crate) fn consent(c: Consent) -> Response {
	let client = escape(c.client);
	let r = c.redirect;
	let authority = match r.port() {
		Some(p) => format!("{}:{p}", r.host_str().expect("https or loopback http")),
		None => r.host_str().expect("https or loopback http").to_owned(),
	};
	let loopback = r.scheme() == "http"; // `redirect_allowed` admits http on loopback only
	let note = match loopback {
		true => r#"<p class="text-xs">That’s an app on this computer. Allow only if you just ran <code>/mcp</code> → Authenticate in Claude Code.</p>"#,
		false => "",
	};
	let body = format!(
		r#"{marks}
<div class="flex w-full flex-col items-center gap-1.5 text-center">
	<h1 class="text-xl font-semibold">{client} wants to access the Service-Arb playbook</h1>
	<p class="text-ink-soft text-[13px]">{site}</p>
</div>
<div class="{account} w-full bg-secondary">
	<span class="{AVATAR}"><span class="{AVATAR_FALLBACK} text-[13px] font-semibold">{initial}</span></span>
	<div class="flex min-w-0 flex-1 flex-col">
		<span class="text-ink-soft text-xs">Signed in as</span>
		<span class="truncate font-medium">{email}</span>
	</div>
	<a class="text-primary-ink text-[13px] font-medium hover:underline" href="{switch}">Switch account</a>
</div>
<div class="flex w-full flex-col gap-3">
	<p class="text-ink-mid text-[13px] font-medium">This will allow {client} to:</p>
	{search}
	{budget}
	<p class="text-ink-soft text-[13px]">{client} cannot change anything in the playbook or the panel.</p>
</div>
<hr class="w-full border-border">
<div class="text-ink-soft flex w-full flex-col gap-1 rounded-lg bg-secondary px-3 py-2.5">
	<p class="text-xs">You’ll be returned to</p>
	<p class="text-[13px] break-all">{scheme}://<b class="text-ink font-medium">{authority}</b>{path}</p>
	{note}
</div>
<form class="grid w-full grid-cols-2 gap-3" method="post" action="{action}">
	<input type="hidden" name="nonce" value="{nonce}">
	<a class="{outline}" href="{cancel}">Cancel</a>
	<button class="{primary}">Allow access</button>
</form>"#,
		marks = marks(Some((c.client, c.logo))),
		site = escape(reqwest::Url::parse(c.base).expect("PUBLIC_URL is a url").host_str().expect("PUBLIC_URL has a host")),
		account = format!("{ITEM_BASE} {} {}", ItemVariant::Outline.as_class(), ItemSize::Sm.as_class()),
		initial = escape(&initial(if c.name.is_empty() { c.email } else { c.name })),
		email = escape(c.email),
		switch = escape(c.switch),
		search = permission("⌕", "Search and read the playbook", "Guides, call notes and reference material, as you."),
		budget = permission("◷", "Use your daily allowance", "Every lookup is logged under your account and counts toward your daily budget."),
		scheme = r.scheme(),
		authority = escape(&authority),
		path = escape(r.path()),
		action = escape(&format!("{}/authorize", c.base)),
		nonce = escape(c.nonce),
		cancel = escape(c.cancel),
		outline = button(ButtonVariant::Outline),
		primary = button(ButtonVariant::Primary),
	);
	page(StatusCode::OK, &format!("Connect {client}"), c.base, &body)
}

/// What went wrong, as the member can act on it; the reason under it is for whoever they ask for help.
pub(crate) enum Trouble {
	Expired,
	BadLink,
	Unverified,
	NoAccess,
}

pub(crate) fn trouble(status: StatusCode, trouble: Trouble, base: &str, reason: &str) -> Response {
	let (headline, sub) = match trouble {
		Trouble::Expired => ("This sign-in link has expired", "Links from Claude Code last 10 minutes and work once."),
		Trouble::BadLink => ("This sign-in link doesn’t work", "It isn’t one Claude Code would open, or it was changed on the way."),
		Trouble::Unverified => ("We couldn’t tell who you are", "Sign-in reaches the playbook through sa.evinvest.ltd only."),
		Trouble::NoAccess => ("No access to the playbook", "Your account no longer holds it. Connecting again shows how to ask."),
	};
	let body = format!(
		r#"{marks}
<div class="flex w-full flex-col items-center gap-1.5 text-center">
	<h1 class="text-xl font-semibold">{headline}</h1>
	<p class="text-ink-soft text-[13px]">{sub}</p>
</div>
<ol class="flex w-full flex-col gap-3.5">
	{back}
	{again}
</ol>
<p class="text-ink-soft text-center text-xs">Details: {reason}</p>"#,
		marks = marks(None),
		back = step("1", "Go back to Claude Code", "Type <code>/mcp</code>, pick service-arb."),
		again = step("2", "Choose Authenticate", "A fresh page opens here."),
		reason = escape(reason),
	);
	page(status, headline, base, &body)
}

fn page(status: StatusCode, title: &str, base: &str, body: &str) -> Response {
	let html = format!(
		r#"<!doctype html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Service-Arb</title>
<link rel="stylesheet" href="{base}/assets/{CSS_FILE}">
<body class="bg-background text-ink font-sans antialiased">
<main class="flex min-h-dvh flex-col items-center justify-center gap-6 p-4 text-sm">
<div class="{CARD} w-[440px] max-w-full items-center px-8 pt-8 pb-7">
{body}
</div>
<p class="text-ink-soft text-xs">Service-Arb · EV Invest</p>
</main>
</body>
</html>
"#,
		base = escape(base),
	);
	(status, [(header::CONTENT_SECURITY_POLICY, "frame-ancestors 'none'")], Html(html)).into_response()
}

/// The client's mark beside Service-Arb's; Service-Arb's alone where the client is not known.
fn marks(client: Option<(&str, Option<&str>)>) -> String {
	let sa = r#"<span class="bg-brand flex size-12 items-center justify-center rounded-xl border border-border font-serif text-[22px] font-semibold">SA</span>"#;
	let Some((name, logo)) = client else { return format!(r#"<div class="flex items-center">{sa}</div>"#) };
	let mark = match logo {
		Some(src) => format!(r#"<img class="size-12 rounded-xl object-cover" src="{}" alt="">"#, escape(src)),
		None => format!(r#"<span class="bg-muted flex size-12 items-center justify-center rounded-xl text-[22px] font-semibold">{}</span>"#, escape(&initial(name))),
	};
	format!(r#"<div class="flex items-center gap-3">{mark}<span class="text-ink-soft text-xs" aria-hidden="true">• • •</span>{sa}</div>"#)
}

fn permission(icon: &str, title: &str, detail: &str) -> String {
	format!(
		r#"<div class="flex w-full items-start gap-3"><span class="bg-popover text-primary-ink flex size-7 shrink-0 items-center justify-center rounded-lg" aria-hidden="true">{icon}</span><div class="flex flex-col gap-0.5"><span class="font-medium">{title}</span><span class="text-ink-soft text-[13px]">{detail}</span></div></div>"#
	)
}

fn step(n: &str, title: &str, detail: &str) -> String {
	format!(
		r#"<li class="flex w-full items-start gap-3"><span class="bg-popover text-primary-ink flex size-6 shrink-0 items-center justify-center rounded-full text-xs font-semibold">{n}</span><div class="flex flex-col gap-0.5"><span class="font-medium">{title}</span><span class="text-ink-soft text-[13px]">{detail}</span></div></li>"#
	)
}

fn button(variant: ButtonVariant) -> String {
	format!("{BUTTON_BASE} {} {} w-full", variant.as_class(), button_size_class(Size::Lg, false))
}

fn initial(s: &str) -> String {
	s.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into()) // a name of punctuation only still gets a mark
}

fn escape(s: &str) -> String {
	s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}
