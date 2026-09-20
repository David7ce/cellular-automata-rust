//! Reader/writer for Golly's two-state RLE pattern format: optional `#` comment
//! lines, an optional `x = .., y = .., rule = ..` header, then a `b`/`o`/`$`
//! run-length body ended by `!`.

use crate::rules::RuleSet;

/// Bounds hostile or accidental input (`99999999999o!`, a megabyte of `$`).
const MAX_EXTENT: i64 = 1 << 20;
const MAX_CELLS: usize = 1 << 20;

#[derive(Debug)]
pub struct Rle {
    pub cells: Vec<(i32, i32)>,
    /// From the header's `rule =` field; `None` when the header has no rule
    /// (Golly then assumes B3/S23) or there is no header at all.
    pub rule: Option<RuleSet>,
}

pub fn parse(text: &str) -> Result<Rle, String> {
    let mut rle = Rle { cells: Vec::new(), rule: None };
    let (mut x, mut y, mut count) = (0i64, 0i64, 0i64);
    let mut in_body = false;

    for (idx, line) in text.lines().enumerate() {
        let line = line.trim();
        let at = |msg: String| format!("line {}: {msg}", idx + 1);
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if !in_body && line.starts_with('x') {
            for field in line.split(',') {
                if let Some((key, value)) = field.split_once('=')
                    && key.trim() == "rule"
                {
                    rle.rule = Some(RuleSet::from_bs_string(value).map_err(at)?);
                }
            }
            continue;
        }
        in_body = true;
        for ch in line.chars() {
            match ch {
                '0'..='9' => {
                    count = count * 10 + ch.to_digit(10).unwrap() as i64;
                    if count > MAX_EXTENT {
                        return Err(at("run count too large".into()));
                    }
                }
                'b' | 'o' | '$' => {
                    let n = count.max(1);
                    count = 0;
                    match ch {
                        'b' => x += n,
                        '$' => (x, y) = (0, y + n),
                        _ => {
                            if x + n > MAX_EXTENT || rle.cells.len() + n as usize > MAX_CELLS {
                                return Err(at("pattern too large".into()));
                            }
                            rle.cells.extend((x..x + n).map(|cx| (cx as i32, y as i32)));
                            x += n;
                        }
                    }
                    if x > MAX_EXTENT || y > MAX_EXTENT {
                        return Err(at("pattern too large".into()));
                    }
                }
                '!' => return Ok(rle),
                c if c.is_whitespace() => {}
                c => return Err(at(format!("unexpected character '{c}'"))),
            }
        }
    }
    Err("missing terminating '!'".into())
}

/// For the built-in, test-covered pattern strings.
pub fn cells(rle: &str) -> Vec<(i32, i32)> {
    parse(rle).expect("built-in RLE is valid").cells
}

