use ratatui::style::Color;

pub const COLOR_PRIMARY: Color = Color::Cyan;
pub const COLOR_PRIMARY_BRIGHT: Color = Color::LightCyan;
pub const COLOR_ACCENT: Color = Color::Magenta;
pub const COLOR_SUCCESS: Color = Color::Green;
pub const COLOR_SUCCESS_BRIGHT: Color = Color::LightGreen;
pub const COLOR_ERROR: Color = Color::Red;
pub const COLOR_ERROR_BRIGHT: Color = Color::LightRed;
pub const COLOR_WARNING: Color = Color::Yellow;
pub const COLOR_WARNING_BRIGHT: Color = Color::LightYellow;
pub const COLOR_MUTED: Color = Color::DarkGray;
pub const COLOR_HIGHLIGHT: Color = Color::Yellow;
pub const COLOR_BG_HIGHLIGHT: Color = Color::DarkGray;
pub const COLOR_BG_BANNER: Color = Color::Rgb(20, 20, 32);
pub const COLOR_BG_PANEL: Color = Color::Rgb(30, 30, 44);
/// Foreground for disabled rows (e.g. "gh CLI: coming soon").
pub const COLOR_DISABLED: Color = Color::DarkGray;
/// Background for the device-flow code block — slightly green-tinted
/// to make the bright code pop without high-contrast eye strain.
pub const COLOR_CODE_BG: Color = Color::Rgb(18, 32, 26);
/// Foreground for underlined links (the verification URL).
pub const COLOR_LINK: Color = Color::LightCyan;
