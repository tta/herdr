use crossterm::event::{KeyCode, KeyModifiers};
use unicode_width::UnicodeWidthStr;

use crate::input::TerminalKey;

pub(crate) struct CopyFindGlyph<'a> {
    pub col: u16,
    pub symbol: &'a str,
}

pub(crate) fn copy_find_glyphs<'a>(symbols: &'a [String]) -> Vec<CopyFindGlyph<'a>> {
    let mut glyphs = Vec::new();
    let mut col = 0u16;
    while usize::from(col) < symbols.len() {
        let symbol = symbols[usize::from(col)].as_str();
        if symbol.is_empty() {
            col = col.saturating_add(1);
            continue;
        }
        let width = u16::try_from(UnicodeWidthStr::width(symbol))
            .unwrap_or(u16::MAX)
            .max(1);
        glyphs.push(CopyFindGlyph { col, symbol });
        col = col.saturating_add(width);
    }
    glyphs
}

pub(crate) fn copy_find_char_col(
    glyphs: &[CopyFindGlyph<'_>],
    cursor_col: u16,
    needle: char,
    forward: bool,
    till: bool,
    repeat: bool,
) -> Option<u16> {
    if glyphs.is_empty() {
        return None;
    }
    let origin = glyphs
        .iter()
        .rposition(|glyph| glyph.col <= cursor_col)
        .map_or(-1, |index| index as isize);
    // A repeated till must step past the character it stopped beside, or `;`
    // finds that same character and the cursor does not move.
    let extra = isize::from(repeat && till);
    if forward {
        let mut index = origin + 1 + extra;
        while index < glyphs.len() as isize {
            let glyph_index = index as usize;
            if copy_find_glyph_matches(&glyphs[glyph_index], needle) {
                return copy_find_landing(glyphs, glyph_index, true, till);
            }
            index += 1;
        }
    } else {
        let mut index = origin - 1 - extra;
        while index >= 0 {
            let glyph_index = index as usize;
            if copy_find_glyph_matches(&glyphs[glyph_index], needle) {
                return copy_find_landing(glyphs, glyph_index, false, till);
            }
            index -= 1;
        }
    }
    None
}

fn copy_find_glyph_matches(glyph: &CopyFindGlyph<'_>, needle: char) -> bool {
    glyph.symbol.chars().any(|ch| ch == needle)
}

fn copy_find_landing(
    glyphs: &[CopyFindGlyph<'_>],
    index: usize,
    forward: bool,
    till: bool,
) -> Option<u16> {
    if !till {
        return Some(glyphs[index].col);
    }
    if forward {
        index.checked_sub(1).map(|previous| glyphs[previous].col)
    } else {
        glyphs.get(index.saturating_add(1)).map(|glyph| glyph.col)
    }
}

pub(crate) fn first_non_blank_col(text: &str) -> Option<u16> {
    let mut col = 0u16;
    for ch in text.chars() {
        if !ch.is_whitespace() {
            return Some(col);
        }
        col = col.saturating_add(char_cell_width(ch));
    }
    None
}

pub(crate) fn last_character_col(text: &str) -> Option<u16> {
    let mut col = 0u16;
    let mut last_col = None;
    for ch in text.chars() {
        let width = u16::from(crate::ghostty::unicode_codepoint_width(ch as u32));
        if width > 0 {
            last_col = Some(col);
            col = col.saturating_add(width);
        }
    }
    last_col
}

fn char_cell_width(ch: char) -> u16 {
    u16::from(crate::ghostty::unicode_codepoint_width(ch as u32)).max(1)
}

pub(crate) fn copy_mode_page_lines(height: u16, half_page: bool) -> usize {
    if height <= 2 {
        1
    } else if half_page {
        usize::from(height / 2)
    } else {
        usize::from(height - 2)
    }
}

pub(crate) fn copy_mode_command_char(key: TerminalKey) -> Option<char> {
    if !key.modifiers.difference(KeyModifiers::SHIFT).is_empty() {
        return None;
    }
    if let Some(ch) = key.shifted_codepoint.and_then(char::from_u32) {
        return Some(ch);
    }
    let KeyCode::Char(ch) = key.code else {
        return None;
    };
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        Some(shifted_ascii_char(ch).unwrap_or(ch))
    } else {
        Some(ch)
    }
}

fn shifted_ascii_char(ch: char) -> Option<char> {
    match ch {
        'a'..='z' => Some(ch.to_ascii_uppercase()),
        '1' => Some('!'),
        '2' => Some('@'),
        '3' => Some('#'),
        '4' => Some('$'),
        '5' => Some('%'),
        '6' => Some('^'),
        '7' => Some('&'),
        '8' => Some('*'),
        '9' => Some('('),
        '0' => Some(')'),
        '-' => Some('_'),
        '=' => Some('+'),
        '[' => Some('{'),
        ']' => Some('}'),
        '\\' => Some('|'),
        ';' => Some(':'),
        '\'' => Some('"'),
        ',' => Some('<'),
        '.' => Some('>'),
        '/' => Some('?'),
        '`' => Some('~'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_find_char_col, copy_find_glyphs};

    fn line(symbols: &[&str]) -> Vec<String> {
        symbols.iter().map(|symbol| (*symbol).to_owned()).collect()
    }

    fn find(symbols: &[&str], col: u16, needle: char, forward: bool, till: bool, repeat: bool) -> Option<u16> {
        let owned = line(symbols);
        let glyphs = copy_find_glyphs(&owned);
        copy_find_char_col(&glyphs, col, needle, forward, till, repeat)
    }

    #[test]
    fn find_and_till_stay_on_the_current_line() {
        let text = ["a", "b", "X", "c", "d", "X", "e", "f", "X"];
        assert_eq!(find(&text, 0, 'X', true, false, false), Some(2));
        assert_eq!(find(&text, 2, 'X', true, false, false), Some(5));
        assert_eq!(find(&text, 5, 'X', false, false, false), Some(2));
        assert_eq!(find(&text, 0, 'X', true, true, false), Some(1));
        assert_eq!(find(&text, 1, 'X', true, true, true), Some(4));
        assert_eq!(find(&text, 4, 'X', false, true, true), Some(3));
        assert_eq!(find(&text, 1, 'X', false, true, true), None);
        assert_eq!(find(&text, 0, 'Z', true, false, false), None);
        assert_eq!(find(&text, 0, 'a', true, false, false), None);
    }

    #[test]
    fn find_lands_on_a_wide_glyph_and_till_stops_before_it() {
        let text = ["a", "あ", "", "x"];
        assert_eq!(find(&text, 0, 'あ', true, false, false), Some(1));
        assert_eq!(find(&text, 0, 'x', true, true, false), Some(1));
        assert_eq!(find(&text, 3, 'あ', false, true, false), Some(3));
    }
}
