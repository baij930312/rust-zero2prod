-- Add migration script here
ALTER TABLE users ADD salt TEXT NOT NULL;