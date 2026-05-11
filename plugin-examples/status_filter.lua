-- status_filter.lua
--
-- Naming convention: {category}_{layer}.lua
-- Rename to match your actual category and layer names.
--
-- Purpose: Show only features in a specific operational status.
-- Useful for: infrastructure networks (roads, utilities, pipelines) where
-- features can be planned, under construction, active, or decommissioned.
--
-- The set of visible statuses can be changed here without touching the
-- database or restarting the server (only a plugin reload — i.e. restart
-- with the updated file — is needed).

-- Statuses to include at each zoom level.
-- Adjust the values to match your table's status column.
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

    -- At high zoom, show planned features too
    return string.format("status IN (%s)", VISIBLE_FROM_14)
end
