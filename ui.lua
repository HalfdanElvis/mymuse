-- ui.lua
local function fmt_time(seconds)
	seconds = math.max(0, math.floor(seconds or 0))
	return string.format("%d:%02d", math.floor(seconds / 60), seconds % 60)
end

local CONFIG = {
	SCREEN_W = 320,
	SCREEN_H = 240,
	ART_PANEL_W = 144, -- (ART_SIZE 128 + ART_PAD 8 * 2)
	LIST_PAD_X = 6,
	ITEM_FONT = 10,
	NOWPLAY_TITLE_FONT = 10,
	LIST_SCROLL_GAP = 30,
	NOWPLAY_SCROLL_GAP = 20,
	VISIBLE_ROWS = 7,
}
CONFIG.LIST_W = CONFIG.SCREEN_W - CONFIG.ART_PANEL_W
CONFIG.LIST_ITEM_MAX_WIDTH = CONFIG.LIST_W - (CONFIG.LIST_PAD_X * 2)
-- 320 (screen) - 6 (pad) - 32 (art) - 6 (pad) - 42 (approx time text width)
CONFIG.NOWPLAY_MAX_WIDTH = CONFIG.SCREEN_W - 6 - 32 - 24

function get_layout_config()
	return CONFIG
end

function draw_screen(state)
	local cmds = {}

	-- ===== Screen =====
	local SCREEN_W = CONFIG.SCREEN_W
	local SCREEN_H = CONFIG.SCREEN_H

	-- ===== Colors =====
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
	local COLOR_PROGRESS_FG = 0xFF66AAFF
	local COLOR_BATTERY = 0xFF88CC66

	-- Placeholder cover colors, cycled by selection until real art exists.
	local ART_PLACEHOLDER_COLORS = {
		0xFF6A4C93,
		0xFF1982C4,
		0xFF8AC926,
		0xFFFF924C,
		0xFFFF595E,
		0xFF2EC4B6,
	}

	-- ===== Layout bands =====
	local STATUS_H = 16
	local HEADER_H = 22
	local LIST_GAP = 4
	local NOWPLAY_H = 44

	local CONTENT_TOP = STATUS_H + HEADER_H + LIST_GAP
	local CONTENT_BOTTOM = SCREEN_H - NOWPLAY_H
	local CONTENT_H = CONTENT_BOTTOM - CONTENT_TOP

	-- ===== Status bar =====
	local STATUS_PAD_X = 6
	local STATUS_FONT = 9
	local STATUS_SLOT_W = 16
	local BAT_W, BAT_H = 16, 8
	local STATUS_TEXT_Y = math.floor((STATUS_H - STATUS_FONT) / 2)

	-- ===== Header =====
	local HEADER_PAD_X = 10
	local HEADER_FONT = 12
	local HEADER_TEXT_PAD_Y = math.floor((HEADER_H - HEADER_FONT) / 2)

	-- ===== Right album-art panel =====
	local ART_SIZE = 128
	local ART_PAD = 8
	local ART_PANEL_W = ART_SIZE + ART_PAD * 2

	-- ===== List (Left pane) =====
	local LIST_X = 0
	local LIST_W = SCREEN_W - ART_PANEL_W
	local LIST_PAD_X = CONFIG.LIST_PAD_X
	local ITEM_FONT = CONFIG.ITEM_FONT
	local ITEM_VPAD = 4
	local ITEM_LINE_H = ITEM_FONT * (state.line_height_ratio or 1.0)
	local ITEM_H = ITEM_LINE_H + ITEM_VPAD * 2
	local ITEM_TEXT_PAD_Y = ITEM_VPAD

	-- ===== Now playing =====
	local NOWPLAY_PAD = 6
	local NOWPLAY_ART = 32
	local NOWPLAY_TITLE_FONT = CONFIG.NOWPLAY_TITLE_FONT
	local NOWPLAY_ARTIST_FONT = 8
	local PROGRESS_H = 2

	-- FADE
	local FADE_HEIGHT = 12
	local FADE_START_COLOR = 0x00181818
	local FADE_END_COLOR = 0xFF181818

	local selected = state.selected_index or 1

	-- Background
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

	-- Right-aligned cluster: [BT] [HP] [battery]
	local right = SCREEN_W - STATUS_PAD_X
	local bat_x = right - BAT_W - 2
	local bat_y = math.floor((STATUS_H - BAT_H) / 2)

	-- Battery: body outline, interior, fill, nub
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

	-- ===== 2. Header =====
	local header_y = STATUS_H
	table.insert(
		cmds,
		{ type = "rect", x = 0, y = header_y, width = SCREEN_W, height = HEADER_H, color = COLOR_HEADER_BG }
	)
	table.insert(cmds, {
		type = "text",
		x = HEADER_PAD_X,
		y = header_y + HEADER_TEXT_PAD_Y,
		text = state.title or "MyMuse",
		color = COLOR_HEADER_TEXT,
		size = HEADER_FONT,
	})

	-- ===== 3. Right album-art panel (art of the selected item) =====
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
	table.insert(cmds, { type = "rect", x = art_x, y = art_y, width = ART_SIZE, height = ART_SIZE, color = art_color })
	-- TODO: replace the rect above with a real image command once Rust can decode album art.

	-- ===== 4. List (left pane, with scrolling) =====
	local n = #state.items
	local fully_visible_rows = math.max(1, math.floor(CONTENT_H / ITEM_H))

	local first = state.scroll_offset + 1

	-- Draw one extra row to enable peeking of next item
	local last = math.min(n, first + CONFIG.VISIBLE_ROWS)

	for i = first, last do
		local y = CONTENT_TOP + (i - first) * ITEM_H

		-- Marquee scroll state for the selected item
		local scroll_x = 0
		local text_w = 0
		if i == selected and state.list_scroll then
			scroll_x = state.list_scroll[1]
			text_w = state.list_scroll[2]
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

		-- Second copy for looping (disabled if reversing/ping-ponging)
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

	-- gradient fade to obscure peeking track from list

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

		-- Album art thumbnail (placeholder until real images exist)
		local th_x = NOWPLAY_PAD
		local th_y = np_y + math.floor((NOWPLAY_H - PROGRESS_H - NOWPLAY_ART) / 2)
		table.insert(
			cmds,
			{ type = "rect", x = th_x, y = th_y, width = NOWPLAY_ART, height = NOWPLAY_ART, color = COLOR_ART_FRAME }
		)

		-- Title / artist
		local text_x = th_x + NOWPLAY_ART + NOWPLAY_PAD
		local title_y = np_y + NOWPLAY_PAD

		-- Marquee scroll state for the now playing title
		local now_scroll_x = 0
		local now_text_w = 0
		if state.now_scroll then
			now_scroll_x = state.now_scroll[1]
			now_text_w = state.now_scroll[2]
		end

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

		-- Second copy for looping (disabled if reversing/ping-ponging)
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

		-- Artist (no scroll yet, usually fits)
		table.insert(cmds, {
			type = "text",
			x = text_x,
			y = title_y + NOWPLAY_TITLE_FONT + 3,
			text = now.artist or "",
			color = COLOR_NOWPLAY_DIM,
			size = NOWPLAY_ARTIST_FONT,
		})

		-- Elapsed / duration, right-aligned
		local time_str = fmt_time(now.elapsed) .. " / " .. fmt_time(now.duration)
		local approx_w = #time_str * 5
		table.insert(cmds, {
			type = "text",
			x = SCREEN_W - NOWPLAY_PAD - approx_w,
			y = title_y + NOWPLAY_TITLE_FONT + 3,
			text = time_str,
			color = COLOR_NOWPLAY_DIM,
			size = NOWPLAY_ARTIST_FONT,
		})

		-- Progress bar along the very bottom
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
