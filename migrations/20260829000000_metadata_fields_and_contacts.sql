-- Phase 1.5: adds 8 descriptive fields to metadata_records and introduces
-- metadata_contacts (structural clone of metadata_links: no SQL FK — see
-- config/db.rs, no PRAGMA foreign_keys=ON — cascade delete is handled in
-- Rust inside config::metadata::delete_metadata_for_layer, called from
-- config::layers::delete_layer).
--
-- Destructive step (design decision #1, user-approved): the 3 DROP COLUMNs
-- below discard `data_creator_contact`, `metadata_contact`, and
-- `reference_date` with no data migration. libsqlite3-sys 0.37.0 bundles
-- SQLite well past 3.35 (minimum for DROP COLUMN); all 3 columns are plain,
-- unindexed, non-UNIQUE, non-PK, so none of SQLite's DROP COLUMN
-- restrictions apply.
ALTER TABLE metadata_records ADD COLUMN purpose TEXT;
ALTER TABLE metadata_records ADD COLUMN creation_date TEXT;
ALTER TABLE metadata_records ADD COLUMN publication_date TEXT;
ALTER TABLE metadata_records ADD COLUMN revision_date TEXT;
ALTER TABLE metadata_records ADD COLUMN temporal_extent_start TEXT;
ALTER TABLE metadata_records ADD COLUMN temporal_extent_end TEXT;
ALTER TABLE metadata_records ADD COLUMN credits TEXT;
ALTER TABLE metadata_records ADD COLUMN supplemental_information TEXT;

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

ALTER TABLE metadata_records DROP COLUMN data_creator_contact;
ALTER TABLE metadata_records DROP COLUMN metadata_contact;
ALTER TABLE metadata_records DROP COLUMN reference_date;