/// Golly-style RLE for `cells`, shifted so the bounding box starts at (0, 0),
/// wrapped at 70 columns, with `rule` in the header.
pub fn to_rle(cells: impl IntoIterator<Item = (i64, i64)>, rule: RuleSet) -> String {
    let mut cells: Vec<(i64, i64)> = cells.into_iter().map(|(x, y)| (y, x)).collect();
    cells.sort_unstable();
    let (Some(&(min_y, _)), Some(&(max_y, _))) = (cells.first(), cells.last()) else {
        return format!("x = 0, y = 0, rule = {}\n!\n", rule.to_bs_string());
    };
    let min_x = cells.iter().map(|c| c.1).min().unwrap();
    let max_x = cells.iter().map(|c| c.1).max().unwrap();

    let mut tokens: Vec<String> = Vec::new();
    let run = |n: i64, ch: char| if n == 1 { ch.to_string() } else { format!("{n}{ch}") };
    let (mut row, mut col) = (min_y, 0i64);
    let mut i = 0;
    while i < cells.len() {
        let (cy, cx) = (cells[i].0, cells[i].1 - min_x);
        if cy != row {
            tokens.push(run(cy - row, '$'));
            (row, col) = (cy, 0);
        }
        if cx > col {
            tokens.push(run(cx - col, 'b'));
        }
        let mut len = 1;
        while i + (len as usize) < cells.len() && cells[i + len as usize] == (cy, cells[i].1 + len) {
            len += 1;
        }
        tokens.push(run(len, 'o'));
        col = cx + len;
        i += len as usize;
    }
    tokens.push("!".into());

    let mut out = format!("x = {}, y = {}, rule = {}\n", max_x - min_x + 1, max_y - min_y + 1, rule.to_bs_string());
    let mut line_len = 0;
    for token in tokens {
        if line_len + token.len() > 70 {
            out.push('\n');
            line_len = 0;
        }
        out.push_str(&token);
        line_len += token.len();
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn life() -> RuleSet {
        RuleSet::from_counts(&[3], &[2, 3])
    }

    fn normalized(cells: impl IntoIterator<Item = (i64, i64)>) -> HashSet<(i64, i64)> {
        let cells: Vec<_> = cells.into_iter().collect();
        let (mx, my) = (cells.iter().map(|c| c.0).min().unwrap(), cells.iter().map(|c| c.1).min().unwrap());
        cells.into_iter().map(|(x, y)| (x - mx, y - my)).collect()
    }

    #[test]
    fn parses_a_golly_file_with_header_and_comments() {
        let rle = parse("#N Glider\n#C comment\nx = 3, y = 3, rule = B3/S23\nbo$2bo$3o!\n").unwrap();
        assert_eq!(rle.cells.len(), 5);
        assert_eq!(rle.rule, Some(life()));
    }

    #[test]
    fn header_is_optional() {
        let rle = parse("2o$2o!").unwrap();
        assert_eq!((rle.cells.len(), rle.rule), (4, None));
    }

    #[test]
    fn accepts_legacy_rule_spelling_in_the_header() {
        assert_eq!(parse("x = 1, y = 1, rule = 23/36\no!").unwrap().rule.unwrap().to_bs_string(), "B36/S23");
    }

    #[test]
    fn stops_at_the_terminator() {
        assert_eq!(parse("3o!\n2o!").unwrap().cells.len(), 3);
        assert_eq!(parse("3o!garbage").unwrap().cells.len(), 3);
    }

    #[test]
    fn rejects_malformed_input_with_a_line_number() {
        for (text, needle) in [
            ("zq3o!", "unexpected character 'z'"),
            ("3o", "missing terminating"),
            ("", "missing terminating"),
            ("99999999999o!", "run count too large"),
            ("1000000o1000000$1000000o!", "too large"),
            ("x = 1, y = 1, rule = B0/S\no!", "B0"),
            ("x = 1, y = 1, rule = B3/S23:T5,5\no!", "bounded-grid"),
        ] {
            let err = parse(text).err().unwrap_or_else(|| panic!("{text:?} should fail"));
            assert!(err.contains(needle), "{text:?} -> {err}");
        }
        assert!(parse("2o\n3z!").unwrap_err().starts_with("line 2:"));
    }

    #[test]
    fn writes_the_documented_golly_form() {
        let r_pentomino = cells("b2o$2o$bo!").iter().map(|&(x, y)| (x as i64, y as i64)).collect::<Vec<_>>();
        assert_eq!(to_rle(r_pentomino, life()), "x = 3, y = 3, rule = B3/S23\nb2o$2o$bo!\n");
        assert_eq!(to_rle([], life()), "x = 0, y = 0, rule = B3/S23\n!\n");
    }

    #[test]
    fn export_wraps_at_70_columns_and_round_trips() {
        let cells: Vec<(i64, i64)> = (0..60).flat_map(|i| [(i * 2, i), (i * 2 + 5, i)]).collect();
        let text = to_rle(cells.iter().copied(), life());
        assert!(text.lines().all(|l| l.len() <= 70), "{text}");
        let back = parse(&text).unwrap();
        assert_eq!(back.rule, Some(life()));
        assert_eq!(normalized(back.cells.iter().map(|&(x, y)| (x as i64, y as i64))), normalized(cells));
    }
}
