-- postprocessors/heidenhain.lua
local base = require("base")
local Fmt = base.Fmt

-- Define module to return formatted NC code to Chipmunk
local M = {}

-- Set postprocessor information fields
M.name = "Heidenhain"
M.file_extension = ".h"

-- Indicate what canned cycles are supported by this postprocessor
-- Omit or return empty table if none are supported
M.capabilities = { cycles = { drill = {} } }

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
	lines[#lines + 1] = "0 BEGIN PGM " .. context.name .. " " .. string.upper(context.units)
	-- block form? Check context if stock is provided.

	-- NC Blocks
	for _, block in ipairs(blocks) do
		-- First, check for blocks that are important context for other lines (e.g. modal commands, spindle on)

		-- Match all blocks and append to lines
		-- `M.format_block` returns a table of lines, as some blocks are expanded into multiple lines.
		-- Every line in block_lines receives a line number, use `\n` if no line number is needed or permitted.
		local block_lines = M.format_block(block)
		if block_lines then
			for _, line in ipairs(block_lines) do
				lines[#lines + 1] = #lines .. " " .. line
			end
		else
			return nil, "unimplemented block: " .. block.type
		end
	end

	-- Postamble
	lines[#lines + 1] = #lines .. " M30"
	lines[#lines + 1] = #lines .. " END PGM " .. context.name .. " " .. string.upper(context.units)

	return table.concat(lines, "\n")
end

function M.format_block(block)
	------------------------------------------------------------------------------
	-- Standard blocks
	------------------------------------------------------------------------------
	if block.type == "operation_start" then
		return { "" }
	elseif block.type == "operation_end" then
		return { "" }
	elseif block.type == "tool_change" then
		return { "TOOL CALL " .. block.tool_number .. " Z S" .. block.spindle_speed }
	elseif block.type == "comment" then
		-- `* <comment>` is also a valid comment block, when to use what comment type?
		return { "; " .. block.text }
	elseif block.type == "stop" then
		return { "M0" }
	elseif block.type == "spindle_on" then
		-- TODO: tricky block as it is merged with the next rapid
		-- > Q: But what if there is no rapid programmed before the next cut?
		-- > A: For now, we activate the spindle with a dummy line
		return { "L M3" }
	elseif block.type == "spindle_off" then
		-- TODO: tricky block as it is merged with the next (or previous in some cases) rapid
		-- For now, we stop the spindle with a dummy line
		return { "L M5" }

	------------------------------------------------------------------------------
	-- Moves
	------------------------------------------------------------------------------
	elseif block.type == "retract" then
		return { "L " .. M.ax_coord("Z", block.height) .. " FMAX" }
	elseif block.type == "retract_full" then
		-- Retract in machine coordinates to the top of the z-axis
		return { "L Z+0 R0 FMAX M92" }
	elseif block.type == "home" then
		-- Retract in machine coordinates first, then home in the plane
		return { "L Z+0 R0 FMAX M92", "L X+0 Y+0 R0 FMAX M92" }
	elseif block.type == "rapid" then
		return { "L " .. M.format_coords(block) .. " FMAX" }

	------------------------------------------------------------------------------
	-- Cycles
	------------------------------------------------------------------------------
	elseif block.type == "cycle_call" then
		-- Or we can use lines `L X Y Z FMAX` and `CYCL CALL`
		return { "L " .. M.format_coords(block) .. " FMAX M99" }
	elseif block.type == "cycle_drill" then
		return { table.concat(M.CYCLE200(block), "~\n") }
	end

	-- Unknown block
	return nil
end

function M.CYCLE200(block)
	local cycle = {}
	cycle[#cycle + 1] = "CYCL DEF 200 DRILLING"
	cycle[#cycle + 1] = "   Q200=" .. M.cycle_coord(block.clearance) .. ";SET-UP CLEARANCE"
	cycle[#cycle + 1] = "   Q201=" .. M.cycle_coord(block.depth) .. ";DEPTH"
	cycle[#cycle + 1] = "   Q206=" .. M.cycle_coord(block.feed) .. ";FEED RATE FOR PLNGNG"
	cycle[#cycle + 1] = "   Q202=" .. M.cycle_coord(block.plunge_depth) .. ";PLUNGING DEPTH"
	cycle[#cycle + 1] = "   Q210=" .. M.cycle_coord(block.dwell_top) .. ";DWELL TIME AT TOP"
	cycle[#cycle + 1] = "   Q203=" .. M.cycle_coord(block.surface_position) .. ";SURFACE COORDINATE"
	cycle[#cycle + 1] = "   Q204=" .. M.cycle_coord(block.second_clearance) .. ";2ND SET-UP CLEARANCE"
	cycle[#cycle + 1] = "   Q211=" .. M.cycle_coord(block.dwell_bottom) .. ";DWELL TIME AT DEPTH"
	cycle[#cycle + 1] = "   Q395=" .. M.cycle_coord(block.tip_trough and 1 or 0) .. ";DEPTH REFERENCE"
	return cycle
end

--------------------------------------------------------------------------------
-- Helper functions
--------------------------------------------------------------------------------
function M.format_coords(block)
	local lines = {}
	-- All coordinates are present in moves initially, but we optimize the blocks by dropping unchanged axes
	if block.x then
		lines[#lines + 1] = M.ax_coord("X", block.x)
	end
	if block.y then
		lines[#lines + 1] = M.ax_coord("Y", block.y)
	end
	if block.z then
		lines[#lines + 1] = M.ax_coord("Z", block.z)
	end
	return table.concat(lines, " ")
end

function M.ax_coord(axis, value)
	return axis .. M.coord(value)
end

function M.coord(value)
	local sign = value >= 0 and "+" or "-"
	return sign .. Fmt(value, 3)
end

function M.cycle_coord(value)
	local sign = value >= 0 and "+" or "-"
	return sign .. string.format("%-15s", value)
end

return M
