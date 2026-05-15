-- @name Maintenance blackout
-- @description Blocks all tile access during a scheduled maintenance window. Returns empty tiles during the window; serves normally otherwise. Activate by setting MAINT_ACTIVE = true and reloading the plugin.
-- @author MVT-Server examples
-- @version 1.0
--
-- Naming convention: {category}.lua or {category}_{layer}.lua
-- Rename to match your actual category / layer names.
--
-- Parameters to adjust:
--   MAINT_ACTIVE      set to true to enable the blackout
--   MAINT_START_HOUR  start of the window, inclusive (0–23)
--   MAINT_END_HOUR    end of the window, exclusive (0–23)
--   Time zone: os.date() uses server local time.
--              Use os.date("!%H") for UTC.

local MAINT_ACTIVE     = false
local MAINT_START_HOUR = 2     -- 02:00 inclusive
local MAINT_END_HOUR   = 4     -- 04:00 exclusive

function filter(ctx)
    if not MAINT_ACTIVE then
        return ""
    end

    local hour = tonumber(os.date("%H"))

    if hour >= MAINT_START_HOUR and hour < MAINT_END_HOUR then
        log(string.format(
            "[maintenance] blocking tile z=%d x=%d y=%d layer=%s",
            ctx.z, ctx.x, ctx.y, ctx.layer
        ))
        return "1=0"
    end

    return ""
end
