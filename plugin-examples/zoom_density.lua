-- @name Zoom-based density filter
-- @description Shows fewer, more important features at low zoom and progressively reveals detail as the user zooms in. Useful for population centers, road networks, building footprints.
-- @author MVT-Server examples
-- @version 1.0
--
-- Naming convention: {category}_{layer}.lua
-- Rename to match your actual category and layer names.
--
-- Parameters to adjust:
--   The "importance_rank" column must exist in your table (numeric, lower = more important).
--   Rename it and adjust the rank thresholds below to match your data.

function filter(ctx)
    if ctx.z < 6 then
        return "importance_rank <= 1"
    end

    if ctx.z < 9 then
        return "importance_rank <= 2"
    end

    if ctx.z < 12 then
        return "importance_rank <= 4"
    end

    return ""
end
