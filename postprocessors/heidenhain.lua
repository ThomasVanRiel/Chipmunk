-- postprocessors/heidenhain.lua
-- Load base module with helper functions
local base = require("base")

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
	-- (None for heidenhain)

	-- NC Blocks
	for _, block in ipairs(blocks) do
		-- match all blocks and append to lines
		local line = M.format_block(block)
		if line then
			lines[#lines + 1] = line
		end
	end

	-- Postamble
	-- (None for heidenhain)

	return table.concat(lines, "\n")
end

function M.format_block(block)
	if block.type == "program_start" then
		return "BEGIN PGM " .. block.name .. block.units
	elseif block.type == "program_end" then
		return "END PGM " .. block.name .. block.units
	elseif block.type == "tool_change" then
		return "TOOL CALL " .. block.tool_number .. "Z S" .. block.spindle_speed
	elseif block.type == "comment" then
		return "; " .. block.text
	elseif block.type == "stop" then
		return "M0"
	elseif block.type == "spindle_on" then
		-- TODO: tricky block as it is merged with the next rapid
		return ""
	elseif block.type == "spindle_off" then
		-- TODO: tricky block as it is merged with the next rapid
		return ""
	elseif block.type == "rapid" then
		local line = "L "
		-- TODO: extract to separate function as linear feed will have same logic
		if base.x then
			line = line .. base.hhCoord("X", base.x)
		end
		if base.y then
			line = line .. base.hhCoord("Y", base.y)
		end
		if base.z then
			line = line .. base.hhCoord("Z", base.z)
		end
		line = line .. " FMAX"
		return line
	end
	-- Unknown block
	return nil
end

return M
