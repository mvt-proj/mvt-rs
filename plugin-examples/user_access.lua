-- user_access.lua
-- Applies to: any layer or category
--
-- Demonstrates access control based on the authenticated user and their groups.
-- Rename to {category}.lua or {category}_{layer}.lua as needed.
--
-- Access tiers:
--   admin   → full dataset, no filter
--   premium → full dataset, no filter
--   (any authenticated user) → only public features
--   anonymous → empty tile

-- Group names that bypass all filters
local PRIVILEGED_GROUPS = { "admin", "premium" }

local function has_group(groups, target)
    for _, g in ipairs(groups) do
        if g == target then return true end
    end
    return false
end

function filter(ctx)
    -- Block anonymous requests entirely
    if ctx.user == nil then
        return "1=0"
    end

    -- Privileged groups get the full dataset
    for _, g in ipairs(PRIVILEGED_GROUPS) do
        if has_group(ctx.groups, g) then
            return ""
        end
    end

    -- Authenticated users without a privileged group see only public features
    return "public = true"
end
