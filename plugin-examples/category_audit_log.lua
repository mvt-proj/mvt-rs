-- category_audit_log.lua
--
-- Naming convention: {category}.lua  (category-level plugin)
-- Rename to match your actual category name.
--
-- Purpose: Log every tile request for an entire category to the server log.
-- Does NOT modify the SQL query (returns ""), so all features appear normally.
-- Useful for: access auditing, usage analytics, debugging tile request patterns.
--
-- Log lines appear under the mvt_server::plugins tracing target.
-- Example output:
--   INFO mvt_server::plugins plugin=mycategory tile z=12 x=1234 y=2345 layer=roads

function filter(ctx)
    log(string.format(
        "tile z=%d x=%d y=%d layer=%s",
        ctx.z, ctx.x, ctx.y, ctx.layer
    ))
    -- No SQL filter — just logging
    return ""
end
