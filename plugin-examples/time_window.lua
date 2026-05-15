-- @name Business hours time window
-- @description Restricts tile serving to weekdays between 07:00 and 19:59 (server local time). Returns empty tiles outside the window. Useful for sensitive cadastral data, paid services, or maintenance windows.
-- @author MVT-Server examples
-- @version 1.0
--
-- Naming convention: {category}.lua or {category}_{layer}.lua
-- Rename to match your actual category / layer names.
--
-- Parameters to adjust:
--   START_HOUR  first allowed hour, inclusive (0–23)
--   END_HOUR    last allowed hour, exclusive (0–23)
--   Block weekends: controlled by the wday check below.
--   Time zone: os.date() uses server local time.
--              Use os.date("!%H") / os.date("!%w") for UTC.

local START_HOUR = 7    -- 07:00 inclusive
local END_HOUR   = 20   -- 20:00 exclusive (i.e. up to 19:59)

function filter(ctx)
    local hour = tonumber(os.date("%H"))   -- 0..23
    local wday = tonumber(os.date("%w"))   -- 0=Sunday, 6=Saturday

    if wday == 0 or wday == 6 then
        log(string.format("[time_window] weekend blocked (wday=%d, layer=%s)", wday, ctx.layer))
        return "1=0"
    end

    if hour < START_HOUR or hour >= END_HOUR then
        log(string.format("[time_window] outside hours (%02d:xx, layer=%s)", hour, ctx.layer))
        return "1=0"
    end

    return ""
end
