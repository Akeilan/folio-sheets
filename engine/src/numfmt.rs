//! Number format codes: parsing and formatting, as cells, TEXT() and the status bar
//! show values.

use crate::dates;

/// The result of formatting: text plus what the cell renderer needs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Formatted {
    pub text: String,
    pub color: Option<u32>,
    /// a repeated fill character (*x) and the byte position in `text` where it goes
    pub fill: Option<(usize, char)>,
    /// numbers and dates show ##### instead of overflowing the cell
    pub hash_on_overflow: bool,
    /// widths of `_x` padding characters: (byte position, the character whose width to leave)
    pub pads: Vec<(usize, char)>,
}

impl Formatted {
    pub fn plain(t: &str) -> Formatted {
        Formatted { text: t.to_string(), ..Default::default() }
    }
}

/// Increase/Decrease Decimal: the format code with one decimal place more or less.
/// General starts from the decimals the value shows.
pub fn change_decimals(code: &str, v: &crate::value::Value, delta: i32, _d1904: bool) -> Option<String> {
    if code.eq_ignore_ascii_case("General") || code.is_empty() {
        let n = match v {
            crate::value::Value::Num(n) => *n,
            _ => return if delta > 0 { Some("0.0".into()) } else { None },
        };
        let shown = general(n, 11);
        let dec = match shown.find('.') {
            Some(i) if !shown.contains('E') => shown[i + 1..].len() as i32,
            _ => 0,
        };
        let want = dec + delta;
        if want < 0 {
            return None;
        }
        if want == 0 {
            return Some("0".into());
        }
        return Some(format!("0.{}", "0".repeat(want as usize)));
    }
    let mut out = String::new();
    let mut changed = false;
    for (i, sec) in split_code_sections(code).iter().enumerate() {
        if i > 0 {
            out.push(';');
        }
        match change_section(sec, delta) {
            Some(x) => {
                changed = true;
                out.push_str(&x);
            }
            None => out.push_str(sec),
        }
    }
    if changed { Some(out) } else { None }
}

fn split_code_sections(code: &str) -> Vec<String> {
    let mut v = vec![String::new()];
    let mut q = false;
    let mut esc = false;
    let mut br = false;
    for ch in code.chars() {
        if esc {
            v.last_mut().unwrap().push(ch);
            esc = false;
            continue;
        }
        match ch {
            '\\' if !q => esc = true,
            '"' => q = !q,
            '[' if !q => br = true,
            ']' if !q => br = false,
            ';' if !q && !br => {
                v.push(String::new());
                continue;
            }
            _ => {}
        }
        v.last_mut().unwrap().push(ch);
    }
    v
}

/// One section: the digit placeholders of the number part (before E or %), outside
/// quotes and brackets.
fn change_section(sec: &str, delta: i32) -> Option<String> {
    let chars: Vec<char> = sec.chars().collect();
    let mut q = false;
    let mut br = false;
    let mut esc = false;
    let mut last_digit: Option<usize> = None;
    let mut dot: Option<usize> = None;
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if esc {
            esc = false;
        } else if q {
            if ch == '"' {
                q = false;
            }
        } else if br {
            if ch == ']' {
                br = false;
            }
        } else {
            match ch {
                '"' => q = true,
                '[' => br = true,
                '\\' | '_' | '*' => {
                    esc = true;
                }
                '0' | '#' | '?' => last_digit = Some(i),
                '.' if dot.is_none() => dot = Some(i),
                'E' | 'e' => break,
                '@' => return None,
                'y' | 'd' | 'h' | 's' | 'm' if dot.is_none() && last_digit.is_none() => return None,
                _ => {}
            }
        }
        i += 1;
    }
    let ld = last_digit?;
    let mut out: Vec<char> = chars.clone();
    if delta > 0 {
        match dot {
            Some(_) => out.insert(ld + 1, '0'),
            None => {
                out.insert(ld + 1, '0');
                out.insert(ld + 1, '.');
            }
        }
    } else {
        let d = dot?;
        if ld > d {
            out.remove(ld);
            if ld == d + 1 {
                out.remove(d);
            }
        } else {
            return None;
        }
    }
    Some(out.into_iter().collect())
}

// --------------------------------------------------------------------------- decimal digits

/// Decimal digits of x > 0 rounded to 15 significant digits: (digits, exponent) where
/// x = 0.d1d2d3... * 10^exponent. Trailing zeros removed.
pub fn sig15(x: f64) -> (Vec<u8>, i32) {
    let s = format!("{:.14e}", x);
    let (mant, exp) = s.split_once('e').unwrap();
    let e: i32 = exp.parse().unwrap();
    let mut d: Vec<u8> = mant.bytes().filter(|c| c.is_ascii_digit()).map(|c| c - b'0').collect();
    while d.len() > 1 && *d.last().unwrap() == 0 {
        d.pop();
    }
    (d, e + 1)
}

/// x >= 0 as fixed point with `dec` decimals (15-digit precision, halves rounded away
/// from zero): (integer digits, fraction digits).
pub fn fixed(x: f64, dec: usize) -> (String, String) {
    if x == 0.0 || !x.is_finite() {
        return ("0".into(), "0".repeat(dec));
    }
    let (d, e) = sig15(x);
    // digit vector covering integer and `dec + 1` fraction digits
    let int_len = e.max(0) as usize;
    let mut digits: Vec<u8> = Vec::with_capacity(int_len + dec + 2);
    // integer part
    for i in 0..int_len {
        digits.push(*d.get(i).unwrap_or(&0));
    }
    // fraction part: positions after the point; digit index for fraction position k is e + k
    for k in 0..=dec {
        let idx = e as i64 + k as i64;
        let v = if idx >= 0 { *d.get(idx as usize).unwrap_or(&0) } else { 0 };
        digits.push(v);
    }
    // round at position int_len + dec
    let round_digit = digits.pop().unwrap();
    if round_digit >= 5 {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, 1);
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let ilen = digits.len() - dec;
    let ip: String = digits[..ilen].iter().map(|c| (c + b'0') as char).collect();
    let fp: String = digits[ilen..].iter().map(|c| (c + b'0') as char).collect();
    let ip = ip.trim_start_matches('0').to_string();
    (if ip.is_empty() { "0".into() } else { ip }, fp)
}

