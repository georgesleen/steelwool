//! Turning byte offsets carried by an error into a line and column.

/// A one based position in a source file, the column counted in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// Maps a byte offset onto its position. An offset past the end of `source`
/// clamps to the position just after the last character, and an offset inside
/// a character clamps to that character's start.
pub fn position(source: &str, offset: u32) -> Position {
    let mut offset = (offset as usize).min(source.len());
    while !source.is_char_boundary(offset) {
        offset -= 1;
    }
    let mut line = 1;
    let mut line_start = 0;
    for (index, byte) in source.as_bytes()[..offset].iter().enumerate() {
        if *byte == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }
    let column = source[line_start..offset].chars().count() + 1;
    Position { line, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_byte_is_line_one_column_one() {
        assert_eq!(position("(a)\n", 0), Position { line: 1, column: 1 });
    }

    #[test]
    fn a_newline_starts_the_next_line() {
        let source = "(a)\n(b)\n";
        assert_eq!(position(source, 4), Position { line: 2, column: 1 });
        assert_eq!(position(source, 6), Position { line: 2, column: 3 });
    }

    #[test]
    fn the_column_counts_characters_not_bytes() {
        let source = "(a)\n(naïve x)\n";
        let offset = source.find('x').expect("the source contains x") as u32;
        assert_eq!(offset, 12);
        let at = position(source, offset);
        assert_eq!(at, Position { line: 2, column: 8 });
    }

    #[test]
    fn an_offset_at_or_past_the_end_clamps() {
        let source = "(a)\n(b)";
        assert_eq!(position(source, 7), Position { line: 2, column: 4 });
        assert_eq!(position(source, 900), Position { line: 2, column: 4 });
    }

    #[test]
    fn an_offset_inside_a_character_clamps_to_its_start() {
        let source = "ï)";
        assert_eq!(position(source, 1), Position { line: 1, column: 1 });
    }
}
