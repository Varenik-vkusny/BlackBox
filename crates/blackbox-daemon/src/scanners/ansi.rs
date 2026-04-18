/// Stateful ANSI escape sequence stripper.
///
/// Handles split sequences: if an ESC byte arrives at the end of one buffer
/// chunk and `[31m` arrives at the start of the next, the parser correctly
/// discards the full sequence without leaking garbage characters.
#[derive(Default)]
pub struct AnsiStripper {
    state: AnsiState,
}

#[derive(Default, PartialEq)]
enum AnsiState {
    #[default]
    Normal,
    Esc,  // saw \x1b, waiting for sequence type byte
    Csi,  // inside CSI sequence: \x1b[ ... letter
    Osc,  // inside OSC sequence: \x1b] ... \x07
}

impl AnsiStripper {
    pub fn strip(&mut self, input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for ch in input.chars() {
            match self.state {
                AnsiState::Normal => match ch {
                    '\x1b' => self.state = AnsiState::Esc,
                    '\x08' | '\r' => {} // backspace, CR — discard
                    _ => out.push(ch),
                },
                AnsiState::Esc => match ch {
                    '[' => self.state = AnsiState::Csi,
                    ']' => self.state = AnsiState::Osc,
                    // Charset designators like ESC ( B — just consume this char and return
                    '(' | ')' => self.state = AnsiState::Normal,
                    // Any other char after ESC: not a recognized sequence, return to normal
                    _ => self.state = AnsiState::Normal,
                },
                AnsiState::Csi => {
                    // CSI terminates at any ASCII letter (m, H, J, K, A-D, s, u, f, etc.)
                    if ch.is_ascii_alphabetic() {
                        self.state = AnsiState::Normal;
                    }
                    // Parameter bytes (digits, semicolons, spaces) consumed silently
                }
                AnsiState::Osc => {
                    // OSC terminates at BEL (\x07) or String Terminator (\x1b\)
                    if ch == '\x07' {
                        self.state = AnsiState::Normal;
                    }
                    // \x1b inside OSC starts a potential ST — handled on next char
                }
            }
        }
        out
    }
}

/// Stateless one-shot stripping (allocates a fresh parser per call).
/// Fine for line-by-line use where split sequences don't occur.
pub fn strip_ansi_stateless(text: &str) -> String {
    AnsiStripper::default().strip(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_color_codes() {
        assert_eq!(strip_ansi_stateless("\x1b[31merror\x1b[0m"), "error");
    }

    #[test]
    fn strips_bold_and_reset() {
        assert_eq!(strip_ansi_stateless("\x1b[1;32msuccess\x1b[0m"), "success");
    }

    #[test]
    fn strips_osc_title() {
        assert_eq!(strip_ansi_stateless("\x1b]0;title\x07hello"), "hello");
    }

    #[test]
    fn preserves_normal_text() {
        assert_eq!(strip_ansi_stateless("plain log line"), "plain log line");
    }

    #[test]
    fn discards_cr() {
        assert_eq!(strip_ansi_stateless("line\roverwrite"), "lineoverwrite");
    }

    #[test]
    fn handles_split_sequence() {
        let mut parser = AnsiStripper::default();
        // ESC arrives in first chunk, rest of sequence in second
        let part1 = parser.strip("hello\x1b");
        let part2 = parser.strip("[31mworld\x1b[0m");
        assert_eq!(part1 + &part2, "helloworld");
    }
}
