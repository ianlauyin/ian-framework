use lightningcss::rules::Location;

use crate::rule::fe::css::position::Map;

#[test]
fn an_offset_counts_lines_from_zero_and_utf16_columns_from_one() {
    let map = Map::new("a\r\nbé c");
    let location = Location {
        source_index: 0,
        line: 1,
        column: 4,
    };
    assert_eq!(map.offset(location), 7);
}
