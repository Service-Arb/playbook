-- va_sso named a member by email; concierge's sub does now. Codes and tokens are minted again at the next sign-in.
DROP TABLE IF EXISTS logins;
DROP TABLE codes;
DROP TABLE tokens;
DROP INDEX calls_by_email;
ALTER TABLE calls RENAME TO calls_va_sso; -- the log before concierge, by email; nothing reads it
ALTER TABLE clients ADD COLUMN name TEXT;
