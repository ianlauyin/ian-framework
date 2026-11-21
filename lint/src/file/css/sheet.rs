use lightningcss::rules::CssRule;
use lightningcss::rules::CssRuleList;
use lightningcss::rules::Location;
use lightningcss::rules::style::StyleRule;

use crate::file::css::parse::{Item, Kind, merge};
use crate::file::css::position::Map;
use crate::file::css::scan::{Bounds, bounds};

fn rule_item<R>(rule: &CssRule<'_, R>, map: &Map<'_>) -> Option<Item> {
    match rule {
        CssRule::Style(rule) => style_item(rule, map),
        CssRule::Media(rule) => group_item(rule.loc, &rule.rules, map),
        CssRule::Supports(rule) => group_item(rule.loc, &rule.rules, map),
        CssRule::Container(rule) => group_item(rule.loc, &rule.rules, map),
        CssRule::MozDocument(rule) => group_item(rule.loc, &rule.rules, map),
        CssRule::LayerBlock(rule) => group_item(rule.loc, &rule.rules, map),
        CssRule::Scope(rule) => group_item(rule.loc, &rule.rules, map),
        CssRule::StartingStyle(rule) => group_item(rule.loc, &rule.rules, map),
        CssRule::Nesting(rule) => group_item(rule.loc, &rule.style.rules, map),
        CssRule::Import(rule) => plain_item(rule.loc, map),
        CssRule::Keyframes(rule) => plain_item(rule.loc, map),
        CssRule::FontFace(rule) => plain_item(rule.loc, map),
        CssRule::FontPaletteValues(rule) => plain_item(rule.loc, map),
        CssRule::FontFeatureValues(rule) => plain_item(rule.loc, map),
        CssRule::Page(rule) => plain_item(rule.loc, map),
        CssRule::CounterStyle(rule) => plain_item(rule.loc, map),
        CssRule::Namespace(rule) => plain_item(rule.loc, map),
        CssRule::Viewport(rule) => plain_item(rule.loc, map),
        CssRule::CustomMedia(rule) => plain_item(rule.loc, map),
        CssRule::LayerStatement(rule) => plain_item(rule.loc, map),
        CssRule::Property(rule) => plain_item(rule.loc, map),
        CssRule::ViewTransition(rule) => plain_item(rule.loc, map),
        CssRule::PositionTry(rule) => plain_item(rule.loc, map),
        CssRule::Unknown(rule) => plain_item(rule.loc, map),
        CssRule::NestedDeclarations(_) | CssRule::Ignored | CssRule::Custom(_) => None,
    }
}

pub(crate) fn child_items<R>(rules: &CssRuleList<'_, R>, map: &Map<'_>) -> Vec<Item> {
    rules
        .0
        .iter()
        .filter_map(|rule| rule_item(rule, map))
        .collect()
}

fn style_item<R>(rule: &StyleRule<'_, R>, map: &Map<'_>) -> Option<Item> {
    let start = map.offset(rule.loc);
    let Bounds::Block {
        brace,
        inner_end,
        end,
    } = bounds(map.source, start, map.source.len())
    else {
        return None;
    };
    Some(Item {
        kind: Kind::Rule {
            selector_end: brace as u32,
            children: merge(map, child_items(&rule.rules, map), brace + 1, inner_end),
        },
        start: start as u32,
        end: end as u32,
    })
}

fn group_item<R>(loc: Location, rules: &CssRuleList<'_, R>, map: &Map<'_>) -> Option<Item> {
    at_item(map.offset(loc), child_items(rules, map), map)
}

fn plain_item(loc: Location, map: &Map<'_>) -> Option<Item> {
    at_item(map.offset(loc), Vec::new(), map)
}

fn at_item(start: usize, nested: Vec<Item>, map: &Map<'_>) -> Option<Item> {
    Some(match bounds(map.source, start, map.source.len()) {
        Bounds::Statement { end } if end <= start => return None,
        Bounds::Statement { end } => Item {
            kind: Kind::AtRule {
                brace: None,
                children: Vec::new(),
            },
            start: start as u32,
            end: end as u32,
        },
        Bounds::Block {
            brace,
            inner_end,
            end,
        } => Item {
            kind: Kind::AtRule {
                brace: Some(brace as u32),
                children: merge(map, nested, brace + 1, inner_end),
            },
            start: start as u32,
            end: end as u32,
        },
    })
}
