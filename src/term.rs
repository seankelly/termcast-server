
pub fn clear_screen() -> &'static str { "\x1b[2J" }
pub fn reset_cursor() -> &'static str { "\x1b[H" }
pub fn disable_local_echo() -> [u8; 3] { [0xff, 0xfb, 0x01] }
