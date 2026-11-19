use std::path::Path;

pub(crate) struct Item {
    pub(crate) kind: Kind,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

pub(crate) enum Kind {
    Comment,
    Declaration,
    Rule {
        selector_end: u32,
        children: Vec<Item>,
    },
    AtRule {
        brace: Option<u32>,
        children: Vec<Item>,
    },
}

impl Item {
    pub(crate) fn is_comment(&self) -> bool {
        matches!(self.kind, Kind::Comment)
    }
}

pub(crate) fn parse(source: &str) -> Vec<Item> {
    let mut parser = Parser { source, index: 0 };
    if source.starts_with('\u{feff}') {
        parser.index = '\u{feff}'.len_utf8();
    }
    parser.parse_list(false)
}

struct Parser<'a> {
    source: &'a str,
    index: usize,
}

enum Stop {
    Semicolon,
    Brace,
    CloseBrace,
    Eof,
}

impl<'a> Parser<'a> {
    fn parse_list(&mut self, in_block: bool) -> Vec<Item> {
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            if self.eof() {
                break;
            }
            if in_block && self.byte() == b'}' {
                break;
            }
            if self.starts_comment() {
                let start = self.index as u32;
                self.consume_comment();
                items.push(Item {
                    kind: Kind::Comment,
                    start,
                    end: self.index as u32,
                });
                continue;
            }
            let before = self.index;
            let item = if self.byte() == b'@' {
                self.parse_at_rule()
            } else {
                self.parse_rule_or_declaration()
            };
            if self.index == before {
                self.bump();
                continue;
            }
            items.push(item);
        }
        items
    }

    fn parse_at_rule(&mut self) -> Item {
        let start = self.index as u32;
        self.bump();
        match self.scan() {
            Stop::Brace => {
                let brace = self.index as u32;
                self.bump();
                let (children, end) = self.parse_block();
                Item {
                    kind: Kind::AtRule {
                        brace: Some(brace),
                        children,
                    },
                    start,
                    end,
                }
            }
            Stop::Semicolon => {
                self.bump();
                Item {
                    kind: Kind::AtRule {
                        brace: None,
                        children: Vec::new(),
                    },
                    start,
                    end: self.index as u32,
                }
            }
            Stop::CloseBrace | Stop::Eof => Item {
                kind: Kind::AtRule {
                    brace: None,
                    children: Vec::new(),
                },
                start,
                end: self.index as u32,
            },
        }
    }

    fn parse_rule_or_declaration(&mut self) -> Item {
        let start = self.index as u32;
        match self.scan() {
            Stop::Brace => {
                let selector_end = self.index as u32;
                self.bump();
                let (children, end) = self.parse_block();
                Item {
                    kind: Kind::Rule {
                        selector_end,
                        children,
                    },
                    start,
                    end,
                }
            }
            Stop::Semicolon => {
                self.bump();
                Item {
                    kind: Kind::Declaration,
                    start,
                    end: self.index as u32,
                }
            }
            Stop::CloseBrace | Stop::Eof => Item {
                kind: Kind::Declaration,
                start,
                end: self.index as u32,
            },
        }
    }

    fn parse_block(&mut self) -> (Vec<Item>, u32) {
        let children = self.parse_list(true);
        if !self.eof() && self.byte() == b'}' {
            self.bump();
        }
        (children, self.index as u32)
    }

    fn scan(&mut self) -> Stop {
        let mut paren = 0i32;
        let mut bracket = 0i32;
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
                match byte {
                    b';' => return Stop::Semicolon,
                    b'{' => return Stop::Brace,
                    b'}' => return Stop::CloseBrace,
                    _ => {}
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
        Stop::Eof
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

    fn consume_comment(&mut self) {
        self.bump();
        self.bump();
        let bytes = self.source.as_bytes();
        while self.index + 1 < bytes.len() {
            if bytes[self.index] == b'*' && bytes[self.index + 1] == b'/' {
                self.index += 2;
                return;
            }
            self.bump();
        }
        self.index = bytes.len();
    }

    fn skip_escape(&mut self) {
        self.bump();
        if self.eof() {
            return;
        }
        let rest = &self.source[self.index..];
        if rest.starts_with("\r\n") {
            self.index += 2;
            return;
        }
        self.bump();
    }

    fn skip_ws(&mut self) {
        while !self.eof() && is_css_whitespace(self.byte()) {
            self.bump();
        }
    }

    fn starts_comment(&self) -> bool {
        let bytes = self.source.as_bytes();
        self.index + 1 < bytes.len() && bytes[self.index] == b'/' && bytes[self.index + 1] == b'*'
    }

    fn eof(&self) -> bool {
        self.index >= self.source.len()
    }

    fn byte(&self) -> u8 {
        self.source.as_bytes()[self.index]
    }

    fn bump(&mut self) {
        if self.eof() {
            return;
        }
        let byte = self.byte();
        if byte < 128 {
            self.index += 1;
            return;
        }
        let len = self.source[self.index..]
            .chars()
            .next()
            .map_or(1, char::len_utf8);
        self.index += len;
    }
}

pub(crate) fn is_css_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\x0c')
}

pub(crate) fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}
