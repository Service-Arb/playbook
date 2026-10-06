-- clients that registered through DCR; CIMD clients are fetched on every /authorize instead
CREATE TABLE IF NOT EXISTS clients (
	id TEXT PRIMARY KEY,
	redirect_uris TEXT NOT NULL, -- json array
	created INTEGER NOT NULL,
	name TEXT -- RFC 7591's optional client_name, shown on the consent page
);
-- a consent page's form, spent by its POST
CREATE TABLE IF NOT EXISTS consents (
	hash TEXT PRIMARY KEY, -- sha256 of the nonce
	sub TEXT NOT NULL,
	client_id TEXT NOT NULL,
	redirect_uri TEXT NOT NULL,
	challenge TEXT NOT NULL,
	state TEXT,
	expires INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS codes (
	hash TEXT PRIMARY KEY,
	client_id TEXT NOT NULL,
	redirect_uri TEXT NOT NULL,
	challenge TEXT NOT NULL,
	sub TEXT NOT NULL, -- concierge's user id
	email TEXT NOT NULL, -- for reading the log
	expires INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS tokens (
	hash TEXT PRIMARY KEY, -- sha256 of the token; the token itself is never stored
	kind TEXT NOT NULL CHECK (kind IN ('access', 'refresh')),
	sub TEXT NOT NULL,
	email TEXT NOT NULL,
	client_id TEXT NOT NULL,
	expires INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS calls (
	at INTEGER NOT NULL,
	sub TEXT NOT NULL,
	email TEXT NOT NULL,
	tool TEXT NOT NULL,
	args TEXT NOT NULL, -- json
	ids TEXT NOT NULL, -- space-separated `path:line`, or the guide section
	bytes INTEGER NOT NULL,
	error INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS calls_by_sub ON calls (sub, at);
PRAGMA user_version = 1;
