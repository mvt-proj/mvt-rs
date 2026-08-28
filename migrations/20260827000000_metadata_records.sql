-- ISO 19115 metadata records, one per layer (Phase 1 MVP storage foundation).
-- No SQL foreign keys: config/db.rs connects without `PRAGMA foreign_keys=ON`,
-- so a declared FK here would silently never fire. Cascade delete is handled
-- in Rust inside config::layers::delete_layer (see design decision #2).
CREATE TABLE metadata_records (
    id TEXT PRIMARY KEY,
    layer_id TEXT NOT NULL UNIQUE,
    file_identifier TEXT NOT NULL,
    language TEXT NOT NULL DEFAULT 'spa',
    character_set TEXT,
    topic_category TEXT,
    keywords TEXT,
    data_creator_contact TEXT,
    metadata_contact TEXT,
    maintenance_frequency TEXT,
    restrictions TEXT,
    lineage TEXT,
    scale TEXT,
    spatial_resolution TEXT,
    status TEXT,
    edition TEXT,
    reference_date TEXT,
    metadata_date TEXT NOT NULL
);

CREATE TABLE metadata_links (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    protocol TEXT NOT NULL,
    url TEXT NOT NULL,
    label TEXT
);

CREATE INDEX idx_metadata_links_record_id ON metadata_links(record_id);
