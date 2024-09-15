local function turn_off_gui_colors()
	vim.cmd("set notermguicolors")
end

return {
	-- theme = "PaperColor",
	-- ui = "light",
	-- font = {
	-- 	family = "Nova Nerd Font",
	theme = "aylin",
	ui = "dark",
	font = {
		family = "Maple Mono",
		size = 15,
		fallbacks = { "Mononoki Nerd Font" },
	},
	line_spacing = 3,
	-- lua = {
	-- 	turn_off_gui_colors,
	-- },
}
