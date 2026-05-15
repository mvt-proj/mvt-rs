-- @name Category audit log
-- @description Logs every tile request for an entire category to the server log without modifying the query. Useful for access auditing, usage analytics, and debugging tile request patterns.
-- @author MVT-Server examples
-- @version 1.0
--
-- Naming convention: {category}.lua  (category-level plugin)
-- Rename to match your actual category name.
--
-- No configurable parameters — just rename the file.
-- Log lines appear under the mvt_server::plugins tracing target.
-- Example output:
--   INFO mvt_server::plugins plugin=mycategory tile z=12 x=1234 y=2345 layer=roads

function filter(ctx)
    log(string.format(
        "tile z=%d x=%d y=%d layer=%s user=%s",
        ctx.z, ctx.x, ctx.y, ctx.layer, ctx.user or "anonymous"
    ))
    return ""
end
