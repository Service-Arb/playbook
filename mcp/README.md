# mcp

The playbook for members: a remote MCP server that answers from `skill/service-arb/`, `structured/`
and `ref/**/*.md`, in slices, to the Google accounts on `members.toml`. `build.rs` bakes that text
into the binary, so the server reads nothing from disk per request and nothing else of the repo
reaches the image. `ref/cases/` states no source and no date, so it stays out.

```
claude mcp add --transport http service-arb https://<host>/mcp
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
client ──/register (DCR) or a CIMD url──► /authorize ──► Google ──► /callback
                                                                    │ email on members.toml? else 403
client ◄── code ◄───────────────────────────────────────────────────┘
client ──/token (PKCE S256)──► access token (1h) + refresh token (30d, rotated on use)
client ──/mcp, Bearer──► guard: token live · still a member · under today's byte budget (else 429)
```

Tokens are opaque; only their sha256 is stored. A member removed from `members.toml` is refused from
the next deploy on.

## Configuration

| env | |
|---|---|
| `PORT` | set by the image |
| `PUBLIC_URL` | where members reach it, e.g. `https://mcp.<domain>`; issuer, resource, and Google's redirect `<PUBLIC_URL>/callback` |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | a Google OAuth web client |

Missing any of them fails the start. The database is `mcp.db` in the working directory, `/data` in
the image.

## Reading the log

```sh
kubectl -n service-arb exec deploy/service-arb-mcp -- sqlite3 /data/mcp.db \
  "SELECT datetime(at, 'unixepoch'), email, tool, args, ids, bytes FROM calls ORDER BY at DESC LIMIT 50"
```

`calls`: `at` (unix seconds), `email`, `tool`, `args` (json), `ids` (the `path:line` of each hit
returned, or the section), `bytes` (what the member got back), `error`. The full schema is
`src/schema.sql`.
