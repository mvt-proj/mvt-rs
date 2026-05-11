# MVT-RS Plugin System

MVT-RS supports a Lua scripting engine that lets sysadmins inject custom SQL `WHERE` clauses into tile queries at runtime — without modifying server code or restarting the process... (wait: plugins are loaded at startup; a restart is needed to pick up new/changed files).

## Overview

Plugins are Lua 5.4 scripts loaded from a configurable directory at server startup. Each plugin is sandboxed in its own Lua VM. When a tile is requested, the server calls the plugin's `filter()` function and appends the returned string to the SQL `WHERE` clause of the tile query.

This enables:
- Zoom-dependent feature filtering (density control)
- Time-based access control
- Business-hour restrictions
- Layer-specific data visibility rules
- Any logic expressible as a SQL condition

## Configuration

In `config/config.yaml` (or via environment variable):

```yaml
paths:
  plugins: plugins   # default: "plugins" relative to working directory
```

Environment variable: `MVT_SERVER__PATHS__PLUGINS=/srv/mvt/plugins`

## Directory and file naming

```
plugins/
├── {category}.lua            # category-level plugin — applies to ALL layers in the category
└── {category}_{layer}.lua    # layer-level plugin — applies to one specific layer
```

Where:
- `{category}` is the layer's category name (e.g. `caroya`, `public`)
- `{layer}` is the layer's name within that category (e.g. `roads`, `parcels`)

### Example

For a layer with `category = "caroya"` and `name = "parcels"`:

| File | Scope |
|---|---|
| `caroya.lua` | All layers in the `caroya` category |
| `caroya_parcels.lua` | Only the `caroya / parcels` layer |

If both files exist, both `filter()` functions run and their results are combined with `AND`.

## The `filter()` function

Every plugin must define a global `filter(ctx)` function:

```lua
function filter(ctx)
    -- ctx fields:
    --   ctx.layer     string           layer name
    --   ctx.category  string           category name
    --   ctx.z         integer          zoom level (0–22)
    --   ctx.x         integer          tile column
    --   ctx.y         integer          tile row
    --   ctx.user      string | nil     authenticated username, or nil if anonymous
    --   ctx.groups    table<string>    list of group names the user belongs to (empty table if anonymous)

    -- Return a SQL WHERE clause fragment (no leading "AND"), or "" for no filter.
    return "population > 1000"
end
```

### Return values

| Return value | Effect |
|---|---|
| `""` (empty string) | No extra condition added to the query |
| `"col > value"` | Appended to SQL `WHERE` with `AND` |
| `"1=0"` | Always-false condition: returns an empty tile |

The server validates the returned string against an SQL injection filter before using it.

## Global functions available in scripts

### `log(msg)`

Writes to the server's structured log at INFO level.

```lua
log(string.format("[my_plugin] zoom=%d", ctx.z))
```

Log output appears under the `mvt_server::plugins` tracing target. It is visible when the server runs with the default `mvt_server=info` filter.

## Access control with `ctx.user` and `ctx.groups`

`ctx.user` contains the authenticated username (string), or `nil` for anonymous requests.  
`ctx.groups` is always a Lua table (array of strings). It is empty for anonymous requests or users with no groups.

### Block anonymous requests

```lua
function filter(ctx)
    if ctx.user == nil then
        return "1=0"   -- return an empty tile for anonymous users
    end
    return ""
end
```

### Restrict by group membership

```lua
function filter(ctx)
    for _, g in ipairs(ctx.groups) do
        if g == "premium" then
            return ""   -- premium users see everything
        end
    end
    return "public = true"   -- others only see public features
end
```

### Combine user/group with zoom or other conditions

```lua
function filter(ctx)
    -- Admins always get the full dataset
    for _, g in ipairs(ctx.groups) do
        if g == "admin" then return "" end
    end

    -- Anonymous users get coarse zoom only
    if ctx.user == nil then
        if ctx.z < 12 then return "" end
        return "1=0"
    end

    return ""
end
```

### Authentication methods

`ctx.user` and `ctx.groups` are populated from whichever auth method the client uses:

| Client auth | `ctx.user` populated? |
|---|---|
| Bearer JWT token | Yes |
| HTTP Basic auth | Yes |
| Session cookie (admin panel) | Yes |
| No credentials | No (`nil`) |

## Filter combination

When both a category plugin and a layer plugin exist:

```
cat_result = category_plugin.filter(ctx)   -- e.g. "active = true"
lay_result = layer_plugin.filter(ctx)      -- e.g. "type = 'primary'"

final = "active = true AND type = 'primary'"
```

Empty strings are skipped. `None` (plugin absent or runtime error) contributes nothing.

| cat result | layer result | combined |
|---|---|---|
| absent | absent | no plugin active (cache works normally) |
| `""` | absent | plugin active, no extra filter |
| `"clause"` | absent | `"clause"` |
| absent | `"clause"` | `"clause"` |
| `"A"` | `"B"` | `"A AND B"` |
| `""` | `"B"` | `"B"` |
| `"A"` | `""` | `"A"` |
| `""` | `""` | plugin active, no extra filter |

## Cache behavior

Layers with an active plugin **bypass the server tile cache** (both read and write). This ensures that time-dependent or context-dependent filters always produce fresh results. Tiles are still fetched from the PostGIS database on every request.

If a layer has no active plugin, normal cache behavior applies.

## Error handling

| Situation | Behavior |
|---|---|
| Script has invalid Lua syntax | Logged as warning at startup; plugin not loaded |
| `filter()` not defined | Returns `None` (no filter, no crash) |
| `filter()` raises a runtime error | Logged as warning; that plugin contributes `None` |
| Returned string fails SQL injection check | Request fails with 400 Bad Request |

The server never crashes due to a plugin error. A misbehaving plugin produces a warning in the log and the tile is served without the plugin's filter.

## Performance

Each plugin lives in its own Lua VM protected by a `tokio::sync::Mutex`. The overhead per tile request is:
- One mutex acquisition per active plugin (microseconds)
- One Lua function call (microseconds)
- No I/O, no allocations beyond the returned string

For typical tile servers (< 1000 req/s), overhead is negligible. Under extreme load, plugin mutex contention is the bottleneck; a VM pool would be the next optimization.

## Writing your first plugin

1. Decide the scope: category or layer.
2. Create a file named `{category}.lua` or `{category}_{layer}.lua` in the plugins directory.
3. Define a `filter(ctx)` function that returns a SQL condition string.
4. Restart the server (plugins are loaded at startup).
5. Watch the logs for `Loaded Lua plugin: '...'` at startup and your `log()` calls during tile requests.

### Minimal example

```lua
-- public_roads.lua
-- Applies to: category="public", layer="roads"

function filter(ctx)
    if ctx.z < 10 then
        return "road_class IN ('motorway', 'trunk', 'primary')"
    end
    return ""
end
```

### Testing your filter

The easiest way to test is to request a tile and inspect the server log. With `RUST_LOG=mvt_server=debug`, the full SQL query including injected WHERE clauses is printed.

## Security notes

- Plugin files are read from a directory controlled by the sysadmin, not by end users.
- The SQL string returned by `filter()` passes through the same injection validator used by URL query parameters (`validate_filter` in `src/services/utils.rs`).
- Lua's `os`, `io`, and `require` standard libraries are available to scripts. Restrict filesystem permissions on the plugins directory appropriately.

## Plugin examples

See the `plugin-examples/` directory for documented examples covering common use cases.
