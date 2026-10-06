# playbook_web

The playbook for members: a remote MCP server that answers from `skill/service-arb/`, `structured/`
and `ref/**/*.md`, in slices, to the Service-Arb members holding `sa:playbook:mcp:use`. `build.rs` bakes that text
into the binary, so the server reads nothing from disk per request and nothing else of the repo
reaches the image. `ref/cases/` states no source and no date, so it stays out.

```
claude mcp add --transport http service-arb https://sa.evinvest.ltd/playbook_mcp
```

## Tools

| tool | returns |
|---|---|
| `search(pattern, scope?)` | case-insensitive regex hits: `structured/approved`, then `suggested`, then `ref/` newest first; two per file, each with path:line, header, date, reliability and source URL. A pattern hitting 40% of the files in scope is refused |
| `read(path, header, line?)` | one `###` chapter of a capture or one `##` section of a note, never a whole file |
| `guide(section)` | one `skill/service-arb/sections/<section>.md` |

The server's `instructions` are `SKILL.md`'s hook and `## Answering`, held to 2,048 characters at
build time. The rules that matter most are repeated in `search`'s description.

## Sign-in

The panel at `sa.evinvest.ltd` forwards `<base>/*` and the two `.well-known/oauth-*<base>` paths
unchanged. On `<base>/authorize` (GET and POST) it adds `x-sa-assertion`, its signed word for who
is asking and what of the playbook they may do (`sa_auth`), bound to that method and path.

```
client ──/register (DCR) or a CIMD url──► GET /authorize
   assertion: missing or refused ──► 401 · without sa:playbook:mcp:use ──► 403
   ok ──► consent page: client name, redirect host, form with a nonce (10 min, single use, bound to sub + this request)
browser ──POST /authorize, nonce──► assertion again, nonce spent for the same sub ──► code ──► redirect_uri
client ──/token (PKCE S256)──► access token (1h) + refresh token (rotated on use, 7d from /authorize)
client ──<base>, Bearer──► guard: token live · under today's byte budget (else 429)
```

A code is only ever issued by the POST. Who the member is (concierge's `sub`) and the permission
are decided at `/authorize`; a refresh does not re-check them, and rotating keeps the deadline its
`/authorize` set, so revoking `sa:playbook:mcp:use` takes up to 7 days to bite. Tokens and nonces
are opaque; only their sha256 is stored.

## Configuration

| env | |
|---|---|
| `PORT` | set by the image |
| `PUBLIC_URL` | where members reach it, `https://sa.evinvest.ltd/playbook_mcp`: the MCP endpoint, the issuer and the resource. Its path is `<base>`: `/authorize`, `/token` and `/register` sit under it, the metadata at `/.well-known/oauth-{protected-resource,authorization-server}<base>` (RFC 9728, 8414), `/health` at the root |
| `PANEL_ASSERTION_KEYS` | the panel's assertion keys, `<kid>:<base64 Ed25519 public key>`, comma-separated while one rotates |

Missing any fails the start. The database is `mcp.db` in the working directory, `/data` in
the image.

## Reading the log

```sh
kubectl -n personal exec deploy/playbook-web -- sqlite3 /data/mcp.db \
  "SELECT datetime(at, 'unixepoch'), email, tool, args, ids, bytes FROM calls ORDER BY at DESC LIMIT 50"
```

`calls`: `at` (unix seconds), `sub` (concierge's user id), `email`, `tool`, `args` (json), `ids` (the `path:line` of each hit
returned, or the section), `bytes` (what the member got back), `error`. The full schema is
`src/schema.sql`; `calls_va_sso` is the log from before concierge, by email.