/// A number as text the way formulas convert it ("0.333333333333333", "1E+21").
pub fn num_to_text(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let neg = v < 0.0;
    let a = v.abs();
    let (d, e) = sig15(a);
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    if e > 15 || e < -9 {
        // scientific: d.ddddE+xx
        s.push((d[0] + b'0') as char);
        if d.len() > 1 {
            s.push('.');
            for c in &d[1..] {
                s.push((c + b'0') as char);
            }
        }
        let ex = e - 1;
        s.push_str(&format!("E{}{:02}", if ex < 0 { '-' } else { '+' }, ex.abs()));
        return s;
    }
    if e <= 0 {
        s.push_str("0.");
        for _ in 0..(-e) {
            s.push('0');
        }
        for c in &d {
            s.push((c + b'0') as char);
        }
    } else {
        let e = e as usize;
        for i in 0..e.max(d.len()) {
            if i == e {
                s.push('.');
            }
            s.push((*d.get(i).unwrap_or(&0) + b'0') as char);
        }
    }
    s
}

/// General format within `width` characters (11 for a cell of standard width, as the
/// original does), switching to scientific notation when the digits don't fit.
pub fn general(v: f64, width: usize) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let width = width.max(1);
    let neg = v < 0.0;
    let a = v.abs();
    let room = if neg { width.saturating_sub(1) } else { width }.max(1);
    let (d, e) = sig15(a);
    // try plain decimal
    let plain = |dec: usize| -> String {
        let (ip, fp) = fixed(a, dec);
        let fp = fp.trim_end_matches('0').to_string();
        if fp.is_empty() { ip } else { format!("{ip}.{fp}") }
    };
    if e <= 11 && e > -9 || (e > 0 && (e as usize) <= room) {
        let int_len = if e > 0 { e as usize } else { 1 };
        if int_len <= room {
            let max_dec = room.saturating_sub(int_len + 1).min(15usize.saturating_sub(if e > 0 { e as usize } else { 0 }));
            let s = if int_len >= room { plain(0) } else { plain(max_dec) };
            let nonzero = s.bytes().any(|c| (b'1'..=b'9').contains(&c));
            if s.len() <= room && nonzero && !(e <= 0 && sig_digits_shown(&s) < d.len().min(2) && e < -4) {
                return if neg { format!("-{s}") } else { s };
            }
        }
    }
    // scientific
    let exp = e - 1;
    let exp_s = format!("E{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
    let mant_room = room.saturating_sub(exp_s.len());
    let mdec = mant_room.saturating_sub(2).min(5);
    let m = a / 10f64.powi(exp);
    let (mut ip, fp) = fixed(m, mdec);
    let mut exp_adj = exp;
    if ip == "10" {
        ip = "1".into();
        exp_adj += 1;
    }
    let fp = fp.trim_end_matches('0');
    let exp_s = format!("E{}{:02}", if exp_adj < 0 { '-' } else { '+' }, exp_adj.abs());
    let s = if fp.is_empty() { format!("{ip}{exp_s}") } else { format!("{ip}.{fp}{exp_s}") };
    if neg {
        format!("-{s}")
    } else {
        s
    }
}

fn sig_digits_shown(s: &str) -> usize {
    s.trim_start_matches(['0', '.']).chars().filter(|c| c.is_ascii_digit()).count()
}

// --------------------------------------------------------------------------- format codes

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Lit(String),
    Digit(u8), // b'0' b'#' b'?'
    Point,
    Comma,
    Percent,
    Exp(bool, bool), // plus sign, upper case
    Slash,
    At,
    Fill(char),
    Pad(char),
    General,
    Year(u8),
    Month(u8),
    Day(u8),
    Hour(u8),
    Minute(u8),
    Second(u8),
    SubSec(u8),
    AmPm(String),
    Elapsed(char, u8),
}

#[derive(Clone, Debug)]
struct Section {
    toks: Vec<Tok>,
    color: Option<u32>,
    cond: Option<(String, f64)>,
}

#[derive(Clone, Debug)]
pub struct Format {
    sections: Vec<Section>,
}

const COLORS: [(&str, u32); 8] = [
    ("black", 0x000000), ("blue", 0x0000FF), ("cyan", 0x00FFFF), ("green", 0x00FF00),
    ("magenta", 0xFF00FF), ("red", 0xFF0000), ("white", 0xFFFFFF), ("yellow", 0xFFFF00),
];

pub const PALETTE: [u32; 56] = [
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0x800000, 0x008000,
    0x000080, 0x808000, 0x800080, 0x008080, 0xC0C0C0, 0x808080, 0x9999FF, 0x993366, 0xFFFFCC, 0xCCFFFF,
    0x660066, 0xFF8080, 0x0066CC, 0xCCCCFF, 0x000080, 0xFF00FF, 0xFFFF00, 0x00FFFF, 0x800080, 0x800000,
    0x008080, 0x0000FF, 0x00CCFF, 0xCCFFFF, 0xCCFFCC, 0xFFFF99, 0x99CCFF, 0xFF99CC, 0xCC99FF, 0xFFCC99,
    0x3366FF, 0x33CCCC, 0x99CC00, 0xFFCC00, 0xFF9900, 0xFF6600, 0x666699, 0x969696, 0x003366, 0x339966,
    0x003300, 0x333300, 0x993300, 0x993366, 0x333399, 0x333333,
];

/// System locale codes the original writes for its long-date and time presets.
fn system_code(code: &str) -> Option<&'static str> {
    let u = code.to_ascii_uppercase();
    if u.starts_with("[$-F800]") {
        return Some("dddd, mmmm d, yyyy");
    }
    if u.starts_with("[$-F400]") {
        return Some("h:mm:ss AM/PM");
    }
    None
}

impl Format {
    pub fn parse(code: &str) -> Format {
        let code = system_code(code).unwrap_or(code);
        let mut sections = Vec::new();
        for part in split_sections(code) {
            sections.push(parse_section(&part));
        }
        if sections.is_empty() {
            sections.push(Section { toks: vec![Tok::General], color: None, cond: None });
        }
        Format { sections }
    }

