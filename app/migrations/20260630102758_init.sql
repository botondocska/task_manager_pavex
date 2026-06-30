-- Add migration script here
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE CHECK(length(username) <= 255),
    email TEXT NOT NULL UNIQUE CHECK(length(email) <= 255),
    password_hash TEXT NOT NULL CHECK(length(password_hash) <= 255),
    bio TEXT CHECK(bio IS NULL OR length(bio) <= 2048),
    image TEXT CHECK(image IS NULL OR length(image) <= 2048),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE TABLE IF NOT EXISTS labels (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE CHECK(length(name) <= 255),
    color TEXT NOT NULL CHECK(length(color) <= 7)
) STRICT;

CREATE TABLE IF NOT EXISTS todos (
    id INTEGER PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    label_id INTEGER REFERENCES labels(id) ON DELETE SET NULL,
    duration INTEGER, -- Minutes
    rrule TEXT CHECK(rrule IS NULL OR length(rrule) <= 2048),
    title TEXT NOT NULL CHECK(length(title) <= 255),
    description TEXT CHECK(description IS NULL OR length(description) <= 2048),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE TABLE IF NOT EXISTS todo_history (
    id INTEGER PRIMARY KEY NOT NULL,
    completed_todo_id INTEGER NOT NULL REFERENCES todos(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;