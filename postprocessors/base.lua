-- Base helper library for post-processors.
-- This file is compiled into the binary at build time via include_str!().
-- Changes here are NOT picked up at runtime — recompile to apply them.
-- Post-processors use: local base = require("base")

local M = {}

function M.Fmt(n, decimals)
	return string.format("%." .. decimals .. "f", n)
end

return M