    pub fn is_date(&self) -> bool {
        self.sections.first().map(|s| s.toks.iter().any(is_date_tok)).unwrap_or(false)
    }
    pub fn is_text(&self) -> bool {
        self.sections.len() == 1 && self.sections[0].toks.iter().any(|t| matches!(t, Tok::At))
            && !self.sections[0].toks.iter().any(|t| matches!(t, Tok::Digit(_) | Tok::General))
    }
    pub fn has_time(&self) -> bool {
        self.sections.first().map(|s| s.toks.iter().any(|t| matches!(t, Tok::Hour(_) | Tok::Minute(_) | Tok::Second(_) | Tok::Elapsed(..)))).unwrap_or(false)
    }
    pub fn is_percent(&self) -> bool {
        self.sections.first().map(|s| s.toks.iter().any(|t| matches!(t, Tok::Percent))).unwrap_or(false)
    }
    pub fn is_general(&self) -> bool {
        self.sections.len() == 1 && self.sections[0].toks.iter().any(|t| matches!(t, Tok::General))
    }
    /// Number of decimals in the first section (for Increase/Decrease Decimal).
    pub fn decimals(&self) -> Option<usize> {
        let s = self.sections.first()?;
        let mut after = false;
        let mut n = 0;
        for t in &s.toks {
            match t {
                Tok::Point => after = true,
                Tok::Digit(_) if after => n += 1,
                Tok::Exp(..) | Tok::Slash => break,
                _ => {}
            }
        }
        Some(n)
    }

    /// Format a number. `width`: characters available for General.
    pub fn number(&self, v: f64, date1904: bool, width: usize) -> Formatted {
        if !v.is_finite() {
            return Formatted { text: "#NUM!".into(), ..Default::default() };
        }
        let (sec, abs_sign) = self.pick(v);
        let s = &self.sections[sec];
        let mut f = format_section(s, v, abs_sign, date1904, width);
        if f.color.is_none() {
            f.color = s.color;
        }
        f
    }

    fn pick(&self, v: f64) -> (usize, bool) {
        let n = self.sections.len();
        let has_cond = self.sections.iter().any(|s| s.cond.is_some());
        if has_cond {
            for (i, s) in self.sections.iter().enumerate().take(2) {
                if let Some((op, x)) = &s.cond {
                    if cond_ok(op, *x, v) {
                        // a condition section shows the sign itself unless it tests for negatives
                        let strip = v < 0.0 && (op.starts_with('<') && *x <= 0.0);
                        return (i, strip);
                    }
                }
            }
            // the section without a condition (third, or second) takes the rest
            let idx = self.sections.iter().position(|s| s.cond.is_none()).unwrap_or(n - 1);
            return (idx.min(n - 1), false);
        }
        match n {
            1 => (0, false),
            2 => {
                if v < 0.0 {
                    (1, true)
                } else {
                    (0, false)
                }
            }
            _ => {
                if v > 0.0 {
                    (0, false)
                } else if v < 0.0 {
                    (1, true)
                } else {
                    (2, false)
                }
            }
        }
    }

    pub fn text(&self, t: &str) -> Formatted {
        let sec = if self.sections.len() >= 4 {
            Some(&self.sections[3])
        } else if self.sections.len() == 1 && self.sections[0].toks.iter().any(|x| matches!(x, Tok::At)) {
            Some(&self.sections[0])
        } else {
            None
        };
        let sec = match sec {
            Some(s) => s,
            None => return Formatted { text: t.to_string(), ..Default::default() },
        };
        let mut f = Formatted { color: sec.color, ..Default::default() };
        for tok in &sec.toks {
            match tok {
                Tok::At => f.text.push_str(t),
                Tok::Lit(l) => f.text.push_str(l),
                Tok::Fill(c) => f.fill = Some((f.text.len(), *c)),
                Tok::Pad(c) => {
                    f.pads.push((f.text.len(), *c));
                    f.text.push(' ');
                }
                _ => {}
            }
        }
        f
    }
}

fn cond_ok(op: &str, x: f64, v: f64) -> bool {
    match op {
        "<" => v < x,
        "<=" => v <= x,
        ">" => v > x,
        ">=" => v >= x,
        "=" => v == x,
        "<>" => v != x,
        _ => false,
    }
}

fn is_date_tok(t: &Tok) -> bool {
    matches!(t, Tok::Year(_) | Tok::Month(_) | Tok::Day(_) | Tok::Hour(_) | Tok::Minute(_) | Tok::Second(_)
        | Tok::AmPm(_) | Tok::Elapsed(..) | Tok::SubSec(_))
}

