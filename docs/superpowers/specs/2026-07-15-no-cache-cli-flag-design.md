# `--no-cache` CLI Flag — Design

**Date:** 2026-07-15
**Status:** Approved by user
**Scope:** Add a CLI-only flag that disables the tile cache (Redis or disk)
entirely, for use in local development and tests. No YAML/env equivalent,
no startup cleanup of pre-existing cache contents.

## Problem

There is no way to run the server without a tile cache. Tests that want to
observe fresh tile generation on every request currently have no clean way
to bypass caching — they have to point at an empty disk cache dir and hope
nothing else populates it, or clear it manually between runs.

## Decisions

- **CLI-only.** `--no-cache` is parsed by `clap` and carried on `Settings`
  as a plain field, not merged into the `config` crate's YAML/env layering.
  No `MVT_...` env var, no `config.yaml` key. This matches the stated
  purpose (ad hoc test runs) and avoids adding a config surface that would
  need documenting/validating.
- **No-op, not "clear on start".** When disabled, the cache is skipped
  entirely: reads always miss, writes never persist, and — critically —
  nothing touches Redis or the disk cache directory at startup. Today
  `CacheWrapper::initialize_cache` unconditionally clears the cache
  (`redis_cache.delete_cache` / `disk_cache.delete_cache_dir`) before
  returning; with `--no-cache` that whole path is skipped, so any
  pre-existing cache on disk/Redis is left untouched.
- **Single new `CacheMode` variant**, not conditionals scattered across the
  three tile-handler call sites (`src/services/tiles/handlers.rs:147,297,411`).
  All existing callers already go through `CacheWrapper`'s methods, so a
  `CacheMode::Disabled` arm in each method keeps the no-op behavior in one
  place and callers stay unchanged.

## Changes

### `src/config/settings.rs`
- `CliArgs` gains `#[arg(long)] pub no_cache: bool` (clap flag, default
  `false`).
- `Settings` gains `#[serde(skip)] pub no_cache: bool`, set from
  `args.no_cache` in `Settings::new()` after the config builder step
  (independent of the `config` crate's file/env layering, mirroring how
  `args.host`/`args.port` are read from the same `CliArgs` value).

### `src/cache/cachewrapper.rs`
- `CacheMode` gains a `Disabled` variant (unit variant, no wrapped state).
- `CacheWrapper::initialize_cache(redis_conn, disk_cache_dir, catalog, disabled: bool)`
  — new `disabled` parameter. When `true`, returns `CacheWrapper { mode: CacheMode::Disabled }`
  immediately, before constructing `RedisCache`/`DiskCache` or issuing any
  clear.
- Every method matches `Disabled` as a no-op:
  - `get_tile` → `None`
  - `write_tile` → `Ok(())`
  - `delete_cache` → `Ok(())`, skipping the per-layer version-increment loop
    (nothing to invalidate when nothing is ever cached)
  - `delete_layer_cache` → `Ok(())`, no version increment
  - `get_layer_version` → `0`
  - `increment_layer_version` → no-op
  - `exists_key` → `Ok(false)`
  - `cache_dir` → `PathBuf::new()`

### `src/main.rs`
- Both call sites of `CacheWrapper::initialize_cache` (cluster-client
  branch, ~line 240, and standalone/shared/owner branch, ~line 287) pass
  `settings.no_cache` as the new argument.

### Tests
- `src/cache/cachewrapper.rs` unit tests (new `#[cfg(test)]` module):
  - `initialize_cache` with `disabled: true` succeeds without a Redis URL
    and without creating/touching a disk cache directory.
  - `write_tile` followed by `get_tile` on a `Disabled` wrapper returns
    `None` (write did not persist).
  - `delete_cache` / `delete_layer_cache` / `exists_key` / `get_layer_version`
    on `Disabled` return their documented no-op values without error.

## Out of scope

- YAML/env configuration for this setting.
- Clearing pre-existing cache contents when `--no-cache` is passed.
- README/TUTORIAL documentation.
- Any change to ETag computation semantics beyond `get_layer_version`
  returning a constant `0` in disabled mode (existing callers in
  `src/services/tiles/handlers.rs` already treat the version as an opaque
  cache-busting counter, so a fixed `0` is a valid value, just never
  incremented).
