use lightningcss::rules::Location;

pub(crate) struct Map<'a> {
    pub(crate) source: &'a str,
    lines: Vec<usize>,
}

impl<'a> Map<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            source,
            lines: line_starts(source),
        }
    }

    pub(crate) fn offset(&self, loc: Location) -> usize {
        let start = self
            .lines
            .get(loc.line as usize)
            .copied()
            .unwrap_or(self.source.len());
        let mut index = start;
        let mut column = 1u32;
        while index < self.source.len() && column < loc.column {
            let byte = self.source.as_bytes()[index];
            if matches!(byte, b'\n' | b'\r' | b'\x0c') {
                break;
            }
            let ch = self.source[index..].chars().next().unwrap_or('\u{fffd}');
            column += ch.len_utf16() as u32;
            index += ch.len_utf8();
        }
        index
    }
}

fn line_starts(source: &str) -> Vec<usize> {
    let bytes = source.as_bytes();
    let mut lines = vec![0];
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                index += 1;
                if index < bytes.len() && bytes[index] == b'\n' {
                    index += 1;
                }
                lines.push(index);
            }
            b'\n' | b'\x0c' => {
                index += 1;
                lines.push(index);
            }
            byte if byte < 128 => index += 1,
            _ => index += source[index..].chars().next().map_or(1, char::len_utf8),
        }
    }
    lines
}
