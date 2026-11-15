//! What a typed entry means: numbers with separators, currency, percentages, dates and
//! times (US conventions, as the original reads them), and the number format they bring.

use crate::dates;
use crate::value::{Value, XlErr};

pub const FMT_CURRENCY2: &str = "\"$\"#,##0.00_);[Red]\\(\"$\"#,##0.00\\)";
pub const FMT_CURRENCY0: &str = "\"$\"#,##0_);[Red]\\(\"$\"#,##0\\)";

const MONTHS: [&str; 12] = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"];

fn month_from(s: &str) -> Option<u32> {
    let l = s.to_lowercase();
    if l.len() < 3 {
        return None;
    }
    for (i, m) in MONTHS.iter().enumerate() {
        if l.starts_with(m) {
            let full = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"][i];
            if full.starts_with(&l) || l == *m || (l.len() == 4 && l == "sept") {
                return Some(i as u32 + 1);
            }
        }
    }
    None
}

/// Days in a month as dates are typed: February 1900 has the 29th the original counts.
fn mdays(y: i64, m: u32, d1904: bool) -> i64 {
    if y == 1900 && m == 2 && !d1904 {
        29
    } else {
        dates::days_in_month(y, m) as i64
    }
}

fn current_year() -> i64 {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    dates::civil_from_days(secs.div_euclid(86400)).0
}

/// A plain number with optional thousands separators (grouped correctly), sign,
/// parentheses, currency sign, percent and exponent. Returns (value, format to apply).
fn parse_plain(s: &str) -> Option<(f64, Option<String>)> {
    let mut t = s.trim().to_string();
    if t.is_empty() {
        return None;
    }
    let mut neg = false;
    let mut fmt: Option<String> = None;
    if t.starts_with('(') && t.ends_with(')') {
        neg = true;
        t = t[1..t.len() - 1].trim().to_string();
    }
    if let Some(x) = t.strip_prefix('-') {
        if neg {
            return None;
        }
        neg = true;
        t = x.trim_start().to_string();
    } else if let Some(x) = t.strip_prefix('+') {
        t = x.trim_start().to_string();
    }
    let (dec, grp, sym) = crate::locale::with(|l| (l.decimal, l.group, l.currency.clone()));
    let mut currency = false;
    if let Some(x) = t.strip_prefix(sym.as_str()) {
        currency = true;
        t = x.trim_start().to_string();
        if let Some(x) = t.strip_prefix('-') {
            if neg {
                return None;
            }
            neg = true;
            t = x.to_string();
        }
    } else if let Some(x) = t.strip_suffix(sym.as_str()) {
        // "12,50 €": the symbol after the amount, as many regions write it
        currency = true;
        t = x.trim_end().to_string();
    }
    // the region's separators, read as the point and comma the rest of this works with
    if dec != '.' || grp != ',' {
        if t.contains('.') && grp != '.' && dec != '.' {
            return None;
        }
        t = t.chars().map(|c| if c == dec { '.' } else if c == grp { ',' } else if c == '\u{a0}' && grp == ' ' { ',' } else { c }).collect();
        if grp == '.' || grp == ' ' {
            // "1.234" is a thousand and more, never 1.234, when the point groups
        }
    }
    let mut percent = false;
    if let Some(x) = t.strip_suffix('%') {
        percent = true;
        t = x.trim_end().to_string();
    }
    if t.is_empty() {
        return None;
    }
    // exponent
    let (mant, exp) = match t.find(['e', 'E']) {
        Some(i) => (t[..i].to_string(), Some(t[i + 1..].to_string())),
        None => (t.clone(), None),
    };
    let (int_part, frac_part) = match mant.find('.') {
        Some(i) => (mant[..i].to_string(), Some(mant[i + 1..].to_string())),
        None => (mant.clone(), None),
    };
    let grouped = int_part.contains(',');
    if grouped {
        let groups: Vec<&str> = int_part.split(',').collect();
        if groups[0].is_empty() || groups[0].len() > 3 || groups[1..].iter().any(|g| g.len() != 3) {
            return None;
        }
    }
    let digits: String = int_part.chars().filter(|c| *c != ',').collect();
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if let Some(f) = &frac_part {
        if !f.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
    }
    if digits.is_empty() && frac_part.as_deref().map(|f| f.is_empty()).unwrap_or(true) {
        return None;
    }
    let mut num_text = if digits.is_empty() { "0".to_string() } else { digits };
    if let Some(f) = &frac_part {
        num_text.push('.');
        num_text.push_str(f);
    }
    if let Some(e) = &exp {
        if e.is_empty() || !(e.chars().all(|c| c.is_ascii_digit()) || ((e.starts_with('+') || e.starts_with('-')) && e.len() > 1 && e[1..].chars().all(|c| c.is_ascii_digit()))) {
            return None;
        }
        if currency || percent || grouped {
            return None;
        }
        num_text.push('e');
        num_text.push_str(e);
    }
    let mut v: f64 = num_text.parse().ok()?;
    if !v.is_finite() {
        return None;
    }
    let decimals = frac_part.as_ref().map(|f| f.len()).unwrap_or(0);
    if percent {
        v /= 100.0;
        fmt = Some(if decimals > 0 { "0.00%".into() } else { "0%".into() });
    } else if currency {
        fmt = Some(crate::locale::currency_code(decimals > 0));
    } else if exp.is_some() {
        fmt = Some("0.00E+00".into());
    } else if grouped {
        fmt = Some(if decimals > 0 { "#,##0.00".into() } else { "#,##0".into() });
    }
    if neg {
        v = -v;
    }
    Some((v, fmt))
}

