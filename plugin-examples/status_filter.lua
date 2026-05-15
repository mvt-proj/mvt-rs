-- @name Status filter
-- @description Shows only features in a specific operational status. Visible statuses expand progressively with zoom level. Useful for infrastructure networks (roads, utilities, pipelines).
-- @author MVT-Server examples
-- @version 1.0
--
-- Naming convention: {category}_{layer}.lua
-- Rename to match your actual category and layer names.
--
-- Parameters to adjust:
--   VISIBLE_BELOW_10  statuses shown at low zoom (z < 10)
--   VISIBLE_FROM_10   statuses shown at mid zoom (10 ≤ z < 14)
--   VISIBLE_FROM_14   statuses shown at high zoom (z ≥ 14)
--   The status column is named "status"; rename if your table differs.

local VISIBLE_BELOW_10 = "'active'"
local VISIBLE_FROM_10  = "'active', 'under_construction'"
local VISIBLE_FROM_14  = "'active', 'under_construction', 'planned'"

function filter(ctx)
    if ctx.z < 10 then
        return string.format("status IN (%s)", VISIBLE_BELOW_10)
    end

    if ctx.z < 14 then
        return string.format("status IN (%s)", VISIBLE_FROM_10)
    end

    return string.format("status IN (%s)", VISIBLE_FROM_14)
end
