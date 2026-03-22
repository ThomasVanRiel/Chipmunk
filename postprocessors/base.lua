-- base.lua, available via require("base")
local M = {}

function M.fmt(n, decimals)
	return string.format("%." .. decimals .. "f", n)
end

function M.hhCoord(axis, value)
	local sign = value >= 0 and "+" or ""
	return axis .. sign .. M.fmt(value, 3)
end

return M
