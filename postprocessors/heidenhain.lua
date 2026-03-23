-- postprocessors/heidenhain.lua
-- Base module with helper functions is automatically loaded
-- `local base = require("base")` is not required (nor allowed)

-- Define module to return formatted NC code to Chipmunk
local M = {}

-- Set postprocessor information fields
M.name = "Heidenhain"
M.file_extension = ".h"

-- Indicate what canned cycles are supported by this postprocessor
-- Omit or return empty table if none are supported
M.supported_cycles = {}

-- This function is called by Chipmunk with the list of IR blocks
-- blocks: array of block tables (see IR documentation)
-- context: program conext table (see IR documentation)
-- Returns: NC code as single string on success
-- On error: return nil, "Descriptive error message"
-- Chipmunk prints the error message to stderr and exits with code 1
-- Use the error for machine specific validation (overtravel, unsupported cycles)
-- Error content is free form, no structure is imposed
function M.generate(blocks, context)
	-- Prepare table to store all lines of the NC program
	local lines = {}

	-- Preamble
	lines[#lines + 1] = "0 BEGIN PGM " .. context.name .. " " .. context.units
	-- block form?

	-- NC Blocks
	for _, block in ipairs(blocks) do
		-- match all blocks and append to lines
		local line = M.format_block(block)
		if line then
			lines[#lines + 1] = #lines .. " " .. line
		end
	end

	-- Postamble
	-- TODO: Add retract to home? Add M30?
	lines[#lines + 1] = #lines .. " END PGM " .. context.name .. " " .. context.units

	return table.concat(lines, "\n")
end

function M.format_block(block)
	if block.type == "tool_change" then
		return "TOOL CALL " .. block.tool_number .. " Z S" .. block.spindle_speed
	elseif block.type == "comment" then
		return "; " .. block.comment
	elseif block.type == "stop" then
		return "M0"
	elseif block.type == "spindle_on" then
		-- TODO: tricky block as it is merged with the next rapid
		return ""
	elseif block.type == "spindle_off" then
		-- TODO: tricky block as it is merged with the next rapid
		return ""
	elseif block.type == "rapid" then
		local line = "L"
		line = line .. M.format_coords(block)
		line = line .. " FMAX"
		return line
	end
	-- Unknown block
	return nil
end

function M.format_coords(block)
	local line = ""
	if block.x then
		line = line .. " " .. M.hh_coord("X", block.x)
	end
	if block.y then
		line = line .. " " .. M.hh_coord("Y", block.y)
	end
	if block.z then
		line = line .. " " .. M.hh_coord("Z", block.z)
	end
	return line
end

function M.hh_coord(axis, value)
	local sign = value >= 0 and "+" or ""
	return axis .. sign .. Fmt(value, 3)
end

return M
