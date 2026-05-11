# Plugin Examples

Ready-to-use Lua plugin templates for MVT-RS. Copy any file to your `plugins/` directory, rename it to match your layer, and adjust the SQL conditions and thresholds for your data.

See [docs/plugins.md](../docs/plugins.md) for the full plugin system documentation.

## Examples

| File | Scope | Use case |
|---|---|---|
| `zoom_density.lua` | layer | Show fewer features at low zoom based on an importance rank |
| `area_threshold.lua` | layer | Filter polygons by minimum area per zoom level |
| `time_window.lua` | category or layer | Restrict access to business hours, block weekends |
| `maintenance_blackout.lua` | category or layer | Block all access during a scheduled maintenance window |
| `status_filter.lua` | layer | Show only features with certain status values |
| `category_audit_log.lua` | category | Log every tile request without modifying query results |

## Quick start

```bash
# Copy an example to your plugins directory
cp plugin-examples/zoom_density.lua plugins/mycategory_mylayer.lua

# Edit column names and thresholds
$EDITOR plugins/mycategory_mylayer.lua

# Restart the server to pick up the new plugin
# On startup you should see:
#   INFO mvt_server::plugins Loaded Lua plugin: 'mycategory_mylayer'
```

## Combining plugins

You can have both a category-level plugin (`mycategory.lua`) and a layer-level plugin (`mycategory_mylayer.lua`) active at the same time. Their `filter()` return values are joined with `AND`:

```
-- mycategory.lua returns:      "active = true"
-- mycategory_mylayer.lua returns: "type = 'primary'"
-- Final WHERE clause:          "active = true AND type = 'primary'"
```
