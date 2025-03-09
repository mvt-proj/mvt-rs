CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password TEXT NOT NULL,
    groups TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS categories (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS layers (
    id TEXT PRIMARY KEY,
    category TEXT NOT NULL,
    geometry TEXT NOT NULL,
    name TEXT NOT NULL,
    alias TEXT NOT NULL,
    description TEXT NOT NULL,
    schema TEXT NOT NULL,
    table_name TEXT NOT NULL,
    fields TEXT NOT NULL,
    filter TEXT,
    srid INTEGER,
    geom TEXT,
    sql_mode TEXT,
    buffer INTEGER,
    extent INTEGER,
    zmin INTEGER,
    zmax INTEGER,
    zmax_do_not_simplify INTEGER,
    buffer_do_not_simplify INTEGER,
    extent_do_not_simplify INTEGER,
    clip_geom BOOLEAN,
    delete_cache_on_start BOOLEAN,
    max_cache_age INTEGER,
    published BOOLEAN NOT NULL,
    url TEXT,
    groups TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS groups (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS styles (
    id TEXT PRIMARY KEY NOT NULL,
    category TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    style TEXT NOT NULL
);


INSERT OR IGNORE INTO groups (id, name, description) VALUES
    ('7091390e-5cec-47d7-9d39-4f068d945788', 'admin', 'admin role');

INSERT OR IGNORE INTO categories (id, name, description) VALUES
    ('cf8cdae0-78e8-490e-84bc-b75a5d1fa625', 'public', 'public category');