/// "1/2/2026", "1/2", "2026-01-02", "Jan 2", "2-Jan-26", "January 2, 2026", "Jan 2026"
fn parse_date(s: &str, d1904: bool) -> Option<(f64, &'static str)> {
    let t = s.trim();
    // numeric with / or -
    let order = crate::locale::with(|l| l.date_order);
    let point_dates = crate::locale::with(|l| l.decimal != '.') && t.matches('.').count() == 2;
    let sep = if t.contains('/') { Some('/') } else if t.contains('-') && !t.starts_with('-') { Some('-') } else if point_dates { Some('.') } else { None };
    if let Some(sep) = sep {
        let parts: Vec<&str> = t.split(sep).map(|x| x.trim()).collect();
        if parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
            let n: Vec<i64> = parts.iter().map(|p| p.parse().unwrap_or(0)).collect();
            match n.len() {
                3 => {
                    // y-m-d when the first part has 4 digits
                    let (y, m, d) = if parts[0].len() == 4 || order == crate::locale::DateOrder::Ymd {
                        (n[0], n[1], n[2])
                    } else if order == crate::locale::DateOrder::Dmy {
                        (n[2], n[1], n[0])
                    } else {
                        (n[2], n[0], n[1])
                    };
                    let y = if parts[0].len() != 4 && parts[2].len() <= 2 { if y < 30 { 2000 + y } else { 1900 + y } } else { y };
                    if (1..=12).contains(&m) && d >= 1 && d <= mdays(y, m as u32, d1904) as i64 && (1900..=9999).contains(&y) {
                        return dates::ymd_to_serial(y, m, d, d1904).map(|v| (v, "m/d/yyyy"));
                    }
                    return None;
                }
                2 => {
                    // m/d of this year, or m/yy (month and year)
                    let (a, b) = if order == crate::locale::DateOrder::Dmy && parts[1].len() != 4 { (n[1], n[0]) } else { (n[0], n[1]) };
                    if (1..=12).contains(&a) && b >= 1 && b <= mdays(current_year(), a as u32, d1904) as i64 {
                        return dates::ymd_to_serial(current_year(), a, b, d1904).map(|v| (v, "d-mmm"));
                    }
                    if (1..=12).contains(&a) && parts[1].len() == 4 {
                        return dates::ymd_to_serial(b, a, 1, d1904).map(|v| (v, "mmm-yy"));
                    }
                    return None;
                }
                _ => return None,
            }
        }
    }
    // with month names
    let norm: String = t.replace([',', '-', '/'], " ");
    let words: Vec<&str> = norm.split_whitespace().collect();
    if words.len() >= 2 && words.len() <= 3 {
        let mut month = None;
        let mut nums: Vec<(i64, usize)> = Vec::new();
        for w in &words {
            if let Some(m) = month_from(w) {
                if month.is_some() {
                    return None;
                }
                month = Some(m);
            } else if w.chars().all(|c| c.is_ascii_digit()) {
                nums.push((w.parse().ok()?, w.len()));
            } else {
                return None;
            }
        }
        let m = month? as i64;
        match nums.len() {
            1 => {
                let (x, len) = nums[0];
                if len == 4 || x > 31 {
                    let y = if len <= 2 { if x < 30 { 2000 + x } else { 1900 + x } } else { x };
                    return dates::ymd_to_serial(y, m, 1, d1904).map(|v| (v, "mmm-yy"));
                }
                if x >= 1 && x <= mdays(current_year(), m as u32, d1904) as i64 {
                    return dates::ymd_to_serial(current_year(), m, x, d1904).map(|v| (v, "d-mmm"));
                }
                return None;
            }
            2 => {
                let (d, y, ylen) = if words[0].chars().all(|c| c.is_ascii_digit()) {
                    (nums[0].0, nums[1].0, nums[1].1)
                } else {
                    (nums[0].0, nums[1].0, nums[1].1)
                };
                let y = if ylen <= 2 { if y < 30 { 2000 + y } else { 1900 + y } } else { y };
                if d >= 1 && d <= mdays(y, m as u32, d1904) as i64 {
                    return dates::ymd_to_serial(y, m, d, d1904).map(|v| (v, "d-mmm-yy"));
                }
                return None;
            }
            _ => return None,
        }
    }
    None
}

