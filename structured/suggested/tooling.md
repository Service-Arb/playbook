# phones and intake
- `Quo` ~$15/mo for the number the VA works out of — calls route `CallRail` → `Quo`, and they log in from their own computer to answer and text
  // heard as "Quo"; Eric says it and `OpenPhone` are the same thing
  reason: a filipino VA can't text a US customer off a filipino area code
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=574)
- `Hushed` also works for this — `David`
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=2192)
- `TextVerified` for SMS verifications generally
  > google voice numbers are rejected as VOIP — facebook marketplace now asks for phone verification on some posts
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=6173)
- CRM: google sheets + google calendar. Eric tried `GoHighLevel` and found it made the process more complicated for everyone
  > he has VA templates, a phone script and VA call recordings, and hands them out on request
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=622)

# faking a device
- (!) mock-location apps are visibly spoofed — they all go through the android mock API, so the phone has to be rooted first — `Valera`
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=254)
- root, then `Magisk`, then any GPS spoofer from its store — "GPS Locator", but there are a hundred and they all work
  // heard as "Majestic"
  > rooting is the hard part; youtube it and have chatgpt walk you through
  > if you skip rooting you can still spoof by tethering the phone to a computer — slower, works
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=273)
- `Octobrowser` — free, ~100 profiles, makes a phone look like a different device; runs on iphone or android
  > one member is a week into testing it for reviews, earlier manual attempts held ~60%
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=5767)
- `Octobot` on desktop: load the cookies that ship with a bought aged gmail, then a private residential *mobile* proxy, and the session reads as that person on their phone
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=5945)
- (!) a bad proxy is worse than no proxy at all — `Valera`
  reason: there are many innocent reasons to be on a VPN and exactly one to be on a burnt shared proxy
  > buy residential, from a small seller, and check it hasn't been circulating
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=5887)
- TODO: `Virtual Master` on android — many virtual phones on one handset, one residential IP each, reportedly enough for video verification
  // heard as "Virtual Master" and "PrivacyMaster", unverified, both second-hand from a forum post
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=4180)

# accounts and payments
- (!) `Privacy.com` issues prepaid cards where you set the name, address and zip to anything and it still clears
  > used to buy the cheapest google workspace plan per gmail without every account sharing one payment identity
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=4591)
- `accsmarket` sells facebook accounts with ad manager already enabled // heard as "Accounts Market"
  > `Jordan` says buying business managers is routine and has a dedicated supplier
  > post from a bought account, never your personal one
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=6203)

# automation
- scraping your own reviews: use a headless browser (`Playwright`, `Puppeteer`), not an API — `Valera`
  > `Playwright` alone gets caught on scroll behaviour
  > the goal is a monthly diff of every review ever left, so deletions can be caught and appealed
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=3779)
- `Valera` generates the paper trail — branding, business cards, neighbouring-business filler, the documents a brief needs — from an address and a company name // `ServiceArb` on github
  > "they're not valid, but they look somewhat valid". the alternative people use is buying fake business documents online
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=5300)
- don't run local open-weight models for this — inference sold by the labs is subsidised ~10x below what your own hardware costs — `Valera`
  A: for guardrails, seed the conversation on a weaker model that is already helping, then switch models mid-thread; or frame it as reverse-engineering your own lost repo. ~80% hit rate
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=3932)
- every past coaching call is already transcribed with timestamps — skool homepage → coaching calls → open the loom → transcript pane
  [2026-09-18](https://www.loom.com/share/c197556c940b4d01b53376dab70d32e5?t=4026)
