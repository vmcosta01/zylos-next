CREATE TABLE IF NOT EXISTS channels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    is_private INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    last_message_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_channels_name ON channels(name);