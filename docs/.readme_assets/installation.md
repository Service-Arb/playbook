The playbook is a remote MCP server at `https://sa.valeratrades.com/playbook_mcp`. Anyone
added to the `service-arb` group on valeratrades.com can connect it to Claude Code on any
machine. Nothing is installed locally, and there is no key to copy between devices: each
device signs in through valeratrades.com in the browser.

```
owner: add their Gmail to the group ──► send them the steps under "For members"
member: claude mcp add … ──► /mcp → Authenticate ──► browser: valeratrades.com, "Continue with Google"
                                                    └─► back in Claude Code, connected
```

## For the owner: giving someone access

1. Add their **Gmail address** to `service-arb` in `~/s/site/flake.nix` (`prodConfig.groups`):

   ```nix
   groups = {
     admin = [ … ];
     service-arb = [ "v79166789533@gmail.com" "them@gmail.com" ];
   };
   ```

2. Commit, push, release the site. Access starts once the new site is serving.
3. Send them the section below, as is. Nothing else needs sharing: no token, no file, no invite.

Removing someone is the same edit in reverse. They lose access within 7 days at most, when their
Claude Code next has to sign in again.

## For members: connecting Claude Code

You need the Google account the owner added. Use **"Continue with Google"** when signing in:
it is how valeratrades.com knows the address is yours. An account made with a password does not
get access.

1. In a terminal:

   ```sh
   claude mcp add --scope user --transport http service-arb https://sa.valeratrades.com/playbook_mcp
   ```

   `--scope user` makes it available in every project on this machine.

2. Start Claude Code, type `/mcp`, pick `service-arb`, choose **Authenticate**.
3. The browser opens valeratrades.com. Sign in with **Continue with Google**, using the added
   account. If you are already signed in there, this step passes by itself.
4. The browser says you can return to Claude Code; `/mcp` now shows `service-arb` connected.

Repeat the same steps on each device.

## When something goes wrong

| what you see | what it means |
|---|---|
| "…is not a service-arb member" | you signed in with an address the owner has not added, or with a password account. Sign out on valeratrades.com, then sign in with Google using the added account |
| `/mcp` shows it needs authentication again | normal: sign-in lasts 7 days. Run step 2 again |
| "…has used today's … bytes" (429) | the daily reading budget is spent; it resets at 00:00 UTC |
| an old `service-arb.valeratrades.com` entry | that address is gone: `claude mcp remove service-arb`, then step 1 |
