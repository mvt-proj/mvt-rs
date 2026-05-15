-- @name Area threshold filter
-- @description Filters polygon features by minimum area at each zoom level. Avoids loading tiny polygons that are invisible at low zoom, reducing tile size and query cost. Useful for land use, parcels, administrative boundaries, water bodies.
-- @author MVT-Server examples
-- @version 1.0
--
-- Naming convention: {category}_{layer}.lua
-- Rename to match your actual category and layer names.
--
-- Parameters to adjust:
--   The "shape_area" column must exist in your table (numeric, in m² if using ST_Area).
--   Rename it and adjust the area thresholds below to match your data and units.

function filter(ctx)
    if ctx.z < 7 then
        return "shape_area > 1000000000"   -- > 1,000 km²
    end

    if ctx.z < 10 then
        return "shape_area > 10000000"     -- > 10 km²
    end

    if ctx.z < 13 then
        return "shape_area > 50000"        -- > 0.05 km²
    end

    if ctx.z < 15 then
        return "shape_area > 1000"         -- > 1,000 m²
    end

    return ""
end
