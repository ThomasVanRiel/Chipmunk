-- Base helper library for post-processors.
-- This file is compiled into the binary at build time via include_str!().
-- Changes here are NOT picked up at runtime — recompile to apply them.
-- Post-processors use: local base = require("base")

local M = {}

---Shorthand alias for coordinate string formatting
---The number is formatted to the number of decimals
---e.g. Fmt(10,3) -> "10.000"
---@param number number
---@param decimals number
---@return string
function M.Fmt(number, decimals)
	return string.format("%." .. decimals .. "f", number)
end

return M
