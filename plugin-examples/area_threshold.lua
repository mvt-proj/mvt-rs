-- area_threshold.lua
--
-- Naming convention: {category}_{layer}.lua
-- Rename to match your actual category and layer names.
--
-- Purpose: Filter polygon features by minimum area at each zoom level.
-- Small polygons at low zoom are invisible anyway and just slow down
-- tile generation. This reduces query cost and tile size significantly.
--
-- Useful for: land use, parcels, administrative boundaries, water bodies.
--
-- Assumes the table has a numeric area column (e.g. ST_Area, shape_area).
-- Units depend on the column; adjust thresholds accordingly.

function filter(ctx)
    if ctx.z < 7 then
        -- Continental / country scale: only very large polygons
        return "shape_area > 1000000000"   -- > 1,000 km² (in m²)
    end

    if ctx.z < 10 then
        -- Regional scale
        return "shape_area > 10000000"     -- > 10 km²
    end

    if ctx.z < 13 then
        -- City scale
        return "shape_area > 50000"        -- > 0.05 km²
    end

    if ctx.z < 15 then
        -- Neighborhood scale
        return "shape_area > 1000"         -- > 1,000 m²
    end

    -- Street level and above: show all features
    return ""
end
