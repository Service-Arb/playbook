-- clients that registered through DCR; CIMD clients are fetched on every /authorize instead
CREATE TABLE IF NOT EXISTS clients (
	id TEXT PRIMARY KEY,
	redirect_uris TEXT NOT NULL, -- json array
	created INTEGER NOT NULL
);
DROP TABLE IF EXISTS logins; -- Google's /callback, gone
CREATE TABLE IF NOT EXISTS codes (
	hash TEXT PRIMARY KEY,
	client_id TEXT NOT NULL,
	redirect_uri TEXT NOT NULL,
	challenge TEXT NOT NULL,
	email TEXT NOT NULL,
	expires INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS tokens (
	hash TEXT PRIMARY KEY, -- sha256 of the token; the token itself is never stored
	kind TEXT NOT NULL CHECK (kind IN ('access', 'refresh')),
	email TEXT NOT NULL,
	client_id TEXT NOT NULL,
	expires INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS calls (
	at INTEGER NOT NULL,
	email TEXT NOT NULL,
	tool TEXT NOT NULL,
	args TEXT NOT NULL, -- json
	ids TEXT NOT NULL, -- space-separated `path:line`, or the guide section
	bytes INTEGER NOT NULL,
	error INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS calls_by_email ON calls (email, at);
