use crate::rule::fe::css::parse::is_css_whitespace;

pub(crate) enum Bounds {
    Statement {
        end: usize,
    },
    Block {
        brace: usize,
        inner_end: usize,
        end: usize,
    },
}

pub(crate) fn bounds(source: &str, start: usize, limit: usize) -> Bounds {
    let mut cursor = Cursor::new(source, start, limit);
    let mut paren = 0u32;
    let mut bracket = 0u32;
    while !cursor.eof() {
        let byte = cursor.byte();
        if byte == b'\\' {
            cursor.skip_escape();
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            cursor.consume_string(byte);
            continue;
        }
        if cursor.starts_comment() {
            cursor.consume_comment();
            continue;
        }
        if paren == 0 && bracket == 0 {
            if byte == b';' {
                cursor.bump();
                return Bounds::Statement { end: cursor.index };
            }
            if byte == b'{' {
                let brace = cursor.index;
                cursor.bump();
                let (inner_end, end) = cursor.matching_close();
                return Bounds::Block {
                    brace,
                    inner_end,
                    end,
                };
            }
            if byte == b'}' {
                return Bounds::Statement { end: cursor.index };
            }
        }
        match byte {
            b'(' => paren += 1,
            b')' => paren = paren.saturating_sub(1),
            b'[' => bracket += 1,
            b']' => bracket = bracket.saturating_sub(1),
            _ => {}
        }
        cursor.bump();
    }
    Bounds::Statement { end: cursor.index }
}

struct Cursor<'a> {
    source: &'a str,
    index: usize,
    end: usize,
}

impl<'a> Cursor<'a> {
    fn new(source: &'a str, index: usize, end: usize) -> Self {
        Self {
            source,
            index,
            end: end.min(source.len()),
        }
    }

    fn eof(&self) -> bool {
        self.index >= self.end
    }

    fn byte(&self) -> u8 {
        self.source.as_bytes()[self.index]
    }

    fn bump(&mut self) {
        if self.eof() {
            return;
        }
        self.index += char_len(self.source, self.index);
    }

    fn starts_comment(&self) -> bool {
        starts_comment(self.source, self.index) && self.index + 1 < self.end
    }

    fn consume_comment(&mut self) {
        self.index = comment_end(self.source, self.index, self.end);
    }

    fn consume_string(&mut self, quote: u8) {
        self.bump();
        while !self.eof() {
            let byte = self.byte();
            if byte == b'\\' {
                self.skip_escape();
                continue;
            }
            if byte == quote {
                self.bump();
                return;
            }
            if byte == b'\n' || byte == b'\r' {
                return;
            }
            self.bump();
        }
    }

    fn skip_escape(&mut self) {
        self.bump();
        if self.eof() {
            return;
        }
        if self.source[self.index..].starts_with("\r\n") {
            self.index = (self.index + 2).min(self.end);
            return;
        }
        self.bump();
    }

    fn matching_close(&mut self) -> (usize, usize) {
        let mut depth = 1i32;
        let mut paren = 0u32;
        let mut bracket = 0u32;
        while !self.eof() {
            let byte = self.byte();
            if byte == b'\\' {
                self.skip_escape();
                continue;
            }
            if byte == b'"' || byte == b'\'' {
                self.consume_string(byte);
                continue;
            }
            if self.starts_comment() {
                self.consume_comment();
                continue;
            }
            if paren == 0 && bracket == 0 {
                if byte == b'{' {
                    depth += 1;
                } else if byte == b'}' {
                    depth -= 1;
                    let inner_end = self.index;
                    self.bump();
                    if depth == 0 {
                        return (inner_end, self.index);
                    }
                    continue;
                }
            }
            match byte {
                b'(' => paren += 1,
                b')' => paren = paren.saturating_sub(1),
                b'[' => bracket += 1,
                b']' => bracket = bracket.saturating_sub(1),
                _ => {}
            }
            self.bump();
        }
        (self.end, self.end)
    }
}

pub(crate) fn skip_ws(source: &str, mut index: usize, end: usize) -> usize {
    let bytes = source.as_bytes();
    while index < end && is_css_whitespace(bytes[index]) {
        index += 1;
    }
    index
}

pub(crate) fn starts_comment(source: &str, index: usize) -> bool {
    let bytes = source.as_bytes();
    index + 1 < bytes.len() && bytes[index] == b'/' && bytes[index + 1] == b'*'
}

pub(crate) fn comment_end(source: &str, start: usize, end: usize) -> usize {
    let bytes = source.as_bytes();
    let mut index = (start + 2).min(end);
    while index + 1 < end {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return index + 2;
        }
        index += char_len(source, index);
    }
    end
}

pub(crate) fn char_len(source: &str, index: usize) -> usize {
    source[index..].chars().next().map_or(1, char::len_utf8)
}
