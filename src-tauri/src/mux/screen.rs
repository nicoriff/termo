//! The daemon's copy of a console's screen (see server.rs): everything the
//! console prints goes through a terminal emulator here too, so a window
//! that attaches later gets the screen as it is now — scrollback, what's on
//! screen, the cursor, the modes the program turned on — rather than a
//! replay of output whose beginning may have been dropped long ago.

/// Lines of scrollback kept per console: what a window that attaches gets
/// to scroll back through.
const SCROLLBACK: usize = 3000;

pub struct Screen {
    parser: vt100::Parser,
}

impl Screen {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self { parser: vt100::Parser::new(rows.max(1), cols.max(1), SCROLLBACK) }
    }

    pub fn process(&mut self, bytes: &[u8]) {
        self.parser.process(bytes);
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows.max(1), cols.max(1));
    }

    /// The answer to "where is the cursor" (`ESC [ 6 n`), for when no window
    /// is attached to answer it.
    pub fn cursor_report(&self) -> Vec<u8> {
        let (row, col) = self.parser.screen().cursor_position();
        format!("\x1b[{};{}R", row + 1, col + 1).into_bytes()
    }

    /// What turns a fresh terminal into this one: the scrollback, pushed up
    /// into the terminal's own, then the screen — the alternate one, when a
    /// full-screen program has it up —, the cursor and the input modes.
    pub fn snapshot(&mut self, title: Option<&str>) -> Vec<u8> {
        let mut out = b"\x1bc".to_vec();
        let screen = self.parser.screen_mut();
        let (rows, cols) = screen.size();

        // Oldest first, a screenful at a time: scrolled back by `offset`,
        // the screen's top rows are scrollback.
        screen.set_scrollback(usize::MAX);
        let total = screen.scrollback();
        let mut offset = total;
        while offset > 0 {
            screen.set_scrollback(offset);
            let take = offset.min(usize::from(rows));
            for row in screen.rows_formatted(0, cols).take(take) {
                out.extend_from_slice(&row);
                out.extend_from_slice(b"\x1b[m\r\n");
            }
            offset -= take;
        }
        screen.set_scrollback(0);
        if total > 0 {
            // Out of view, into the terminal's scrollback: from the bottom
            // row, one line feed per line still on screen — all of them but
            // the empty one the last line break left at the bottom.
            out.extend_from_slice(format!("\x1b[{rows};1H").as_bytes());
            out.extend(std::iter::repeat_n(b'\n', total.min(usize::from(rows).saturating_sub(1))));
        }

        let screen = self.parser.screen();
        if screen.alternate_screen() {
            out.extend_from_slice(b"\x1b[?1049h");
        }
        out.extend(screen.contents_formatted());
        out.extend(screen.input_mode_formatted());
        if let Some(title) = title {
            out.extend_from_slice(format!("\x1b]0;{title}\x07").as_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The test of a snapshot: a fresh terminal fed it looks the same.
    fn replayed(screen: &mut Screen, rows: u16, cols: u16) -> vt100::Parser {
        let mut fresh = vt100::Parser::new(rows, cols, SCROLLBACK);
        fresh.process(&screen.snapshot(None));
        fresh
    }

    fn scrollback_text(parser: &mut vt100::Parser) -> Vec<String> {
        let screen = parser.screen_mut();
        let (rows, cols) = screen.size();
        screen.set_scrollback(usize::MAX);
        let mut lines = Vec::new();
        let mut offset = screen.scrollback();
        while offset > 0 {
            screen.set_scrollback(offset);
            let take = offset.min(usize::from(rows));
            lines.extend(screen.rows(0, cols).take(take));
            offset -= take;
        }
        screen.set_scrollback(0);
        lines
    }

    #[test]
    fn the_screen_and_its_scrollback_come_back() {
        let mut screen = Screen::new(5, 20);
        for i in 0..12 {
            screen.process(format!("line {i}\r\n").as_bytes());
        }
        screen.process(b"\x1b[31mred\x1b[m prompt> ");

        let mut fresh = replayed(&mut screen, 5, 20);
        assert_eq!(fresh.screen().contents(), screen.parser.screen().contents());
        assert_eq!(fresh.screen().cursor_position(), screen.parser.screen().cursor_position());
        let original = scrollback_text(&mut screen.parser);
        assert_eq!(scrollback_text(&mut fresh), original);
        assert_eq!(original.first().map(String::as_str), Some("line 0"));
        // Colors too.
        let red = fresh.screen().cell(4, 0).unwrap();
        assert_eq!(red.fgcolor(), vt100::Color::Idx(1));
    }

    #[test]
    fn a_full_screen_program_comes_back_on_its_screen_with_its_modes() {
        let mut screen = Screen::new(4, 20);
        screen.process(b"$ vim\r\n\x1b[?1049h\x1b[?2004h\x1b[?1h\x1b[Hediting\x1b[2;3H");
        let fresh = replayed(&mut screen, 4, 20);
        assert!(fresh.screen().alternate_screen());
        assert!(fresh.screen().bracketed_paste());
        assert!(fresh.screen().application_cursor());
        assert_eq!(fresh.screen().cursor_position(), (1, 2));
        assert!(fresh.screen().contents().starts_with("editing"));
    }

    #[test]
    fn tells_where_the_cursor_is() {
        let mut screen = Screen::new(10, 40);
        screen.process(b"\x1b[3;7H");
        assert_eq!(screen.cursor_report(), b"\x1b[3;7R");
    }
}