fn split_sections(code: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = code.chars().peekable();
    let mut inq = false;
    while let Some(c) = chars.next() {
        if inq {
            cur.push(c);
            if c == '"' {
                inq = false;
            }
            continue;
        }
        match c {
            '"' => {
                inq = true;
                cur.push(c);
            }
            '\\' | '_' | '*' => {
                cur.push(c);
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            '[' => {
                cur.push(c);
                for n in chars.by_ref() {
                    cur.push(n);
                    if n == ']' {
                        break;
                    }
                }
            }
            ';' => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    out.push(cur);
    if out.len() > 4 {
        out.truncate(4);
    }
    out
}

fn parse_section(s: &str) -> Section {
    let mut toks: Vec<Tok> = Vec::new();
    let mut color = None;
    let mut cond = None;
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    let lit = |toks: &mut Vec<Tok>, s: &str| {
        if let Some(Tok::Lit(l)) = toks.last_mut() {
            l.push_str(s);
        } else {
            toks.push(Tok::Lit(s.to_string()));
        }
    };
    while i < cs.len() {
        let c = cs[i];
        let lower = c.to_ascii_lowercase();
        match c {
            '"' => {
                let mut j = i + 1;
                let mut t = String::new();
                while j < cs.len() && cs[j] != '"' {
                    t.push(cs[j]);
                    j += 1;
                }
                lit(&mut toks, &t);
                i = j + 1;
                continue;
            }
            '\\' => {
                if i + 1 < cs.len() {
                    lit(&mut toks, &cs[i + 1].to_string());
                }
                i += 2;
                continue;
            }
            '_' => {
                if i + 1 < cs.len() {
                    toks.push(Tok::Pad(cs[i + 1]));
                }
                i += 2;
                continue;
            }
            '*' => {
                if i + 1 < cs.len() {
                    toks.push(Tok::Fill(cs[i + 1]));
                }
                i += 2;
                continue;
            }
            '[' => {
                let mut j = i + 1;
                let mut t = String::new();
                while j < cs.len() && cs[j] != ']' {
                    t.push(cs[j]);
                    j += 1;
                }
                let tl = t.to_ascii_lowercase();
                if let Some((_, rgb)) = COLORS.iter().find(|(n, _)| *n == tl) {
                    color = Some(*rgb);
                } else if let Some(n) = tl.strip_prefix("color") {
                    if let Ok(k) = n.trim().parse::<usize>() {
                        if (1..=56).contains(&k) {
                            color = Some(PALETTE[k - 1]);
                        }
                    }
                } else if tl.starts_with('<') || tl.starts_with('>') || tl.starts_with('=') {
                    let op_len = if tl.starts_with("<=") || tl.starts_with(">=") || tl.starts_with("<>") { 2 } else { 1 };
                    if let Ok(x) = tl[op_len..].trim().parse::<f64>() {
                        cond = Some((tl[..op_len].to_string(), x));
                    }
                } else if tl.starts_with('$') {
                    // [$€-407]: currency symbol before the dash
                    let sym = t[1..].split('-').next().unwrap_or("");
                    if !sym.is_empty() {
                        lit(&mut toks, sym);
                    }
                } else if tl.chars().all(|x| x == 'h') && !tl.is_empty() {
                    toks.push(Tok::Elapsed('h', tl.len() as u8));
                } else if tl.chars().all(|x| x == 'm') && !tl.is_empty() {
                    toks.push(Tok::Elapsed('m', tl.len() as u8));
                } else if tl.chars().all(|x| x == 's') && !tl.is_empty() {
                    toks.push(Tok::Elapsed('s', tl.len() as u8));
                }
                i = j + 1;
                continue;
            }
            '0' | '#' | '?' => toks.push(Tok::Digit(c as u8)),
            '.' => {
                // .0 after seconds = fractions of a second
                let after_sec = toks.iter().rev().find(|t| !matches!(t, Tok::Lit(_))).map(|t| matches!(t, Tok::Second(_) | Tok::Elapsed('s', _))).unwrap_or(false);
                if after_sec && i + 1 < cs.len() && cs[i + 1] == '0' {
                    let mut j = i + 1;
                    while j < cs.len() && cs[j] == '0' {
                        j += 1;
                    }
                    toks.push(Tok::SubSec((j - i - 1) as u8));
                    i = j;
                    continue;
                }
                toks.push(Tok::Point)
            }
            ',' => toks.push(Tok::Comma),
            '%' => toks.push(Tok::Percent),
            '/' => {
                // a fraction only when digits come before it
                if toks.iter().any(|t| matches!(t, Tok::Digit(_))) && !toks.iter().any(is_date_tok) {
                    toks.push(Tok::Slash)
                } else {
                    lit(&mut toks, "/")
                }
            }
            '@' => toks.push(Tok::At),
            'E' | 'e' if i + 1 < cs.len() && (cs[i + 1] == '+' || cs[i + 1] == '-') && toks.iter().any(|t| matches!(t, Tok::Digit(_))) => {
                toks.push(Tok::Exp(cs[i + 1] == '+', c == 'E'));
                i += 2;
                continue;
            }
            _ if lower == 'g' && s[s.char_indices().nth(i).map(|x| x.0).unwrap()..].to_ascii_lowercase().starts_with("general") => {
                toks.push(Tok::General);
                i += 7;
                continue;
            }
            _ if lower == 'y' || lower == 'e' => {
                let n = run(&cs, i, |x| x.to_ascii_lowercase() == lower);
                toks.push(Tok::Year(if n <= 2 && lower == 'y' { 2 } else { 4 }));
                i += n;
                continue;
            }
            _ if lower == 'b' && i + 1 < cs.len() && (cs[i + 1] == '1' || cs[i + 1] == '2') => {
                i += 2;
                continue;
            }
            _ if lower == 'm' => {
                let n = run(&cs, i, |x| x.to_ascii_lowercase() == 'm');
                toks.push(Tok::Month(n.min(5) as u8));
                i += n;
                continue;
            }
            _ if lower == 'd' => {
                let n = run(&cs, i, |x| x.to_ascii_lowercase() == 'd');
                toks.push(Tok::Day(n.min(4) as u8));
                i += n;
                continue;
            }
            _ if lower == 'h' => {
                let n = run(&cs, i, |x| x.to_ascii_lowercase() == 'h');
                toks.push(Tok::Hour(n.min(2) as u8));
                i += n;
                continue;
            }
            _ if lower == 's' => {
                let n = run(&cs, i, |x| x.to_ascii_lowercase() == 's');
                toks.push(Tok::Second(n.min(2) as u8));
                i += n;
                continue;
            }
            _ if lower == 'a' => {
                let rest: String = cs[i..].iter().collect();
                let rl = rest.to_ascii_lowercase();
                if rl.starts_with("am/pm") {
                    toks.push(Tok::AmPm(rest[..5].to_string()));
                    i += 5;
                    continue;
                }
                if rl.starts_with("a/p") {
                    toks.push(Tok::AmPm(rest[..3].to_string()));
                    i += 3;
                    continue;
                }
                lit(&mut toks, &c.to_string());
            }
            _ => lit(&mut toks, &c.to_string()),
        }
        i += 1;
    }
    // m after h (or before s) is minutes
    let n = toks.len();
    for k in 0..n {
        if let Tok::Month(m) = toks[k] {
            if m <= 2 {
                let prev = toks[..k].iter().rev().find(|t| is_date_tok(t));
                let next = toks[k + 1..].iter().find(|t| is_date_tok(t));
                let after_h = matches!(prev, Some(Tok::Hour(_)) | Some(Tok::Elapsed('h', _)));
                let before_s = matches!(next, Some(Tok::Second(_)) | Some(Tok::Elapsed('s', _)));
                if after_h || before_s {
                    toks[k] = Tok::Minute(m);
                }
            }
        }
    }
    if toks.is_empty() {
        // an empty section shows nothing
    }
    Section { toks, color, cond }
}

fn run(cs: &[char], i: usize, f: impl Fn(char) -> bool) -> usize {
    let mut n = 0;
    while i + n < cs.len() && f(cs[i + n]) {
        n += 1;
    }
    n
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December"];
const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

fn format_section(s: &Section, v: f64, abs_sign: bool, date1904: bool, width: usize) -> Formatted {
    let mut f = Formatted::default();
    if s.toks.iter().any(is_date_tok) {
        return format_date(s, v, date1904);
    }
    if s.toks.iter().any(|t| matches!(t, Tok::General)) {
        let x = if abs_sign { v.abs() } else { v };
        for t in &s.toks {
            match t {
                Tok::General => f.text.push_str(&general(x, width)),
                Tok::Lit(l) => f.text.push_str(l),
                Tok::Pad(c) => {
                    f.pads.push((f.text.len(), *c));
                    f.text.push(' ');
                }
                Tok::Fill(c) => f.fill = Some((f.text.len(), *c)),
                Tok::At => f.text.push_str(&general(x, width)),
                _ => {}
            }
        }
        f.hash_on_overflow = true;
        return f;
    }
    if !s.toks.iter().any(|t| matches!(t, Tok::Digit(_))) {
        // literal-only section (e.g. "-" for zero, or a text-only format applied to a number)
        let mut neg = !abs_sign && v < 0.0;
        if s.toks.iter().any(|t| matches!(t, Tok::At)) {
            let x = if abs_sign { v.abs() } else { v };
            for t in &s.toks {
                match t {
                    Tok::At => f.text.push_str(&general(x, width)),
                    Tok::Lit(l) => f.text.push_str(l),
                    _ => {}
                }
            }
            return f;
        }
        for t in &s.toks {
            match t {
                Tok::Lit(l) => {
                    if neg {
                        f.text.push('-');
                        neg = false;
                    }
                    f.text.push_str(l)
                }
                Tok::Percent => f.text.push('%'),
                Tok::Pad(c) => {
                    f.pads.push((f.text.len(), *c));
                    f.text.push(' ');
                }
                Tok::Fill(c) => f.fill = Some((f.text.len(), *c)),
                _ => {}
            }
        }
        return f;
    }
    f.hash_on_overflow = true;
    let exp_pos = s.toks.iter().position(|t| matches!(t, Tok::Exp(..)));
    let slash_pos = s.toks.iter().position(|t| matches!(t, Tok::Slash));
    let neg = v < 0.0 && !abs_sign;
    let a = v.abs();
    let mut body = if let Some(ep) = exp_pos {
        format_scientific(&s.toks, ep, a)
    } else if let Some(sp) = slash_pos {
        format_fraction(&s.toks, sp, a)
    } else {
        format_plain(&s.toks, a)
    };
    // negative numbers in a single-section format carry a leading minus sign
    if neg {
        body.text.insert(0, '-');
        for p in body.pads.iter_mut() {
            p.0 += 1;
        }
        if let Some(fl) = body.fill.as_mut() {
            fl.0 += 1;
        }
    }
    body.hash_on_overflow = true;
    body
}

fn group3(ip: &str) -> String {
    let b = ip.as_bytes();
    let mut out = String::with_capacity(ip.len() + ip.len() / 3);
    for (i, c) in b.iter().enumerate() {
        if i > 0 && (b.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c as char);
    }
    out
}

/// Plain numbers: integer placeholders, optional point and decimals, %, commas.
fn format_plain(toks: &[Tok], a: f64) -> Formatted {
    let point = toks.iter().position(|t| matches!(t, Tok::Point));
    let int_end = point.unwrap_or(toks.len());
    // trailing commas right after the last digit placeholder scale by 1000 each
    let last_digit = toks.iter().rposition(|t| matches!(t, Tok::Digit(_))).unwrap_or(0);
    let mut scale_commas = 0;
    let mut k = last_digit + 1;
    while k < toks.len() && matches!(toks[k], Tok::Comma | Tok::Point) {
        if matches!(toks[k], Tok::Comma) {
            scale_commas += 1;
        }
        k += 1;
    }
    let first_digit = toks.iter().position(|t| matches!(t, Tok::Digit(_))).unwrap_or(0);
    let grouping = toks[first_digit..int_end.min(toks.len())].iter().enumerate().any(|(i, t)| {
        matches!(t, Tok::Comma) && toks[first_digit + i + 1..int_end.min(toks.len())].iter().any(|x| matches!(x, Tok::Digit(_)))
    });
    let percents = toks.iter().filter(|t| matches!(t, Tok::Percent)).count() as i32;
    let mut x = a * 100f64.powi(percents);
    if scale_commas > 0 {
        x /= 1000f64.powi(scale_commas);
    }
    let int_ph: Vec<u8> = toks[..int_end].iter().filter_map(|t| if let Tok::Digit(d) = t { Some(*d) } else { None }).collect();
    let frac_ph: Vec<u8> = if let Some(p) = point {
        toks[p + 1..].iter().filter_map(|t| if let Tok::Digit(d) = t { Some(*d) } else { None }).collect()
    } else {
        vec![]
    };
    let (ip, fp) = fixed(x, frac_ph.len());
    let mut ip = if ip == "0" && !int_ph.contains(&b'0') { String::new() } else { ip };
    // integer digits laid onto placeholders right to left
    let n_ph = int_ph.len();
    let mut int_out: Vec<String> = vec![String::new(); n_ph];
    {
        let digits: Vec<char> = ip.chars().collect();
        let mut di = digits.len() as i64 - 1;
        for pi in (0..n_ph).rev() {
            if di >= 0 {
                if pi == 0 {
                    int_out[pi] = digits[..=di as usize].iter().collect();
                    di = -1;
                } else {
                    int_out[pi] = digits[di as usize].to_string();
                    di -= 1;
                }
            } else {
                int_out[pi] = match int_ph[pi] {
                    b'0' => "0".into(),
                    b'?' => " ".into(),
                    _ => String::new(),
                };
            }
        }
    }
    if grouping {
        // group the digits written into the integer placeholders as one string
        let joined: String = int_out.concat();
        let lead_spaces = joined.len() - joined.trim_start().len();
        let digits_only: String = joined.trim_start().to_string();
        let grouped = group3(&digits_only);
        int_out = vec![String::new(); n_ph];
        if n_ph > 0 {
            int_out[0] = format!("{}{}", " ".repeat(lead_spaces), grouped);
        }
        ip = String::new();
    }
    let _ = ip;
    // fraction digits: trailing zeros on # vanish, on ? become spaces
    let fdig: Vec<char> = fp.chars().collect();
    let mut frac_out: Vec<String> = fdig.iter().map(|c| c.to_string()).collect();
    for i in (0..frac_ph.len()).rev() {
        if fdig[i] == '0' {
            match frac_ph[i] {
                b'#' => frac_out[i] = String::new(),
                b'?' => frac_out[i] = " ".into(),
                _ => break,
            }
        } else {
            break;
        }
    }
    let mut out = Formatted::default();
    let mut ii = 0;
    let mut fi = 0;
    let mut after_point = false;
    let mut written_int = false;
    for (pos, t) in toks.iter().enumerate() {
        match t {
            Tok::Digit(_) => {
                if !after_point {
                    if grouping {
                        if !written_int {
                            out.text.push_str(&int_out[0]);
                            written_int = true;
                        }
                    } else {
                        out.text.push_str(&int_out[ii]);
                    }
                    ii += 1;
                } else {
                    out.text.push_str(&frac_out[fi]);
                    fi += 1;
                }
            }
            Tok::Point => {
                if !after_point {
                    after_point = true;
                    out.text.push('.');
                }
            }
            Tok::Comma => {
                // grouping and scaling commas print nothing; a comma outside digits is literal
                let inside = pos > first_digit && pos < last_digit;
                if !inside && pos < first_digit {
                    out.text.push(',');
                }
            }
            Tok::Percent => out.text.push('%'),
            Tok::Lit(l) => out.text.push_str(l),
            Tok::Pad(c) => {
                out.pads.push((out.text.len(), *c));
                out.text.push(' ');
            }
            Tok::Fill(c) => out.fill = Some((out.text.len(), *c)),
            Tok::At => {}
            _ => {}
        }
    }
    out
}

fn format_scientific(toks: &[Tok], ep: usize, a: f64) -> Formatted {
    let (plus, upper) = if let Tok::Exp(p, u) = toks[ep] { (p, u) } else { (true, true) };
    let mant_toks = &toks[..ep];
    let exp_toks = &toks[ep + 1..];
    let point = mant_toks.iter().position(|t| matches!(t, Tok::Point));
    let int_ph = mant_toks[..point.unwrap_or(mant_toks.len())].iter().filter(|t| matches!(t, Tok::Digit(_))).count().max(1);
    let frac_n = point.map(|p| mant_toks[p + 1..].iter().filter(|t| matches!(t, Tok::Digit(_))).count()).unwrap_or(0);
    let exp_digits = exp_toks.iter().filter(|t| matches!(t, Tok::Digit(_))).count().max(1);
    // engineering style: exponent a multiple of the integer placeholder count
    let mut e = if a == 0.0 { 0 } else { a.log10().floor() as i32 };
    if a != 0.0 {
        let (_, ee) = sig15(a);
        e = ee - 1;
    }
    let step = int_ph as i32;
    if step > 1 {
        e = e.div_euclid(step) * step;
    }
    let mut m = if a == 0.0 { 0.0 } else { a / 10f64.powi(e) };
    let (mut ip, mut fp) = fixed(m, frac_n);
    if ip.len() > int_ph && a != 0.0 && step == 1 {
        e += 1;
        m = a / 10f64.powi(e);
        let r = fixed(m, frac_n);
        ip = r.0;
        fp = r.1;
    }
    let mut out = Formatted::default();
    // mantissa via the plain formatter on a rebuilt token list
    let mant = format_plain_digits(mant_toks, &ip, &fp);
    out.text.push_str(&mant);
    out.text.push(if upper { 'E' } else { 'e' });
    if e < 0 {
        out.text.push('-');
    } else if plus {
        out.text.push('+');
    }
    let es = format!("{:0width$}", e.abs(), width = exp_digits);
    out.text.push_str(&es);
    for t in exp_toks {
        if let Tok::Lit(l) = t {
            out.text.push_str(l);
        }
    }
    out
}

/// Lay already-rounded integer/fraction digit strings onto placeholder tokens.
fn format_plain_digits(toks: &[Tok], ip: &str, fp: &str) -> String {
    let point = toks.iter().position(|t| matches!(t, Tok::Point));
    let int_end = point.unwrap_or(toks.len());
    let int_ph: Vec<u8> = toks[..int_end].iter().filter_map(|t| if let Tok::Digit(d) = t { Some(*d) } else { None }).collect();
    let frac_ph: Vec<u8> = point.map(|p| toks[p + 1..].iter().filter_map(|t| if let Tok::Digit(d) = t { Some(*d) } else { None }).collect()).unwrap_or_default();
    let ip = if ip == "0" && !int_ph.contains(&b'0') { "" } else { ip };
    let digits: Vec<char> = ip.chars().collect();
    let n_ph = int_ph.len();
    let mut int_out = vec![String::new(); n_ph];
    let mut di = digits.len() as i64 - 1;
    for pi in (0..n_ph).rev() {
        if di >= 0 {
            if pi == 0 {
                int_out[pi] = digits[..=di as usize].iter().collect();
                di = -1;
            } else {
                int_out[pi] = digits[di as usize].to_string();
                di -= 1;
            }
        } else {
            int_out[pi] = match int_ph[pi] {
                b'0' => "0".into(),
                b'?' => " ".into(),
                _ => String::new(),
            };
        }
    }
    let fdig: Vec<char> = fp.chars().collect();
    let mut frac_out: Vec<String> = fdig.iter().map(|c| c.to_string()).collect();
    for i in (0..frac_ph.len().min(fdig.len())).rev() {
        if fdig[i] == '0' {
            match frac_ph[i] {
                b'#' => frac_out[i] = String::new(),
                b'?' => frac_out[i] = " ".into(),
                _ => break,
            }
        } else {
            break;
        }
    }
    let mut s = String::new();
    let (mut ii, mut fi, mut after) = (0, 0, false);
    for t in toks {
        match t {
            Tok::Digit(_) => {
                if !after {
                    s.push_str(&int_out[ii]);
                    ii += 1;
                } else {
                    if fi < frac_out.len() {
                        s.push_str(&frac_out[fi]);
                    }
                    fi += 1;
                }
            }
            Tok::Point => {
                after = true;
                s.push('.');
            }
            Tok::Lit(l) => s.push_str(l),
            Tok::Percent => s.push('%'),
            _ => {}
        }
    }
    s
}

fn format_fraction(toks: &[Tok], sp: usize, a: f64) -> Formatted {
    // integer part placeholders (if separated from the numerator by a literal space)
    let before = &toks[..sp];
    let after = &toks[sp + 1..];
    // numerator digits: the run of digits right before the slash
    let mut num_start = sp;
    while num_start > 0 && matches!(before[num_start - 1], Tok::Digit(_)) {
        num_start -= 1;
    }
    let int_toks = &before[..num_start];
    let has_int = int_toks.iter().any(|t| matches!(t, Tok::Digit(_)));
    let num_ph = sp - num_start;
    // denominator: digits (placeholders) or a fixed number
    let mut den_ph = 0;
    let mut fixed_den: Option<u64> = None;
    let mut den_lit = String::new();
    let mut k = 0;
    while k < after.len() {
        match &after[k] {
            Tok::Digit(d) => {
                den_ph += 1;
                den_lit.push(*d as char);
            }
            Tok::Lit(l) if l.chars().all(|c| c.is_ascii_digit()) && den_ph == 0 => {
                den_lit.push_str(l);
                fixed_den = l.parse().ok();
            }
            _ => break,
        }
        k += 1;
    }
    if fixed_den.is_none() && den_lit.chars().all(|c| c.is_ascii_digit()) && !den_lit.is_empty() && den_ph > 0
        && den_lit.chars().all(|c| c != '0') && den_lit.chars().any(|c| c.is_ascii_digit())
    {
        // e.g. "?/4" parses 4 as digits? no: digits in format are placeholders only when 0 # ?
    }
    let rest = &after[k..];
    let (whole, frac) = if has_int { (a.trunc(), a - a.trunc()) } else { (0.0, a) };
    let (mut n, mut d) = if let Some(fd) = fixed_den {
        ((frac * fd as f64).round() as u64, fd)
    } else {
        best_fraction(frac, 10u64.pow(den_ph.max(1) as u32) - 1)
    };
    let mut whole = whole;
    if has_int && d > 0 && n >= d {
        whole += (n / d) as f64;
        n %= d;
    }
    if !has_int && fixed_den.is_none() && n == 0 {
        d = 1;
    }
    let mut out = Formatted::default();
    for t in int_toks {
        match t {
            Tok::Lit(l) => out.text.push_str(l),
            _ => {}
        }
    }
    let int_str = if has_int {
        if whole == 0.0 && n != 0 {
            String::new()
        } else {
            format!("{}", whole as i64)
        }
    } else {
        String::new()
    };
    // rebuild: integer text, separator literal, numerator/denominator padded to placeholders
    let mut s = String::new();
    if has_int {
        let int_digits: Vec<&Tok> = int_toks.iter().filter(|t| matches!(t, Tok::Digit(_))).collect();
        let pad = int_digits.len().saturating_sub(int_str.len());
        if int_str.is_empty() {
            s.push_str(&" ".repeat(int_digits.len().max(1)));
        } else {
            s.push_str(&" ".repeat(pad));
            s.push_str(&int_str);
        }
        // literal between integer and numerator (usually a space)
        let sep: String = before[..num_start].iter().rev().take_while(|t| matches!(t, Tok::Lit(_))).map(|t| if let Tok::Lit(l) = t { l.clone() } else { String::new() }).collect::<Vec<_>>().into_iter().rev().collect();
        s.push_str(if sep.is_empty() { " " } else { &sep });
    }
    if n == 0 && has_int {
        // no fractional part: blanks where the fraction would be
        let w = num_ph + 1 + den_lit.len().max(den_ph);
        if whole == 0.0 {
            s = format!("{}0", " ".repeat(s.len().saturating_sub(1)));
        }
        s.push_str(&" ".repeat(w));
    } else {
        let ns = n.to_string();
        let ds = d.to_string();
        s.push_str(&" ".repeat(num_ph.saturating_sub(ns.len())));
        s.push_str(&ns);
        s.push('/');
        s.push_str(&ds);
        if fixed_den.is_none() {
            s.push_str(&" ".repeat(den_ph.saturating_sub(ds.len())));
        }
    }
    let _ = int_str;
    out.text = s;
    for t in rest {
        if let Tok::Lit(l) = t {
            out.text.push_str(l);
        }
    }
    out
}

/// Closest fraction with denominator <= max_den (Stern-Brocot / continued fractions).
pub fn best_fraction(x: f64, max_den: u64) -> (u64, u64) {
    if x <= 0.0 {
        return (0, 1);
    }
    let (mut a0, mut b0, mut a1, mut b1) = (0u64, 1u64, 1u64, 0u64);
    let mut v = x;
    loop {
        let q = v.floor();
        let qi = q as u64;
        let a2 = qi.saturating_mul(a1).saturating_add(a0);
        let b2 = qi.saturating_mul(b1).saturating_add(b0);
        if b2 > max_den {
            // best semiconvergent
            let k = (max_den - b0) / b1.max(1);
            let (ca, cb) = (a0 + k * a1, b0 + k * b1);
            let e1 = (x - a1 as f64 / b1 as f64).abs();
            let e2 = (x - ca as f64 / cb as f64).abs();
            return if b1 == 0 || (cb > 0 && e2 < e1) { (ca, cb) } else { (a1, b1) };
        }
        a0 = a1;
        b0 = b1;
        a1 = a2;
        b1 = b2;
        let f = v - q;
        if f < 1e-12 || (x - a1 as f64 / b1 as f64).abs() < 1e-12 {
            return (a1, b1);
        }
        v = 1.0 / f;
    }
}

fn format_date(s: &Section, v: f64, date1904: bool) -> Formatted {
    let mut f = Formatted { hash_on_overflow: true, ..Default::default() };
    if v < 0.0 || v >= 2958466.0 {
        f.text = "#".repeat(10);
        f.fill = Some((0, '#'));
        f.text.clear();
        return f;
    }
    let subsec = s.toks.iter().filter_map(|t| if let Tok::SubSec(n) = t { Some(*n) } else { None }).max().unwrap_or(0);
    let has_ampm = s.toks.iter().any(|t| matches!(t, Tok::AmPm(_)));
    // round to the shown precision of seconds
    let scale = 10f64.powi(subsec as i32);
    let total_units = (v * 86400.0 * scale).round();
    let day_units = 86400.0 * scale;
    let days = (total_units / day_units).floor();
    let rem = total_units - days * day_units;
    let secs_total = (rem / scale).floor() as u64;
    let sub = (rem - secs_total as f64 * scale).round() as u64;
    let (y, mo, d) = dates::serial_to_ymd(days, date1904).unwrap_or((1900, 1, 0));
    let h = (secs_total / 3600) as u32;
    let mi = ((secs_total / 60) % 60) as u32;
    let se = (secs_total % 60) as u32;
    let wd = dates::weekday(days, date1904) as usize;
    for t in &s.toks {
        match t {
            Tok::Year(n) => {
                if *n == 2 {
                    f.text.push_str(&format!("{:02}", y.rem_euclid(100)));
                } else {
                    f.text.push_str(&format!("{:04}", y));
                }
            }
            Tok::Month(n) => match n {
                1 => f.text.push_str(&mo.to_string()),
                2 => f.text.push_str(&format!("{:02}", mo)),
                3 => f.text.push_str(&MONTHS[(mo - 1) as usize][..3]),
                4 => f.text.push_str(MONTHS[(mo - 1) as usize]),
                _ => f.text.push_str(&MONTHS[(mo - 1) as usize][..1]),
            },
            Tok::Day(n) => match n {
                1 => f.text.push_str(&d.to_string()),
                2 => f.text.push_str(&format!("{:02}", d)),
                3 => f.text.push_str(&DAYS[wd - 1][..3]),
                _ => f.text.push_str(DAYS[wd - 1]),
            },
            Tok::Hour(n) => {
                let hh = if has_ampm {
                    let x = h % 12;
                    if x == 0 { 12 } else { x }
                } else {
                    h
                };
                if *n >= 2 {
                    f.text.push_str(&format!("{:02}", hh));
                } else {
                    f.text.push_str(&hh.to_string());
                }
            }
            Tok::Minute(n) => {
                if *n >= 2 {
                    f.text.push_str(&format!("{:02}", mi));
                } else {
                    f.text.push_str(&mi.to_string());
                }
            }
            Tok::Second(n) => {
                if *n >= 2 {
                    f.text.push_str(&format!("{:02}", se));
                } else {
                    f.text.push_str(&se.to_string());
                }
            }
            Tok::SubSec(n) => {
                f.text.push('.');
                f.text.push_str(&format!("{:0width$}", sub, width = *n as usize));
            }
            Tok::Elapsed(k, n) => {
                let total_secs = days as u64 * 86400 + secs_total;
                let val = match k {
                    'h' => total_secs / 3600,
                    'm' => total_secs / 60,
                    _ => total_secs,
                };
                f.text.push_str(&format!("{:0width$}", val, width = *n as usize));
            }
            Tok::AmPm(src) => {
                let pm = h >= 12;
                let out = if src.len() == 5 {
                    let base = if pm { "PM" } else { "AM" };
                    if src.starts_with('a') || src.starts_with('p') {
                        base.to_lowercase()
                    } else {
                        base.to_string()
                    }
                } else {
                    let c = if pm { 'P' } else { 'A' };
                    if src.starts_with('a') { c.to_ascii_lowercase().to_string() } else { c.to_string() }
                };
                f.text.push_str(&out);
            }
            Tok::Lit(l) => f.text.push_str(l),
            Tok::Pad(c) => {
                f.pads.push((f.text.len(), *c));
                f.text.push(' ');
            }
            Tok::Fill(c) => f.fill = Some((f.text.len(), *c)),
            Tok::Digit(b'0') => f.text.push('0'),
            Tok::Point => f.text.push('.'),
            Tok::Comma => f.text.push(','),
            Tok::Percent => f.text.push('%'),
            Tok::Slash => f.text.push('/'),
            _ => {}
        }
    }
    f
}

// --------------------------------------------------------------------------- cache & helpers

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static CACHE: RefCell<HashMap<String, std::rc::Rc<Format>>> = RefCell::new(HashMap::new());
}

pub fn get(code: &str) -> std::rc::Rc<Format> {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(f) = c.get(code) {
            return f.clone();
        }
        let f = std::rc::Rc::new(Format::parse(code));
        if c.len() > 4096 {
            c.clear();
        }
        c.insert(code.to_string(), f.clone());
        f
    })
}

/// Format a number with a code (the status bar and the module-level format_number).
pub fn format_number(v: f64, code: &str, date1904: bool) -> String {
    let f = get(code).number(v, date1904, 11);
    f.text
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fmt(v: f64, c: &str) -> String {
        format_number(v, c, false)
    }
    #[test]
    fn plain() {
        assert_eq!(fmt(1234.5, "#,##0.00"), "1,234.50");
        assert_eq!(fmt(-1234.5, "#,##0.00"), "-1,234.50");
        assert_eq!(fmt(0.5, "0%"), "50%");
        assert_eq!(fmt(0.125, "0.00%"), "12.50%");
        assert_eq!(fmt(1234567.0, "#,##0,"), "1,235");
        assert_eq!(fmt(0.5, "#.##"), ".5");
        assert_eq!(fmt(5.0, "000"), "005");
        assert_eq!(fmt(2.5, "0"), "3");
        assert_eq!(fmt(-2.5, "0"), "-3");
        assert_eq!(fmt(1.005, "0.00"), "1.01");
        assert_eq!(fmt(1234.5, "\"$\"#,##0.00_);[Red]\\(\"$\"#,##0.00\\)"), "$1,234.50 ");
        assert_eq!(fmt(-1234.5, "\"$\"#,##0.00_);[Red]\\(\"$\"#,##0.00\\)"), "($1,234.50)");
        assert_eq!(fmt(0.0, "0.00;-0.00;\"zero\""), "zero");
    }
    #[test]
    fn sci() {
        assert_eq!(fmt(100000.0, "0.00E+00"), "1.00E+05");
        assert_eq!(fmt(0.000123, "0.00E+00"), "1.23E-04");
        assert_eq!(fmt(12345.0, "##0.0E+0"), "12.3E+3");
    }
    #[test]
    fn dates() {
        assert_eq!(fmt(46024.0, "m/d/yyyy"), "1/2/2026");
        assert_eq!(fmt(45000.0, "m/d/yyyy"), "3/15/2023");
        assert_eq!(fmt(46024.5, "dddd, mmmm d, yyyy"), "Friday, January 2, 2026");
        assert_eq!(fmt(0.5208333333333334, "h:mm"), "12:30");
        assert_eq!(fmt(0.5208333333333334, "h:mm AM/PM"), "12:30 PM");
        assert_eq!(fmt(1.5, "[h]:mm:ss"), "36:00:00");
        assert_eq!(fmt(46024.0, "d-mmm-yy"), "2-Jan-26");
        assert_eq!(fmt(0.5, "[$-F400]h:mm:ss AM/PM"), "12:00:00 PM");
    }
    #[test]
    fn fractions() {
        assert_eq!(fmt(1.5, "# ?/?"), "1 1/2");
        assert_eq!(fmt(0.75, "# ?/4"), "  3/4");
        assert_eq!(fmt(3.14159, "# ??/??"), "3 14/99");
    }
    #[test]
    fn gen() {
        assert_eq!(general(1.0 / 3.0, 11), "0.333333333");
        assert_eq!(general(123456789012.0, 11), "1.23457E+11");
        assert_eq!(general(1e-10, 11), "1E-10");
        assert_eq!(general(4.5, 11), "4.5");
        assert_eq!(general(-1234.5, 11), "-1234.5");
        assert_eq!(num_to_text(1.0 / 3.0), "0.333333333333333");
        assert_eq!(num_to_text(0.1 + 0.2), "0.3");
        assert_eq!(num_to_text(1e21), "1E+21");
        assert_eq!(num_to_text(12.5), "12.5");
    }
}
