pub fn clear_screen() -> &'static str { "\x1b[2J" }
pub fn reset_cursor() -> &'static str { "\x1b[H" }
pub fn disable_local_echo() -> [u8; 3] {
    /* IAC WILL  ECHO */
    [0xff, 0xfb, 0x01]
}
pub fn disable_linemode() -> [u8; 10] {
    [
    /*  IAC   DO    LM */
        0xff, 0xfd, 0x22,
    /*  IAC   SB    LM    MODE  mask  IAC   SE */
        0xff, 0xfa, 0x22, 0x01, 0x00, 0xff, 0xf0
    ]
}
