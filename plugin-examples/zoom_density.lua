-- zoom_density.lua
--
-- Naming convention: {category}_{layer}.lua
-- Rename to match your actual category and layer names.
--
-- Purpose: Show fewer, more important features at low zoom levels and
-- progressively reveal more detail as the user zooms in.
-- Useful for: population centers, road networks, building footprints.
--
-- Assumes the table has a numeric column indicating feature importance
-- (e.g. population, area, road class rank). Adjust column names below.

function filter(ctx)
    if ctx.z < 6 then
        -- World overview: only major features
        return "importance_rank <= 1"
    end

    if ctx.z < 9 then
        -- Regional: top two tiers
        return "importance_rank <= 2"
    end

    if ctx.z < 12 then
        -- City scale: exclude the least important features
        return "importance_rank <= 4"
    end

    -- Street level and above: show everything
    return ""
end
