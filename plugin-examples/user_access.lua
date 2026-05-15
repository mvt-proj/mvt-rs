-- @name User access control
-- @description Restricts tile access by authentication and group membership. Anonymous users receive empty tiles; privileged groups get the full dataset; other authenticated users see only public features.
-- @author MVT-Server examples
-- @version 1.0
--
-- Naming convention: {category}.lua or {category}_{layer}.lua
-- Rename to match your actual category / layer names.
--
-- Parameters to adjust:
--   PRIVILEGED_GROUPS  list of group names that bypass all filters
--   The "public" column must exist in your table; rename if needed.
--
-- Access tiers:
--   admin / premium → full dataset, no filter
--   (any authenticated user) → only features where public = true
--   anonymous → empty tile (1=0)

local PRIVILEGED_GROUPS = { "admin", "premium" }

local function has_group(groups, target)
    for _, g in ipairs(groups) do
        if g == target then return true end
    end
    return false
end

function filter(ctx)
    if ctx.user == nil then
        return "1=0"
    end

    for _, g in ipairs(PRIVILEGED_GROUPS) do
        if has_group(ctx.groups, g) then
            return ""
        end
    end

    return "public = true"
end
