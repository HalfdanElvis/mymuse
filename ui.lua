-- ui.lua

-- Scroll tuning (previously Rust consts; now live here since Lua owns
-- the scroll math).
local SCROLL_START_TIME = 1.0
local SCROLL_SPEED = 30.0
local MIN_LOOP_DISTANCE = 20.0

local function fmt_time(seconds)
	seconds = math.max(0, math.floor(seconds or 0))
	return string.format("%d:%02d", math.floor(seconds / 60), seconds % 60)
end

-- Compute (offset, text_width) for a scrolling label.
--   elapsed  = seconds since the scroll timer was (re)started
--   pingpong = bounce back and forth instead of looping
-- Mirrors what `scroll_state` + `get_textloop_offset` used to do in Rust.
local function scroll_offset(text, font_size, max_width, gap, elapsed, pingpong)
	if not elapsed or elapsed <= SCROLL_START_TIME then
		return 0.0, 0.0
	end
	local scroll_time = elapsed - SCROLL_START_TIME
	local text_w = measure_text(text, font_size)
	if text_w <= max_width then
		return 0.0, text_w
	end
	local loop_dist
	if pingpong then
		loop_dist = text_w - max_width
	else
		loop_dist = text_w + gap
	end
	local pause_mult = 1.0
	if pingpong and loop_dist < MIN_LOOP_DISTANCE then
		pause_mult = 1.0 + (1.0 - loop_dist / MIN_LOOP_DISTANCE) * 2.0
	end
	local dynamic_pause = SCROLL_START_TIME * pause_mult
	local offset
	if pingpong then
		local travel_time = loop_dist / SCROLL_SPEED
		local cycle = (travel_time + dynamic_pause) * 2.0
		local t = scroll_time % cycle
		if t < travel_time then
			offset = t * SCROLL_SPEED
		elseif t < travel_time + dynamic_pause then
			offset = loop_dist
		elseif t < travel_time * 2.0 + dynamic_pause then
			offset = loop_dist - ((t - travel_time - dynamic_pause) * SCROLL_SPEED)
		else
			offset = 0.0
		end
	else
		offset = (scroll_time * SCROLL_SPEED) % loop_dist
	end
	return offset, text_w
end

local CONFIG = {
	SCREEN_W = 320,
	SCREEN_H = 240,
	ART_PANEL_W = 144,
	LIST_PAD_X = 6,
	HEADER_PAD_X = 10,
	ITEM_FONT = 10,
	HEADER_FONT = 12,
	NOWPLAY_TITLE_FONT = 10,
	NOWPLAY_ARTIST_FONT = 8,
	LIST_SCROLL_GAP = 30,
	NOWPLAY_SCROLL_GAP = 20,
	HEADER_SCROLL_GAP = 30,
	ARTIST_SCROLL_GAP = 20,
	VISIBLE_ROWS = 7,
}
CONFIG.LIST_W = CONFIG.SCREEN_W - CONFIG.ART_PANEL_W
CONFIG.LIST_ITEM_MAX_WIDTH = CONFIG.LIST_W - (CONFIG.LIST_PAD_X * 2)
CONFIG.NOWPLAY_MAX_WIDTH = 240
CONFIG.HEADER_MAX_WIDTH = CONFIG.SCREEN_W - (CONFIG.HEADER_PAD_X * 2)
CONFIG.NOWPLAY_ARTIST_MAX_WIDTH = 200

function get_layout_config()
	return CONFIG
end