/// "12:30", "12:30:15", "1:30 PM", "1 PM", "13:00:00.5"
fn parse_time(s: &str) -> Option<(f64, &'static str)> {
    let t = s.trim();
    let lower = t.to_lowercase();
    let (body, ampm) = if let Some(x) = lower.strip_suffix("pm").or_else(|| lower.strip_suffix("p")) {
        (x.trim().to_string(), Some(true))
    } else if let Some(x) = lower.strip_suffix("am").or_else(|| lower.strip_suffix("a")) {
        (x.trim().to_string(), Some(false))
    } else {
        (lower.clone(), None)
    };
    let parts: Vec<&str> = body.split(':').collect();
    if parts.len() == 1 && ampm.is_none() {
        return None;
    }
    if parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let h: f64 = parts[0].parse().ok()?;
    let m: f64 = if parts.len() > 1 { parts[1].parse().ok()? } else { 0.0 };
    let sec: f64 = if parts.len() > 2 { parts[2].parse().ok()? } else { 0.0 };
    if !parts[0].chars().all(|c| c.is_ascii_digit()) || parts.get(1).map(|p| !p.chars().all(|c| c.is_ascii_digit())).unwrap_or(false) {
        return None;
    }
    if m >= 60.0 || sec >= 60.0 {
        return None;
    }
    let mut hh = h;
    if let Some(pm) = ampm {
        if !(1.0..=12.0).contains(&h) {
            return None;
        }
        hh = h % 12.0 + if pm { 12.0 } else { 0.0 };
    } else if h >= 10000.0 {
        return None;
    }
    let v = (hh * 3600.0 + m * 60.0 + sec) / 86400.0;
    let fmt = match (ampm.is_some(), parts.len()) {
        (true, 3) => "h:mm:ss AM/PM",
        (true, _) => "h:mm AM/PM",
        (false, 3) => "h:mm:ss",
        _ => {
            if h >= 24.0 {
                "[h]:mm:ss"
            } else {
                "h:mm"
            }
        }
    };
    Some((v, fmt))
}

/// A fraction typed as "1 1/2" or "0 3/4".
fn parse_fraction(s: &str) -> Option<(f64, &'static str)> {
    let t = s.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(x) => (true, x),
        None => (false, t),
    };
    let (whole, frac) = t.split_once(' ')?;
    let (n, d) = frac.trim().split_once('/')?;
    let w: f64 = whole.parse().ok()?;
    let n: f64 = n.parse().ok()?;
    let d: f64 = d.parse().ok()?;
    if d == 0.0 || !whole.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let v = w + n / d;
    Some((if neg { -v } else { v }, if d > 9.0 { "# ??/??" } else { "# ?/?" }))
}

/// Text that reads as a number (VALUE() and arithmetic on text use this too).
pub fn parse_number_text(s: &str, d1904: bool) -> Option<(f64, Option<String>)> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(x) = parse_plain(t) {
        return Some(x);
    }
    if let Some((v, f)) = parse_fraction(t) {
        return Some((v, Some(f.into())));
    }
    // date and time together: "1/2/2026 13:00"
    if let Some(sp) = t.rfind(' ') {
        let (a, b) = (&t[..sp], &t[sp + 1..]);
        let (a2, b2) = if b.eq_ignore_ascii_case("am") || b.eq_ignore_ascii_case("pm") {
            match a.rfind(' ') {
                Some(k) => (&t[..k], &t[k + 1..]),
                None => (a, b),
            }
        } else {
            (a, b)
        };
        if let (Some((d, _)), Some((tm, tf))) = (parse_date(a2, d1904), parse_time(b2)) {
            // the original shows a typed date and time as m/d/yyyy h:mm, whatever clock was typed
            let _ = tf;
            return Some((d + tm, Some("m/d/yyyy h:mm".into())));
        }
    }
    if let Some((v, f)) = parse_time(t) {
        return Some((v, Some(f.into())));
    }
    if let Some((v, f)) = parse_date(t, d1904) {
        return Some((v, Some(f.into())));
    }
    None
}

/// What a typed entry becomes.
pub enum Typed {
    Empty,
    Formula(String),
    Value(Value, Option<String>),
    /// text after an apostrophe: kept as text, marked so it stays text
    QuotedText(String),
}

