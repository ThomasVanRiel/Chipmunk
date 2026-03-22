function Fmt(n, decimals)
	return string.format("%." .. decimals .. "f", n)
end

function HhCoord(axis, value)
	local sign = value >= 0 and "+" or ""
	return axis .. sign .. Fmt(value, 3)
end