function draw_screen(state)
	local cmds = {}

	local SCREEN_W = CONFIG.SCREEN_W
	local SCREEN_H = CONFIG.SCREEN_H

	local COLOR_BG = 0xFF222222
	local COLOR_STATUS_BG = 0xFF111111
	local COLOR_STATUS_TEXT = 0xFFAAAAAA
	local COLOR_STATUS_OFF = 0xFF555555
	local COLOR_HEADER_BG = 0xFF555555
	local COLOR_HEADER_TEXT = 0xFFFFFFFF
	local COLOR_ITEM_TEXT = 0xFFFFFFFF
	local COLOR_SELECTED = 0xFF444444
	local COLOR_ART_BG = 0xFF2A2A2A
	local COLOR_ART_FRAME = 0xFF3A3A3A
	local COLOR_NOWPLAY_BG = 0xFF181818
	local COLOR_NOWPLAY_TEXT = 0xFFFFFFFF
	local COLOR_NOWPLAY_DIM = 0xFF999999
	local COLOR_PROGRESS_BG = 0xFF333333
	local COLOR_PROGRESS_FG = 0xFFFFFFFF
	local COLOR_BATTERY = 0xFF88CC66

	local ART_PLACEHOLDER_COLORS = {
		0xFF6A4C93,
		0xFF1982C4,
		0xFF8AC926,
		0xFFFF924C,
		0xFFFF595E,
		0xFF2EC4B6,
	}

	local STATUS_H = 16
	local HEADER_H = 22
	local LIST_GAP = 4
	local NOWPLAY_H = 44

	local CONTENT_TOP = STATUS_H + HEADER_H + LIST_GAP
	local CONTENT_BOTTOM = SCREEN_H - NOWPLAY_H
	local CONTENT_H = CONTENT_BOTTOM - CONTENT_TOP

	local STATUS_PAD_X = 6
	local STATUS_FONT = 9
	local STATUS_SLOT_W = 16
	local BAT_W, BAT_H = 16, 8
	local STATUS_TEXT_Y = math.floor((STATUS_H - STATUS_FONT) / 2)

	local HEADER_PAD_X = CONFIG.HEADER_PAD_X
	local HEADER_FONT = CONFIG.HEADER_FONT
	local HEADER_TEXT_PAD_Y = math.floor((HEADER_H - HEADER_FONT) / 2)

	local ART_SIZE = 128
	local ART_PAD = 8
	local ART_PANEL_W = ART_SIZE + ART_PAD * 2

	local LIST_X = 0
	local LIST_W = SCREEN_W - ART_PANEL_W
	local LIST_PAD_X = CONFIG.LIST_PAD_X
	local ITEM_FONT = CONFIG.ITEM_FONT
	local ITEM_VPAD = 4
	local ITEM_LINE_H = ITEM_FONT * (state.line_height_ratio or 1.0)
	local ITEM_H = ITEM_LINE_H + ITEM_VPAD * 2
	local ITEM_TEXT_PAD_Y = ITEM_VPAD

	local NOWPLAY_PAD = 6
	local NOWPLAY_ART = 32
	local NOWPLAY_TITLE_FONT = CONFIG.NOWPLAY_TITLE_FONT
	local NOWPLAY_ARTIST_FONT = CONFIG.NOWPLAY_ARTIST_FONT
	local PROGRESS_H = 4

	local FADE_HEIGHT = 12
	local FADE_START_COLOR = 0x00181818
	local FADE_END_COLOR = 0xFF181818

	local selected = state.selected_index or 1

	table.insert(cmds, { type = "rect", x = 0, y = 0, width = SCREEN_W, height = SCREEN_H, color = COLOR_BG })

	-- ===== 1. Status bar =====
	table.insert(cmds, { type = "rect", x = 0, y = 0, width = SCREEN_W, height = STATUS_H, color = COLOR_STATUS_BG })

	table.insert(cmds, {
		type = "text",
		x = STATUS_PAD_X,
		y = STATUS_TEXT_Y,
		text = state.clock or "--:--",
		color = COLOR_STATUS_TEXT,
		size = STATUS_FONT,
	})

	local right = SCREEN_W - STATUS_PAD_X
	local bat_x = right - BAT_W - 2
	local bat_y = math.floor((STATUS_H - BAT_H) / 2)

	table.insert(
		cmds,
		{ type = "rect", x = bat_x, y = bat_y, width = BAT_W, height = BAT_H, color = COLOR_STATUS_TEXT }
	)
	table.insert(
		cmds,
		{ type = "rect", x = bat_x + 1, y = bat_y + 1, width = BAT_W - 2, height = BAT_H - 2, color = COLOR_STATUS_BG }
	)
	local pct = math.max(0, math.min(1, state.battery or 0))
	local fill_w = math.floor((BAT_W - 2) * pct)
	if fill_w > 0 then
		table.insert(
			cmds,
			{ type = "rect", x = bat_x + 1, y = bat_y + 1, width = fill_w, height = BAT_H - 2, color = COLOR_BATTERY }
		)
	end
	table.insert(
		cmds,
		{ type = "rect", x = bat_x + BAT_W, y = bat_y + 2, width = 2, height = BAT_H - 4, color = COLOR_STATUS_TEXT }
	)

	local bt_x = bat_x - 2 - STATUS_SLOT_W
	local hp_x = bt_x - STATUS_SLOT_W
	table.insert(cmds, {
		type = "text",
		x = bt_x,
		y = STATUS_TEXT_Y,
		text = "BT",
		color = state.bluetooth and COLOR_STATUS_TEXT or COLOR_STATUS_OFF,
		size = STATUS_FONT,
	})
	table.insert(cmds, {
		type = "text",
		x = hp_x,
		y = STATUS_TEXT_Y,
		text = "HP",
		color = state.headphones and COLOR_STATUS_TEXT or COLOR_STATUS_OFF,
		size = STATUS_FONT,
	})

	-- ===== 2. Header (with scrolling) =====
	local header_y = STATUS_H
	table.insert(
		cmds,
		{ type = "rect", x = 0, y = header_y, width = SCREEN_W, height = HEADER_H, color = COLOR_HEADER_BG }
	)

	local hdr_scroll_x, hdr_text_w = scroll_offset(
		state.title or "MyMuse",
		HEADER_FONT,
		CONFIG.HEADER_MAX_WIDTH,
		CONFIG.HEADER_SCROLL_GAP,
		state.header_scroll_elapsed,
		state.is_reversing
	)

	local hdr_draw_x = HEADER_PAD_X - hdr_scroll_x
	local hdr_clip_left = HEADER_PAD_X
	local hdr_clip_right = HEADER_PAD_X + CONFIG.HEADER_MAX_WIDTH
	table.insert(cmds, {
		type = "text",
		x = hdr_draw_x,
		y = header_y + HEADER_TEXT_PAD_Y,
		text = state.title or "MyMuse",
		color = COLOR_HEADER_TEXT,
		size = HEADER_FONT,
		clip_left = hdr_clip_left,
		clip_right = hdr_clip_right,
	})
	if hdr_scroll_x > 0 and not state.is_reversing then
		table.insert(cmds, {
			type = "text",
			x = hdr_draw_x + hdr_text_w + CONFIG.HEADER_SCROLL_GAP,
			y = header_y + HEADER_TEXT_PAD_Y,
			text = state.title or "MyMuse",
			color = COLOR_HEADER_TEXT,
			size = HEADER_FONT,
			clip_left = hdr_clip_left,
			clip_right = hdr_clip_right,
		})
	end

	-- ===== 3. Right album-art panel =====
	local art_x = SCREEN_W - ART_PANEL_W + ART_PAD
	local art_y = CONTENT_TOP + math.floor((CONTENT_H - ART_SIZE) / 2)
	local art_color = ART_PLACEHOLDER_COLORS[((selected - 1) % #ART_PLACEHOLDER_COLORS) + 1]

	table.insert(cmds, {
		type = "rect",
		x = art_x - 2,
		y = art_y - 2,
		width = ART_SIZE + 4,
		height = ART_SIZE + 4,
		color = COLOR_ART_FRAME,
	})
	if state.has_pane_art_prev then
		table.insert(cmds, {
			type = "album_art_prev",
			x = art_x,
			y = art_y,
			width = ART_SIZE,
			height = ART_SIZE,
		})
	end
	if state.has_pane_art then
		table.insert(cmds, {
			type = "album_art",
			x = art_x,
			y = art_y,
			width = ART_SIZE,
			height = ART_SIZE,
		})
	elseif not state.has_pane_art_prev then
		table.insert(cmds, {
			type = "rect",
			x = art_x,
			y = art_y,
			width = ART_SIZE,
			height = ART_SIZE,
			color = art_color,
		})
	end

	-- ===== 4. List =====
	local n = #state.items
	local first = state.scroll_offset + 1
	local last = math.min(n, first + CONFIG.VISIBLE_ROWS)

	for i = first, last do
		local y = CONTENT_TOP + (i - first) * ITEM_H

		local scroll_x, text_w = 0.0, 0.0
		if i == selected then
			scroll_x, text_w = scroll_offset(
				state.items[i],
				ITEM_FONT,
				CONFIG.LIST_ITEM_MAX_WIDTH,
				CONFIG.LIST_SCROLL_GAP,
				state.list_scroll_elapsed,
				state.is_reversing
			)
		end

		if i == selected then
			table.insert(
				cmds,
				{ type = "rect", x = LIST_X, y = y, width = LIST_W, height = ITEM_H, color = COLOR_SELECTED }
			)
		end

		local draw_x = LIST_X + LIST_PAD_X - scroll_x
		table.insert(cmds, {
			type = "text",
			x = draw_x,
			y = y + ITEM_TEXT_PAD_Y,
			text = state.items[i],
			color = COLOR_ITEM_TEXT,
			size = ITEM_FONT,
			clip_left = LIST_X + LIST_PAD_X,
			clip_right = LIST_X + LIST_PAD_X + CONFIG.LIST_ITEM_MAX_WIDTH,
		})

		if scroll_x > 0 and not state.is_reversing then
			table.insert(cmds, {
				type = "text",
				x = draw_x + text_w + CONFIG.LIST_SCROLL_GAP,
				y = y + ITEM_TEXT_PAD_Y,
				text = state.items[i],
				color = COLOR_ITEM_TEXT,
				size = ITEM_FONT,
				clip_left = LIST_X + LIST_PAD_X,
				clip_right = LIST_X + LIST_PAD_X + CONFIG.LIST_ITEM_MAX_WIDTH,
			})
		end
	end

	-- ===== 5. Now-playing bar =====
	local np_y = CONTENT_BOTTOM

	table.insert(cmds, {
		type = "gradient_rect",
		x = 0,
		y = np_y - FADE_HEIGHT,
		width = SCREEN_W,
		height = FADE_HEIGHT,
		color_start = FADE_START_COLOR,
		color_end = FADE_END_COLOR,
	})

	table.insert(
		cmds,
		{ type = "rect", x = 0, y = np_y, width = SCREEN_W, height = NOWPLAY_H, color = COLOR_NOWPLAY_BG }
	)

	if state.now then
		local now = state.now

		local th_x = NOWPLAY_PAD
		local th_y = np_y + math.floor((NOWPLAY_H - PROGRESS_H - NOWPLAY_ART) / 2)
		if state.has_now_art then
			table.insert(cmds, {
				type = "now_art_thumb",
				x = th_x,
				y = th_y,
				width = NOWPLAY_ART,
				height = NOWPLAY_ART,
			})
		else
			table.insert(cmds, {
				type = "rect",
				x = th_x,
				y = th_y,
				width = NOWPLAY_ART,
				height = NOWPLAY_ART,
				color = COLOR_ART_FRAME,
			})
		end

		local text_x = th_x + NOWPLAY_ART + NOWPLAY_PAD
		local title_y = np_y + NOWPLAY_PAD

		-- Title scroll
		local now_scroll_x, now_text_w = scroll_offset(
			now.title or "",
			NOWPLAY_TITLE_FONT,
			CONFIG.NOWPLAY_MAX_WIDTH,
			CONFIG.NOWPLAY_SCROLL_GAP,
			state.now_scroll_elapsed,
			state.is_reversing
		)

		local draw_title_x = text_x - now_scroll_x
		table.insert(cmds, {
			type = "text",
			x = draw_title_x,
			y = title_y,
			text = now.title or "",
			color = COLOR_NOWPLAY_TEXT,
			size = NOWPLAY_TITLE_FONT,
			clip_left = text_x,
			clip_right = text_x + CONFIG.NOWPLAY_MAX_WIDTH,
		})
		if now_scroll_x > 0 and not state.is_reversing then
			table.insert(cmds, {
				type = "text",
				x = draw_title_x + now_text_w + CONFIG.NOWPLAY_SCROLL_GAP,
				y = title_y,
				text = now.title or "",
				color = COLOR_NOWPLAY_TEXT,
				size = NOWPLAY_TITLE_FONT,
				clip_left = text_x,
				clip_right = text_x + CONFIG.NOWPLAY_MAX_WIDTH,
			})
		end

		-- Artist scroll
		local artist_scroll_x, artist_text_w = scroll_offset(
			now.artist or "",
			NOWPLAY_ARTIST_FONT,
			CONFIG.NOWPLAY_ARTIST_MAX_WIDTH,
			CONFIG.ARTIST_SCROLL_GAP,
			state.artist_scroll_elapsed,
			state.is_reversing
		)

		local artist_y = title_y + NOWPLAY_TITLE_FONT + 3
		local draw_artist_x = text_x - artist_scroll_x
		local artist_clip_left = text_x
		local artist_clip_right = text_x + CONFIG.NOWPLAY_ARTIST_MAX_WIDTH

		table.insert(cmds, {
			type = "text",
			x = draw_artist_x,
			y = artist_y,
			text = now.artist or "",
			color = COLOR_NOWPLAY_DIM,
			size = NOWPLAY_ARTIST_FONT,
			clip_left = artist_clip_left,
			clip_right = artist_clip_right,
		})
		if artist_scroll_x > 0 and not state.is_reversing then
			table.insert(cmds, {
				type = "text",
				x = draw_artist_x + artist_text_w + CONFIG.ARTIST_SCROLL_GAP,
				y = artist_y,
				text = now.artist or "",
				color = COLOR_NOWPLAY_DIM,
				size = NOWPLAY_ARTIST_FONT,
				clip_left = artist_clip_left,
				clip_right = artist_clip_right,
			})
		end

		-- Timer + play/pause icon
		local time_str = fmt_time(now.elapsed) .. " / " .. fmt_time(now.duration)
		local approx_w = #time_str * 5
		local timer_x = SCREEN_W - NOWPLAY_PAD - approx_w
		local time_w = measure_text(time_str, NOWPLAY_ARTIST_FONT)
		local ICON_RIGHT = timer_x + time_w

		local STRIPE_W = 2
		local STRIPE_H = 8
		local STRIPE_GAP = 3
		local ICON_RADIUS = 0.8
		local TRI_W = 8
		local TRI_H = 8

		if state.is_paused then
			table.insert(cmds, {
				type = "triangle",
				x = ICON_RIGHT - TRI_W,
				y = title_y,
				width = TRI_W,
				height = TRI_H,
				radius = 1.0,
				color = COLOR_NOWPLAY_DIM,
			})
		else
			local total_w = STRIPE_W * 2 + STRIPE_GAP
			local stripe_x = ICON_RIGHT - total_w
			table.insert(cmds, {
				type = "rect",
				x = stripe_x,
				y = title_y,
				width = STRIPE_W,
				height = STRIPE_H,
				radius = ICON_RADIUS,
				color = COLOR_NOWPLAY_DIM,
			})
			table.insert(cmds, {
				type = "rect",
				x = stripe_x + STRIPE_W + STRIPE_GAP,
				y = title_y,
				width = STRIPE_W,
				height = STRIPE_H,
				radius = ICON_RADIUS,
				color = COLOR_NOWPLAY_DIM,
			})
		end

		table.insert(cmds, {
			type = "text",
			x = timer_x,
			y = artist_y,
			text = time_str,
			color = COLOR_NOWPLAY_DIM,
			size = NOWPLAY_ARTIST_FONT,
		})

		local bar_y = np_y + NOWPLAY_H - PROGRESS_H
		table.insert(
			cmds,
			{ type = "rect", x = 0, y = bar_y, width = SCREEN_W, height = PROGRESS_H, color = COLOR_PROGRESS_BG }
		)
		local ratio = 0
		if (now.duration or 0) > 0 then
			ratio = math.max(0, math.min(1, (now.elapsed or 0) / now.duration))
		end
		local bar_w = math.floor(SCREEN_W * ratio)
		if bar_w > 0 then
			table.insert(
				cmds,
				{ type = "rect", x = 0, y = bar_y, width = bar_w, height = PROGRESS_H, color = COLOR_PROGRESS_FG }
			)
		end
	end

	return cmds
end