pub fn interpret(text: &str, d1904: bool, cell_is_text_format: bool) -> Typed {
    if text.is_empty() {
        return Typed::Empty;
    }
    if let Some(rest) = text.strip_prefix('\'') {
        return Typed::QuotedText(rest.to_string());
    }
    if cell_is_text_format {
        return Typed::Value(Value::string(text.to_string()), None);
    }
    if text.starts_with('=') && text.len() > 1 {
        return Typed::Formula(text[1..].to_string());
    }
    let t = text.trim();
    if t.eq_ignore_ascii_case("TRUE") {
        return Typed::Value(Value::Bool(true), None);
    }
    if t.eq_ignore_ascii_case("FALSE") {
        return Typed::Value(Value::Bool(false), None);
    }
    if let Some(e) = XlErr::parse(t) {
        return Typed::Value(Value::Err(e), None);
    }
    if let Some((v, f)) = parse_number_text(text, d1904) {
        return Typed::Value(Value::num(fifteen_digits(v)), f);
    }
    // +1+1, +A1, -A1+B1 typed without '=' are formulas
    if (text.starts_with('+') || text.starts_with('-')) && text.len() > 1 && crate::formula::check(text).is_none() {
        return Typed::Formula(text.to_string());
    }
    Typed::Value(Value::string(text.to_string()), None)
}

/// How a number shows in the formula bar for its format (dates as dates, times as times).
pub fn edit_text(v: f64, fmt: &crate::numfmt::Format, code: &str, d1904: bool) -> String {
    if fmt.is_date() && !code.contains("[h]") && !code.contains("[m]") && !code.contains("[s]") {
        let has_date = code.to_lowercase().contains('y') || code.to_lowercase().contains('d') || {
            // m alone (month) counts as a date part when no hour is around
            let l = code.to_lowercase();
            l.contains('m') && !l.contains('h') && !l.contains('s')
        };
        let has_time = fmt.has_time();
        if v >= 0.0 {
            // in the region's short date; a region other than the US shows a 24-hour clock
            let us = crate::locale::with(|l| l.short_date == "m/d/yyyy");
            let (dt, tm) = if us { ("m/d/yyyy h:mm:ss AM/PM", "h:mm:ss AM/PM") } else { ("m/d/yyyy h:mm:ss", "h:mm:ss") };
            if has_date && has_time {
                return crate::numfmt::get(dt).number(v, d1904, 20).text;
            }
            if has_time && v < 1.0 {
                return crate::numfmt::get(tm).number(v, d1904, 20).text;
            }
            if has_time {
                return crate::numfmt::get(dt).number(v, d1904, 20).text;
            }
            return crate::numfmt::get("m/d/yyyy").number(v, d1904, 20).text;
        }
    }
    if fmt.is_percent() {
        let s = crate::locale::num_text(v * 100.0);
        return format!("{s}%");
    }
    crate::locale::num_text(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn num(s: &str) -> Option<(f64, Option<String>)> {
        parse_number_text(s, false)
    }
    #[test]
    fn typed() {
        assert_eq!(num("1,234.5").unwrap().0, 1234.5);
        assert!(num("1,23").is_none());
        assert_eq!(num("$1,234.5").unwrap(), (1234.5, Some(FMT_CURRENCY2.into())));
        assert_eq!(num("10%").unwrap(), (0.1, Some("0%".into())));
        assert_eq!(num("(5)").unwrap().0, -5.0);
        assert_eq!(num("1e5").unwrap(), (100000.0, Some("0.00E+00".into())));
        assert_eq!(num("1/2/2026").unwrap(), (46024.0, Some("m/d/yyyy".into())));
        assert_eq!(num("2026-01-02").unwrap().0, 46024.0);
        assert_eq!(num("12:30").unwrap(), (0.5208333333333334, Some("h:mm".into())));
        assert_eq!(num("1:30 PM").unwrap().1, Some("h:mm AM/PM".into()));
        assert_eq!(num("1 1/2").unwrap().0, 1.5);
        assert_eq!(num("January 2, 2026").unwrap().0, 46024.0);
        assert_eq!(num("2-Jan-26").unwrap().0, 46024.0);
        assert!(num("abc").is_none());
        assert!(num("1.2.3").is_none());
        assert_eq!(num(".5").unwrap().0, 0.5);
    }
}

/// The original keeps 15 significant digits of a typed number and drops the rest
/// (123456789012345678 is stored as 123456789012345000).
pub fn fifteen_digits(v: f64) -> f64 {
    if v == 0.0 || !v.is_finite() {
        return v;
    }
    // the shortest text that reads back as this number (0.015, not 0.01499999...)
    let t = format!("{:e}", v.abs());
    let (mant, exp) = match t.split_once('e') {
        Some(x) => x,
        None => return v,
    };
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() <= 15 || digits[15..].chars().all(|c| c == '0') {
        return v;
    }
    let kept = format!("{}.{}e{}", &digits[..1], &digits[1..15], exp);
    let r: f64 = kept.parse().unwrap_or(v.abs());
    if v < 0.0 { -r } else { r }
}
