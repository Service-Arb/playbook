use std::sync::Arc;

use axum::http::request::Parts;
use rmcp::{
	ErrorData as McpError, RoleServer, ServerHandler,
	handler::server::{router::tool::ToolRouter, wrapper::Parameters},
	model::*,
	schemars,
	service::RequestContext,
	tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

use crate::{
	Member, State,
	corpus::{CORPUS, Found, GUIDE_DESCRIPTION, INSTRUCTIONS},
	now,
};

/// claude.ai reportedly drops server `instructions`, so the rules that matter most ride here too
const SEARCH_DESCRIPTION: &str = "Case-insensitive regex over the service-arb playbook: structured/approved notes, then structured/suggested, then ref/ captures (calls, course lessons, videos, chats), at most two hits per file, captures newest first. Each hit gives path:line, its header, the date and reliability, the source URL and the lines around it. Search the words a speaker would say, alternated: `review(s)? (a|per) day|reviews? daily`. Search `ref/skool_` course lessons first: they outrank any call. A member's failure story is an anecdote, not a rule. Newest wins only between equal sources. When nothing answers, say the corpus does not cover it.";

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
pub struct SearchArgs {
	/// a regex, case-insensitive. Alternate synonyms and likely mis-transcriptions with `|`
	pattern: String,
	/// a path prefix to search under, e.g. `ref/loom` or `structured/approved/profile`
	scope: Option<String>,
}

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
pub struct ReadArgs {
	/// a path as `search` returned it
	path: String,
	/// part of the header line of the chapter or section to read, e.g. `Selling calls` or `## facts`
	header: String,
	/// a line number inside the chapter, when the header text repeats in the file
	line: Option<usize>,
}

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
pub struct GuideArgs {
	section: String,
}

#[derive(Clone)]
pub struct Playbook {
	state: Arc<State>,
	tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Playbook {
	pub fn new(state: Arc<State>) -> Self {
		Self { state, tool_router: Self::tool_router() }
	}

	#[tool(description = SEARCH_DESCRIPTION, annotations(read_only_hint = true, open_world_hint = false))]
	async fn search(&self, Parameters(args): Parameters<SearchArgs>, ctx: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
		Ok(self.answer(&ctx, "search", &args, CORPUS.search(&args.pattern, args.scope.as_deref())))
	}

	#[tool(
		description = "One chapter of a capture (a `###` under `## transcript` or `## summary`) or one section of a note, whole, with every line numbered. Never a whole file.",
		annotations(read_only_hint = true, open_world_hint = false)
	)]
	async fn read(&self, Parameters(args): Parameters<ReadArgs>, ctx: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
		Ok(self.answer(&ctx, "read", &args, CORPUS.read(&args.path, &args.header, args.line)))
	}

	#[tool(description = GUIDE_DESCRIPTION, annotations(read_only_hint = true, open_world_hint = false))]
	async fn guide(&self, Parameters(args): Parameters<GuideArgs>, ctx: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
		Ok(self.answer(&ctx, "guide", &args, CORPUS.guide(&args.section)))
	}

	fn answer(&self, ctx: &RequestContext<RoleServer>, tool: &str, args: &impl Serialize, found: Result<Found, String>) -> CallToolResult {
		let parts = ctx.extensions.get::<Parts>().expect("served over streamable http only");
		let Member { sub, email } = parts.extensions.get::<Member>().expect("the guard admits members only");
		let (text, ids, error) = match found {
			Ok(Found { text, ids }) => (text, ids, false),
			Err(e) => (e, vec![], true),
		};
		self.state
			.db
			.lock()
			.unwrap()
			.execute(
				"INSERT INTO calls (at, sub, email, tool, args, ids, bytes, error) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
				(now(), sub, email, tool, serde_json::to_string(args).unwrap(), ids.join(" "), text.len() as i64, error),
			)
			.expect("the calls table is created on start");
		match error {
			false => CallToolResult::success(vec![ContentBlock::text(text)]),
			true => CallToolResult::error(vec![ContentBlock::text(text)]),
		}
	}
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Playbook {
	fn get_info(&self) -> ServerConfig {
		ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
			.with_server_info(Implementation::from_build_env())
			.with_protocol_version(ProtocolVersion::V_2026_07_28)
			.with_instructions(INSTRUCTIONS)
	}
}
