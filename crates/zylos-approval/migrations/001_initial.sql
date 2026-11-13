-- Migration: Approval Gates HITL
-- Crate: zylos-approval

CREATE TABLE IF NOT EXISTS approval_requests (
    id TEXT PRIMARY KEY,
    bot_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    action_category TEXT NOT NULL,
    summary TEXT NOT NULL,
    arguments_json TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    decision TEXT,
    refuse_reason TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    decided_at TEXT,
    expires_at TEXT,
    receipt_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_approval_bot_status 
    ON approval_requests(bot_id, status);

CREATE INDEX IF NOT EXISTS idx_approval_created 
    ON approval_requests(created_at);

CREATE INDEX IF NOT EXISTS idx_approval_pending 
    ON approval_requests(status) 
    WHERE status = 'pending';

CREATE TABLE IF NOT EXISTS approval_rules (
    id TEXT PRIMARY KEY,
    bot_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    action_category TEXT NOT NULL,
    arguments_pattern TEXT,
    decision TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    created_by TEXT NOT NULL DEFAULT 'user',
    UNIQUE(bot_id, tool_name, action_category, arguments_pattern)
);

CREATE INDEX IF NOT EXISTS idx_approval_rules_lookup 
    ON approval_rules(bot_id, tool_name, action_category);