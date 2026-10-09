# playbook
<br>
[<img alt="ci errors" src="https://img.shields.io/github/actions/workflow/status/Service-Arb/playbook/errors.yml?branch=main&style=for-the-badge&style=flat-square&label=errors&labelColor=420d09" height="20">](https://github.com/Service-Arb/playbook/actions?query=branch%3Amain) <!--NB: Won't find it if repo is private-->
[<img alt="ci warnings" src="https://img.shields.io/github/actions/workflow/status/Service-Arb/playbook/warnings.yml?branch=main&style=for-the-badge&style=flat-square&label=warnings&labelColor=d16002" height="20">](https://github.com/Service-Arb/playbook/actions?query=branch%3Amain) <!--NB: Won't find it if repo is private-->

A knowledge base on service arbitrage — running Google Business Profiles into subcontracted trade work — where every claim traces back to a dated source. Members read it from Claude Code through an MCP server; see Installation.
<!-- markdownlint-disable -->
<details>
<summary>
<h2>Installation</h2>
</summary>

The playbook is a remote MCP server at `https://sa.evinvest.ltd/playbook_mcp`, behind the
Service-Arb panel. Any member granted `sa:playbook:mcp:use` can connect it to Claude Code on any
machine. Nothing is installed locally, and there is no key to copy between devices: each device
signs in through the panel in the browser.

```
owner: grant sa:playbook:mcp:use ──► send them the steps under "For members"
member: claude mcp add … ──► /mcp → Authenticate ──► browser: sa.evinvest.ltd sign-in, then "Allow"
                                                    └─► back in Claude Code, connected
```

### For the owner: giving someone access

1. In the banking cabinet's **Access** tab, grant the member `sa:playbook:mcp:use` (`sa:admin`
   holds it already). No release, and no site config.
2. Send them the section below, as is. Nothing else needs sharing: no token, no file, no invite.

Revoking the permission stops new sign-ins at once, but a client already connected keeps working
for up to 7 days: refreshing its token does not re-check the permission, so it bites when their
Claude Code next has to sign in again.

### For members: connecting Claude Code

1. In a terminal:

   ```sh
   claude mcp add --scope user --transport http service-arb https://sa.evinvest.ltd/playbook_mcp
   ```

   `--scope user` makes it available in every project on this machine.

2. Start Claude Code, type `/mcp`, pick `service-arb`, choose **Authenticate**.
3. The browser opens `sa.evinvest.ltd`. Sign in with Google if asked (any account; the first sign-in
   creates it), then press **Allow** on the page naming Claude Code. Without access yet, the page
   says so: send the owner the email it shows, then repeat this step.
4. The browser says you can return to Claude Code; `/mcp` now shows `service-arb` connected.

Repeat the same steps on each device.

### When something goes wrong

| what you see | what it means |
|---|---|
| "…has no access to the playbook" (403) | you are signed in, but not granted `sa:playbook:mcp:use`. Ask the owner, then step 2 again |
| "this consent form is spent, expired, or someone else's" | the page sat open over 10 minutes, or was submitted twice. Run step 2 again |
| `/mcp` shows it needs authentication again | normal: sign-in lasts 7 days. Run step 2 again |
| "…has used today's … bytes" (429) | the daily reading budget is spent; it resets at 00:00 UTC |
| an old `sa.valeratrades.com` or `service-arb.valeratrades.com` entry | that address is gone: `claude mcp remove service-arb`, then step 1 |

</details>
<!-- markdownlint-restore -->

## Usage
Ask Claude Code about service arbitrage as usual; with `service-arb` connected it answers from the playbook on its own, citing the dated source of each claim:

| tool | returns |
|---|---|
| `search(pattern, scope?)` | matching lines, approved notes first, then suggested, then raw captures newest first |
| `read(path, header, line?)` | one chapter of a capture or one section of a note |
| `guide(section)` | how to decide in one area (`reviews`, `verification`, …) |



<br>

<sup>
	This repository follows <a href="https://github.com/valeratrades/.github/tree/master/best_practices">my best practices</a> and <a href="https://github.com/tigerbeetle/tigerbeetle/blob/main/docs/TIGER_STYLE.md">Tiger Style</a> (except "proper capitalization for acronyms": (VsrState, not VSRState) and formatting). For project's architecture, see <a href="./docs/ARCHITECTURE.md">ARCHITECTURE.md</a>.
</sup>

#### License

<sup>
	Licensed under <a href="LICENSE">Blue Oak 1.0.0</a>
</sup>

<br>

<sub>
	Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be licensed as above, without any additional terms or conditions.
</sub>

