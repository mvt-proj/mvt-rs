-- Add migration script here
ALTER TABLE layers ADD COLUMN max_records INTEGER NOT NULL DEFAULT 0;
UPDATE layers SET max_records = 0;
