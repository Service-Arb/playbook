# playbook_web

The playbook for members: a remote MCP server that answers from `skill/service-arb/`, `structured/`
and `ref/**/*.md`, in slices, to the `service-arb` members of valeratrades.com. `build.rs` bakes that text
into the binary, so the server reads nothing from disk per request and nothing else of the repo
reaches the image. `ref/cases/` states no source and no date, so it stays out.

```
claude mcp add --transport http service-arb https://sa.valeratrades.com/playbook_mcp
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

```
client ──/register (DCR) or a CIMD url──► /authorize
   browser's va_access cookie (valeratrades.com's sign-in, `va_sso`):
     none or expired ──► valeratrades.com/auth/refresh?return_to=<this /authorize> ──► back here
     not in service-arb (nor admin) ──► 403
     a member ──► code, at once
client ──/token (PKCE S256)──► access token (1h) + refresh token (rotated on use, 7d from /authorize)
client ──<base>, Bearer──► guard: token live · under today's byte budget (else 429)
```

Membership is decided at `/authorize`, from the cookie. Rotating a refresh token keeps the
deadline its `/authorize` set, so someone removed from the group on valeratrades.com is out
within 7 days, when their client has to authorize again. Tokens are opaque; only their
sha256 is stored.

## Configuration

| env | |
|---|---|
| `PORT` | set by the image |
| `PUBLIC_URL` | where members reach it, `https://sa.valeratrades.com/playbook_mcp`: the MCP endpoint, the issuer and the resource. Its path is `<base>`: `/authorize`, `/token` and `/register` sit under it, the metadata at `/.well-known/oauth-{protected-resource,authorization-server}<base>` (RFC 9728, 8414), `/health` at the root |
| `SSO_PUBLIC_KEY` | valeratrades.com's Ed25519 public key (PEM), for the `va_access` cookie |
| `SSO_REFRESH_URL` | `https://valeratrades.com/auth/refresh` |

Missing any fails the start. The database is `mcp.db` in the working directory, `/data` in
the image.

## Reading the log

```sh
kubectl -n personal exec deploy/playbook-web -- sqlite3 /data/mcp.db \
  "SELECT datetime(at, 'unixepoch'), email, tool, args, ids, bytes FROM calls ORDER BY at DESC LIMIT 50"
```

`calls`: `at` (unix seconds), `email`, `tool`, `args` (json), `ids` (the `path:line` of each hit
returned, or the section), `bytes` (what the member got back), `error`. The full schema is
`src/schema.sql`.
