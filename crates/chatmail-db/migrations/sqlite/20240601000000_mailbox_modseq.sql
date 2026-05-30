-- Durable per-user INBOX modseq / change-id (monotonic across restarts).
-- Seeds the in-memory EventBus inbox_version at boot so change ids never go backwards,
-- the invariant required for future CONDSTORE/QRESYNC-style delta sync.
CREATE TABLE IF NOT EXISTS mailbox_modseq (
    username TEXT PRIMARY KEY NOT NULL,
    modseq INTEGER NOT NULL DEFAULT 0
);
