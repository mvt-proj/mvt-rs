-- time_window.lua
--
-- Naming convention: {category}.lua  (category-level) or {category}_{layer}.lua
-- Rename to match your actual category / layer names.
--
-- Purpose: Restrict tile serving to a specific time window (business hours).
-- Outside the window, the plugin returns "1=0", which is an always-false
-- SQL condition — PostGIS returns an empty tile instead of real data.
--
-- Useful for: sensitive cadastral data, paid data services, maintenance windows.
--
-- Note: os.date() uses the server's local time zone.
-- For UTC, use os.date("!%H") (note the leading "!").

function filter(ctx)
    local hour = tonumber(os.date("%H"))   -- 0..23, local time
    local wday = tonumber(os.date("%w"))   -- 0=Sunday, 6=Saturday

    -- Block on weekends
    if wday == 0 or wday == 6 then
        log(string.format(
            "[time_window] weekend access blocked (wday=%d, layer=%s)",
            wday, ctx.layer
        ))
        return "1=0"
    end

    -- Block outside 07:00–19:59 on weekdays
    if hour < 7 or hour >= 20 then
        log(string.format(
            "[time_window] outside business hours (%02d:xx, layer=%s)",
            hour, ctx.layer
        ))
        return "1=0"
    end

    return ""
end
