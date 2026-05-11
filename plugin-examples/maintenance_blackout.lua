-- maintenance_blackout.lua
--
-- Naming convention: {category}.lua or {category}_{layer}.lua
-- Rename to match your actual category / layer names.
--
-- Purpose: Block all tile access during a scheduled maintenance window.
-- Returns empty tiles (1=0) during the window; serves normally otherwise.
--
-- To activate a maintenance window, edit the MAINT_* constants below
-- and restart the server.  Set MAINT_ACTIVE = false to disable.
--
-- Times are in server local time. For UTC, replace os.date with os.date("!...")

-- ── Configuration ────────────────────────────────────────────────────────────
local MAINT_ACTIVE     = false   -- set to true during maintenance
local MAINT_START_HOUR = 2       -- start of window (inclusive), 0–23
local MAINT_END_HOUR   = 4       -- end of window (exclusive), 0–23
-- ─────────────────────────────────────────────────────────────────────────────

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
