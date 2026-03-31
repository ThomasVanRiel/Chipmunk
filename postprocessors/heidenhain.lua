-- postprocessors/heidenhain.lua
local base = require("base")
local Fmt = base.Fmt

-- Define module to return formatted NC code to Chipmunk
local M = {}

-- Prepare module variables to store states
M.spindle_state = "off"
M.coolant_state = false

-- Set postprocessor information fields
M.name = "Heidenhain"
M.file_extension = ".h"

-- Indicate what canned cycles are supported by this postprocessor
-- Omit or return empty table if none are supported
-- Chipmunk emits NC Cycle blocks if support is declared (see IR documentation)
-- Make sure the cycles are handled correctly, e.g., `drill` expands to `CYCLE 200` in Heidenhain controllers
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
		local lines = { " " }
		if block.text then
			-- Use structure blocks prefixed with `*` to label the operation.
			-- I think these lines should not have a line number.
			lines[#lines + 1] = "* " .. block.text
		end
		return lines
	elseif block.type == "operation_end" then
		return { "" }
	elseif block.type == "tool_change" then
		return { "TOOL CALL " .. block.tool_number .. " Z S" .. block.spindle_speed }
	elseif block.type == "comment" then
		return { "; " .. block.text }
	elseif block.type == "stop" then
		return { "M0" }
	elseif block.type == "spindle_on" then
		-- Should be handled by rapid and retract moves based on block.state.spindle and M.spindle_state
		return {}
	elseif block.type == "spindle_off" then
		-- Should be handled by rapid and retract moves based on block.state.spindle and M.spindle_state
		return {}
	elseif block.type == "coolant_on" then
		-- Should be handled by rapid and retract moves based on block.state.coolant and M.coolant_state
		return {}
	elseif block.type == "coolant_off" then
		-- Should be handled by rapid and retract moves based on block.state.coolant and M.coolant_state
		return {}

	------------------------------------------------------------------------------
	-- Moves
	------------------------------------------------------------------------------
	elseif block.type == "retract" then
		return { "L " .. M.ax_coord("Z", block.height) .. " FMAX" .. M.spindle_postfix(block.state) }
	elseif block.type == "retract_full" then
		-- Retract in machine coordinates to the top of the z-axis
		return { "L Z+0 R0 FMAX M92" .. M.spindle_postfix(block.state) }
	elseif block.type == "home" then
		-- Retract in machine coordinates first, then home in the plane
		return { "L Z+0 R0 FMAX M92" .. M.spindle_postfix(block.state), "L X+0 Y+0 R0 FMAX M92" }
	elseif block.type == "rapid" then
		return { "L " .. M.format_coords(block) .. " FMAX" .. M.spindle_postfix(block.state) }

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
-- TODO: Also integrate cooling in this function. Coolant on M8, off M9, spindle cw & coolant on M13, ccw M14
function M.spindle_postfix(state)
	local postfix = ""
	-- Check if the stored spindlestate should be updated
	if state.spindle ~= M.spindle_state then
		if state.spindle == "cw" then
			postfix = " M3"
		elseif state.spindle == "ccw" then
			postfix = " M4"
		else
			postfix = " M5"
		end
		M.spindle_state = state.spindle
	end

	-- Check if the stored coolantstate should be updated
	if state.coolant ~= M.coolant_state then
		if state.coolant then
			-- Coolant ON
			if postfix == "" then
				postfix = " M8"
			elseif postfix == " M3" then
				postfix = " M13"
			elseif postfix == " M4" then
				postfix = " M14"
			end
		else
			-- Coolant OFF
			postfix = postfix .. " M9"
		end
		M.coolant_state = state.coolant
	end
	return postfix
end

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
