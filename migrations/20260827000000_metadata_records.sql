-- ISO 19115 metadata records, one per layer (Phase 1 MVP storage foundation,
-- squashed with the Phase 1.5 descriptive-fields/contacts follow-up before
-- merge — this branch has no external consumers yet, so the two migrations
-- are unified into the schema's actual starting shape instead of an
-- add-then-drop history).
--
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
    maintenance_frequency TEXT,
    restrictions TEXT,
    lineage TEXT,
    scale TEXT,
    spatial_resolution TEXT,
    status TEXT,
    edition TEXT,
    purpose TEXT,
    creation_date TEXT,
    publication_date TEXT,
    revision_date TEXT,
    temporal_extent_start TEXT,
    temporal_extent_end TEXT,
    credits TEXT,
    supplemental_information TEXT,
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

-- ISO 19115 CI_ResponsibleParty, one row per contact/role on a metadata
-- record. Structural clone of metadata_links: no SQL FK (same reason as
-- above), cascade delete handled in Rust alongside metadata_links.
CREATE TABLE metadata_contacts (
    id TEXT PRIMARY KEY,
    record_id TEXT NOT NULL,
    individual_name TEXT,
    organisation_name TEXT,
    position_name TEXT,
    email TEXT,
    phone TEXT,
    role TEXT NOT NULL
);

CREATE INDEX idx_metadata_contacts_record_id ON metadata_contacts(record_id);
