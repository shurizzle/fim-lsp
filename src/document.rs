use ropey::Rope;
use tower_lsp_server::ls_types::{Position, TextDocumentContentChangeEvent};

const PREFIX_CHARS: usize = 8192;
const SUFFIX_CHARS: usize = 2048;

#[derive(Debug)]
pub struct Document {
    pub rope: Rope,
    /// Bumped on every inline completion request, used to debounce stale ones.
    pub generation: u64,
}

impl Document {
    pub fn new(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            generation: 0,
        }
    }

    /// LSP position (UTF-16 code units) to char index, clamped to the line end.
    pub fn char_idx(&self, pos: Position) -> usize {
        let rope = &self.rope;
        let line = pos.line as usize;
        if line >= rope.len_lines() {
            return rope.len_chars();
        }
        let start = rope.line_to_char(line);
        let line_slice = rope.line(line);
        let mut end = start + line_slice.len_chars();
        // Exclude the line terminator.
        while end > start && matches!(rope.char(end - 1), '\n' | '\r') {
            end -= 1;
        }
        let start_cu = rope.char_to_utf16_cu(start);
        let end_cu = rope.char_to_utf16_cu(end);
        rope.utf16_cu_to_char((start_cu + pos.character as usize).min(end_cu))
    }

    pub fn apply(&mut self, change: TextDocumentContentChangeEvent) {
        match change.range {
            Some(range) => {
                let start = self.char_idx(range.start);
                let end = self.char_idx(range.end).max(start);
                self.rope.remove(start..end);
                self.rope.insert(start, &change.text);
            }
            None => self.rope = Rope::from_str(&change.text),
        }
    }

    /// Prefix/suffix windows around `cursor`, cut on line boundaries so the
    /// prefix start stays stable while typing (keeps cache keys stable).
    pub fn context(&self, cursor: usize) -> (String, String) {
        let rope = &self.rope;
        let cursor_line_start = rope.line_to_char(rope.char_to_line(cursor));

        let mut start = cursor.saturating_sub(PREFIX_CHARS);
        if start > 0 {
            let next_line = rope.char_to_line(start) + 1;
            start = rope.line_to_char(next_line).min(cursor_line_start);
        }

        let len = rope.len_chars();
        let mut end = (cursor + SUFFIX_CHARS).min(len);
        if end < len {
            end = rope.line_to_char(rope.char_to_line(end)).max(cursor);
        }

        (
            rope.slice(start..cursor).to_string(),
            rope.slice(cursor..end).to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp_server::ls_types::Range;

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn utf16_positions() {
        let doc = Document::new("a😀b\nxy\n");
        // '😀' is 2 UTF-16 code units, 1 char.
        assert_eq!(doc.char_idx(pos(0, 3)), 2);
        assert_eq!(doc.char_idx(pos(0, 99)), 3);
        assert_eq!(doc.char_idx(pos(1, 1)), 5);
        assert_eq!(doc.char_idx(pos(9, 0)), doc.rope.len_chars());
    }

    #[test]
    fn incremental_edit() {
        let mut doc = Document::new("héllo\nworld\n");
        doc.apply(TextDocumentContentChangeEvent {
            range: Some(Range::new(pos(0, 1), pos(1, 0))),
            range_length: None,
            text: "i ".into(),
        });
        assert_eq!(doc.rope.to_string(), "hi world\n");
    }

    #[test]
    fn context_split() {
        let doc = Document::new("fn main() {\n    \n}\n");
        let cursor = doc.char_idx(pos(1, 4));
        let (p, s) = doc.context(cursor);
        assert_eq!(p, "fn main() {\n    ");
        assert_eq!(s, "\n}\n");
    }
}
