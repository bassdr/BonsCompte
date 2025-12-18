-- Migration 003: User Management & Advanced Invite System
-- Adds project invite settings, member status, and participant-specific invites

-- Add project settings for invite control
ALTER TABLE projects ADD COLUMN invites_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE projects ADD COLUMN require_approval INTEGER NOT NULL DEFAULT 0;

-- Add status to project_members for approval workflow
-- Values: 'pending', 'active', 'rejected'
ALTER TABLE project_members ADD COLUMN status TEXT NOT NULL DEFAULT 'active';

-- Add invite tokens for participant-specific invites
CREATE TABLE IF NOT EXISTS participant_invites (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    participant_id INTEGER NOT NULL REFERENCES participants(id) ON DELETE CASCADE,
    invite_token TEXT UNIQUE NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    used_by INTEGER REFERENCES users(id),
    used_at TEXT,
    UNIQUE(participant_id)  -- One active invite per participant
);

-- Index for looking up invites by token
CREATE INDEX IF NOT EXISTS idx_participant_invites_token ON participant_invites(invite_token);
