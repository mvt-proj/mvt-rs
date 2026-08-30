-- Catalog visibility state for metadata_records (draft/published), independent
-- of the resource's own MD_ProgressCode `status` column. Defaults every
-- existing/new row to 'published' so current public-API behavior is
-- unchanged unless an admin explicitly sets a record to 'draft'.
ALTER TABLE metadata_records ADD COLUMN workflow_status TEXT NOT NULL DEFAULT 'published';
