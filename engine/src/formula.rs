//! Formula text: tokens, parsing to an expression tree, and reference rewriting.
//!
//! Rewrites (copying a formula, inserting rows, renaming a sheet) work on the token list
//! of the original text, so everything but the references stays exactly as typed.

use crate::cellref::{col_index, col_name, parse_cell_abs, quote_sheet, MAX_COLS, MAX_ROWS};
use crate::value::XlErr;
use std::rc::Rc;

// --------------------------------------------------------------------------- tokens

#[derive(Clone, Debug, PartialEq)]
pub enum TokKind {
    Num,
    Str,
    Bool,
    Err,
    Ref,      // a reference, with an optional sheet prefix
    Name,     // a defined name or LET/LAMBDA variable (optionally sheet-qualified)
    Func,     // a function name (the '(' follows as its own token)
    Op,       // + - * / ^ & = <> < > <= >= : %
    LParen,
    RParen,
    Comma,
    Semi,
    LBrace,
    RBrace,
    Space,
    Spill,    // # after a reference
    At,       // @ implicit intersection
    Struct,   // Table[...] structured reference
    Ext,      // [1]Sheet!A1 external reference
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub kind: TokKind,
    pub text: String,
    pub start: usize,
    pub end: usize,
}

/// A parsed reference token.
#[derive(Clone, Debug, PartialEq)]
pub struct RefTok {
    pub sheet: Option<String>,
    pub sheet2: Option<String>, // 3D: Sheet1:Sheet3!A1
    pub r1: u32,
    pub c1: u32,
    pub r2: u32,
    pub c2: u32,
    pub ra1: bool,
    pub ca1: bool,
    pub ra2: bool,
    pub ca2: bool,
    pub kind: RefKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RefKind {
    Cell,
    Area,
    Cols,
    Rows,
    Err, // Sheet1!#REF!
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '\\'
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '.' || c == '\\' || c == '?'
}

pub fn tokenize(src: &str) -> Result<Vec<Token>, String> {
    let cs: Vec<(usize, char)> = src.char_indices().collect();
    let n = cs.len();
    let at = |i: usize| -> usize { if i < n { cs[i].0 } else { src.len() } };
    let ch = |i: usize| -> char { if i < n { cs[i].1 } else { '\0' } };
    let mut out: Vec<Token> = Vec::new();
    let mut i = 0;
    let push = |out: &mut Vec<Token>, kind: TokKind, s: usize, e: usize| {
        out.push(Token { kind, text: src[s..e].to_string(), start: s, end: e });
    };
    while i < n {
        let c = ch(i);
        let s = at(i);
        if c.is_whitespace() {
            let mut j = i;
            while j < n && ch(j).is_whitespace() {
                j += 1;
            }
            push(&mut out, TokKind::Space, s, at(j));
            i = j;
            continue;
        }
        if c == '"' {
            let mut j = i + 1;
            loop {
                if j >= n {
                    return Err("A closing quotation mark is missing.".into());
                }
                if ch(j) == '"' {
                    if ch(j + 1) == '"' {
                        j += 2;
                        continue;
                    }
                    break;
                }
                j += 1;
            }
            push(&mut out, TokKind::Str, s, at(j + 1));
            i = j + 1;
            continue;
        }
        if c == '#' {
            // error literal, or the spill operator after a reference / name / ')'
            let rest = &src[s..];
            if let Some(e) = XlErr::ALL.iter().find(|e| rest.to_ascii_uppercase().starts_with(e.text())) {
                let len = e.text().chars().count();
                push(&mut out, TokKind::Err, s, at(i + len));
                i += len;
                continue;
            }
            if matches!(out.last().map(|t| &t.kind), Some(TokKind::Ref) | Some(TokKind::Name) | Some(TokKind::RParen)) {
                push(&mut out, TokKind::Spill, s, at(i + 1));
                i += 1;
                continue;
            }
            return Err(format!("Unexpected '#' at {}", s));
        }
        if c.is_ascii_digit() || (c == '.' && ch(i + 1).is_ascii_digit()) {
            // a row range (3:5) or a number
            if let Some(len) = match_ref(&cs, i, src) {
                push(&mut out, TokKind::Ref, s, at(i + len));
                i += len;
                continue;
            }
            let mut j = i;
            while j < n && ch(j).is_ascii_digit() {
                j += 1;
            }
            if ch(j) == '.' {
                j += 1;
                while j < n && ch(j).is_ascii_digit() {
                    j += 1;
                }
            }
            if (ch(j) == 'e' || ch(j) == 'E') && (ch(j + 1).is_ascii_digit() || ((ch(j + 1) == '+' || ch(j + 1) == '-') && ch(j + 2).is_ascii_digit())) {
                j += 2;
                while j < n && ch(j).is_ascii_digit() {
                    j += 1;
                }
            }
            push(&mut out, TokKind::Num, s, at(j));
            i = j;
            continue;
        }
        if c == '[' {
            // a structured reference inside a table: [@Col], [Col], [[#This Row],[Col]]
            {
                let mut depth = 0;
                let mut k = i;
                while k < n {
                    if ch(k) == '[' {
                        depth += 1;
                    } else if ch(k) == ']' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    } else if ch(k) == '\'' {
                        k += 1;
                    }
                    k += 1;
                }
                let after = ch(k + 1);
                let starts = matches!(ch(i + 1), '@' | '[' | '#');
                if k < n && (starts || !(is_name_start(after) || after == '\'' || after.is_ascii_digit())) {
                    push(&mut out, TokKind::Struct, s, at(k + 1));
                    i = k + 1;
                    continue;
                }
            }
            // external workbook reference: [1]Sheet1!A1 or ['path']...
            let mut j = i + 1;
            while j < n && ch(j) != ']' {
                j += 1;
            }
            let mut k = j + 1;
            // the rest of the reference up to an operator
            while k < n && !matches!(ch(k), '+' | '-' | '*' | '/' | '^' | '&' | '=' | '<' | '>' | ',' | ')' | ' ' | ';' | '}') {
                if ch(k) == '\'' {
                    k += 1;
                    while k < n && !(ch(k) == '\'' && ch(k + 1) != '\'') {
                        if ch(k) == '\'' {
                            k += 1;
                        }
                        k += 1;
                    }
                }
                k += 1;
            }
            push(&mut out, TokKind::Ext, s, at(k.min(n)));
            i = k.min(n);
            continue;
        }
        if c == '\'' && ch(i + 1) == '[' {
            // '[1]Sheet name'!A1: another workbook's sheet whose name needs quotes
            let mut j = i + 1;
            while j < n && !(ch(j) == '\'' && ch(j + 1) != '\'') {
                if ch(j) == '\'' {
                    j += 1;
                }
                j += 1;
            }
            if j < n && ch(j + 1) == '!' {
                let mut k = j + 2;
                while k < n && !matches!(ch(k), '+' | '-' | '*' | '/' | '^' | '&' | '=' | '<' | '>' | ',' | ')' | ' ' | ';' | '}') {
                    k += 1;
                }
                push(&mut out, TokKind::Ext, s, at(k.min(n)));
                i = k.min(n);
                continue;
            }
        }
        if c == '\'' || is_name_start(c) || c == '$' {
            if let Some(len) = match_ref(&cs, i, src) {
                push(&mut out, TokKind::Ref, s, at(i + len));
                i += len;
                continue;
            }
            if c == '\'' {
                // 'Sheet name'!Name
                let mut j = i + 1;
                loop {
                    if j >= n {
                        return Err("A closing quotation mark is missing.".into());
                    }
                    if ch(j) == '\'' {
                        if ch(j + 1) == '\'' {
                            j += 2;
                            continue;
                        }
                        break;
                    }
                    j += 1;
                }
                if ch(j + 1) == '!' {
                    let mut k = j + 2;
                    if k < n && ch(k) == '#' {
                        // 'Sheet'!#REF!
                        let rest = &src[at(k)..];
                        if rest.to_ascii_uppercase().starts_with("#REF!") {
                            push(&mut out, TokKind::Ref, s, at(k + 5));
                            i = k + 5;
                            continue;
                        }
                    }
                    while k < n && is_name_char(ch(k)) {
                        k += 1;
                    }
                    push(&mut out, TokKind::Name, s, at(k));
                    i = k;
                    continue;
                }
                return Err("Unexpected quotation mark.".into());
            }
            if c == '$' {
                return Err("A reference isn't valid.".into());
            }
            let mut j = i;
            while j < n && is_name_char(ch(j)) {
                j += 1;
            }
            // sheet-qualified name: Sheet1!Name or Sheet1!#REF!
            if ch(j) == '!' {
                let mut k = j + 1;
                let rest = &src[at(k)..];
                if rest.to_ascii_uppercase().starts_with("#REF!") {
                    push(&mut out, TokKind::Ref, s, at(k + 5));
                    i = k + 5;
                    continue;
                }
                while k < n && is_name_char(ch(k)) {
                    k += 1;
                }
                push(&mut out, TokKind::Name, s, at(k));
                i = k;
                continue;
            }
            let word = &src[s..at(j)];
            // structured reference: Table1[Column] / Table1[[#Headers],[A]:[B]]
            if ch(j) == '[' {
                let mut depth = 0;
                let mut k = j;
                while k < n {
                    if ch(k) == '[' {
                        depth += 1;
                    } else if ch(k) == ']' {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    } else if ch(k) == '\'' {
                        k += 1;
                    }
                    k += 1;
                }
                push(&mut out, TokKind::Struct, s, at((k + 1).min(n)));
                i = (k + 1).min(n);
                continue;
            }
            let mut k = j;
            while k < n && ch(k) == ' ' {
                k += 1;
            }
            if ch(k) == '(' {
                push(&mut out, TokKind::Func, s, at(j));
            } else if word.eq_ignore_ascii_case("TRUE") || word.eq_ignore_ascii_case("FALSE") {
                push(&mut out, TokKind::Bool, s, at(j));
            } else {
                push(&mut out, TokKind::Name, s, at(j));
            }
            i = j;
            continue;
        }
        let two = if i + 1 < n { &src[s..at(i + 2)] } else { "" };
        if two == "<=" || two == ">=" || two == "<>" {
            push(&mut out, TokKind::Op, s, at(i + 2));
            i += 2;
            continue;
        }
        let kind = match c {
            '+' | '-' | '*' | '/' | '^' | '&' | '=' | '<' | '>' | ':' | '%' => TokKind::Op,
            '(' => TokKind::LParen,
            ')' => TokKind::RParen,
            ',' => TokKind::Comma,
            ';' => TokKind::Semi,
            '{' => TokKind::LBrace,
            '}' => TokKind::RBrace,
            '@' => TokKind::At,
            _ => return Err(format!("Unexpected character '{}'.", c)),
        };
        push(&mut out, kind, s, at(i + 1));
        i += 1;
    }
    Ok(out)
}

/// Length in chars of a reference starting at i, or None.
fn match_ref(cs: &[(usize, char)], i: usize, src: &str) -> Option<usize> {
    let n = cs.len();
    let ch = |k: usize| -> char { if k < n { cs[k].1 } else { '\0' } };
    let mut j = i;
    // optional sheet prefix
    let mut had_sheet = false;
    if ch(j) == '\'' {
        let mut k = j + 1;
        loop {
            if k >= n {
                return None;
            }
            if ch(k) == '\'' {
                if ch(k + 1) == '\'' {
                    k += 2;
                    continue;
                }
                break;
            }
            k += 1;
        }
        if ch(k + 1) != '!' {
            return None;
        }
        j = k + 2;
        had_sheet = true;
    } else {
        // Sheet1! or Sheet1:Sheet3!
        let mut k = j;
        while k < n && (is_name_char(ch(k)) || ch(k) == ':' && k > j) {
            if ch(k) == ':' && !is_name_start(ch(k + 1)) && !ch(k + 1).is_ascii_digit() {
                break;
            }
            k += 1;
        }
        if k > j && ch(k) == '!' && !src[cs[j].0..cs.get(k).map(|x| x.0).unwrap_or(src.len())].starts_with('$') {
            j = k + 1;
            had_sheet = true;
        }
    }
    let start = j;
    // first part: cell, column or row
    let part = |p: usize| -> Option<(usize, u8)> {
        // returns (len, kind) kind: 1 cell, 2 col, 3 row
        let mut q = p;
        if ch(q) == '$' {
            q += 1;
        }
        let ls = q;
        while q < n && ch(q).is_ascii_alphabetic() {
            q += 1;
        }
        let letters = q - ls;
        let after_letters = q;
        if letters > 0 && ch(q) == '$' {
            q += 1;
        }
        let ds = q;
        while q < n && ch(q).is_ascii_digit() {
            q += 1;
        }
        let digits = q - ds;
        if letters > 0 && letters <= 3 && digits > 0 {
            let text: String = cs[p..q].iter().map(|x| x.1).collect();
            if parse_cell_abs(&text).is_some() {
                return Some((q - p, 1));
            }
            return None;
        }
        if letters > 0 && letters <= 3 && digits == 0 {
            let l: String = cs[ls..after_letters].iter().map(|x| x.1).collect();
            col_index(&l)?;
            // a lone column needs the ':' form; checked by the caller
            return Some((after_letters - p + if ch(after_letters) == '$' { 0 } else { 0 }, 2));
        }
        if letters == 0 && digits > 0 {
            let t: String = cs[ds..q].iter().map(|x| x.1).collect();
            let v: u64 = t.parse().ok()?;
            if v == 0 || v > MAX_ROWS as u64 {
                return None;
            }
            return Some((q - p, 3));
        }
        None
    };
    let (l1, k1) = part(j)?;
    let mut end = j + l1;
    // the next character must not continue a name
    let follows_name = |e: usize| -> bool { e < n && (is_name_char(ch(e)) || ch(e) == '(' || ch(e) == '[') };
    if ch(end) == ':' {
        if let Some((l2, k2)) = part(end + 1) {
            if k2 == k1 || (k1 == 1 && k2 == 1) {
                let e2 = end + 1 + l2;
                if !follows_name(e2) {
                    return Some(e2 - i);
                }
            }
        }
    }
    if k1 != 1 {
        return None;
    }
    if follows_name(end) {
        return None;
    }
    // A1 followed directly by '(' is a function like LOG10( ... handled by follows_name
    let _ = (&mut end, had_sheet, start);
    Some(end - i)
}

/// Parse the text of a Ref token.
pub fn parse_ref_tok(text: &str) -> Option<RefTok> {
    let (sheet, sheet2, body) = split_sheet(text);
    if body.eq_ignore_ascii_case("#REF!") {
        return Some(RefTok { sheet, sheet2, r1: 0, c1: 0, r2: 0, c2: 0, ra1: false, ca1: false, ra2: false, ca2: false, kind: RefKind::Err });
    }
    let (a, b) = match body.find(':') {
        Some(i) => (&body[..i], Some(&body[i + 1..])),
        None => (body, None),
    };
    if let Some((r1, c1, ra1, ca1)) = parse_cell_abs(a) {
        let (r2, c2, ra2, ca2) = match b {
            Some(b) => parse_cell_abs(b)?,
            None => (r1, c1, ra1, ca1),
        };
        let kind = if b.is_some() { RefKind::Area } else { RefKind::Cell };
        return Some(RefTok { sheet, sheet2, r1, c1, r2, c2, ra1, ca1, ra2, ca2, kind });
    }
    let b = b?;
    let ca1 = a.starts_with('$');
    let ca2 = b.starts_with('$');
    let a2 = a.trim_start_matches('$');
    let b2 = b.trim_start_matches('$');
    if let (Some(c1), Some(c2)) = (col_index(a2), col_index(b2)) {
        return Some(RefTok { sheet, sheet2, r1: 0, c1, r2: MAX_ROWS - 1, c2, ra1: true, ca1, ra2: true, ca2, kind: RefKind::Cols });
    }
    if let (Ok(r1), Ok(r2)) = (a2.parse::<u32>(), b2.parse::<u32>()) {
        if r1 >= 1 && r2 >= 1 {
            return Some(RefTok { sheet, sheet2, r1: r1 - 1, c1: 0, r2: r2 - 1, c2: MAX_COLS - 1, ra1: ca1, ca1: true, ra2: ca2, ca2: true, kind: RefKind::Rows });
        }
    }
    None
}

/// "Sheet1!A1" -> (Some("Sheet1"), None, "A1"); "'My S'!A1"; "S1:S3!A1"
pub fn split_sheet(text: &str) -> (Option<String>, Option<String>, &str) {
    if let Some(stripped) = text.strip_prefix('\'') {
        // find the closing quote
        let b: Vec<char> = stripped.chars().collect();
        let mut i = 0;
        let mut name = String::new();
        while i < b.len() {
            if b[i] == '\'' {
                if i + 1 < b.len() && b[i + 1] == '\'' {
                    name.push('\'');
                    i += 2;
                    continue;
                }
                break;
            }
            name.push(b[i]);
            i += 1;
        }
        let consumed: usize = stripped.chars().take(i + 1).map(|c| c.len_utf8()).sum::<usize>() + 1;
        let rest = &text[consumed..];
        let rest = rest.strip_prefix('!').unwrap_or(rest);
        let (s1, s2) = match name.find(':') {
            Some(k) => (name[..k].to_string(), Some(name[k + 1..].to_string())),
            None => (name, None),
        };
        return (Some(s1), s2, rest);
    }
    // the body #REF! has its own '!'
    let search = if text.len() > 5 && text[text.len() - 5..].eq_ignore_ascii_case("#REF!") { &text[..text.len() - 5] } else { text };
    match search.rfind('!') {
        Some(i) => {
            let pre = &text[..i];
            let (s1, s2) = match pre.find(':') {
                Some(k) => (pre[..k].to_string(), Some(pre[k + 1..].to_string())),
                None => (pre.to_string(), None),
            };
            (Some(s1), s2, &text[i + 1..])
        }
        None => (None, None, text),
    }
}

impl RefTok {
    /// Write the reference back (sheet prefix included as given).
    pub fn write(&self) -> String {
        let mut s = String::new();
        if let Some(sh) = &self.sheet {
            match &self.sheet2 {
                Some(s2) => {
                    let joined = format!("{}:{}", sh, s2);
                    let q = quote_sheet(&joined);
                    // quote both together if either needs it
                    if q.starts_with('\'') || quote_sheet(sh).starts_with('\'') || quote_sheet(s2).starts_with('\'') {
                        s.push_str(&format!("'{}:{}'", sh.replace('\'', "''"), s2.replace('\'', "''")));
                    } else {
                        s.push_str(&joined);
                    }
                }
                None => s.push_str(&quote_sheet(sh)),
            }
            s.push('!');
        }
        match self.kind {
            RefKind::Err => s.push_str("#REF!"),
            RefKind::Cell => s.push_str(&cell_text(self.r1, self.c1, self.ra1, self.ca1)),
            RefKind::Area => {
                s.push_str(&cell_text(self.r1, self.c1, self.ra1, self.ca1));
                s.push(':');
                s.push_str(&cell_text(self.r2, self.c2, self.ra2, self.ca2));
            }
            RefKind::Cols => {
                s.push_str(&format!("{}{}:{}{}", if self.ca1 { "$" } else { "" }, col_name(self.c1), if self.ca2 { "$" } else { "" }, col_name(self.c2)));
            }
            RefKind::Rows => {
                s.push_str(&format!("{}{}:{}{}", if self.ra1 { "$" } else { "" }, self.r1 + 1, if self.ra2 { "$" } else { "" }, self.r2 + 1));
            }
        }
        s
    }
}

pub fn cell_text(r: u32, c: u32, ra: bool, ca: bool) -> String {
    format!("{}{}{}{}", if ca { "$" } else { "" }, col_name(c), if ra { "$" } else { "" }, r + 1)
}

// --------------------------------------------------------------------------- expression tree

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Str(Rc<str>),
    Bool(bool),
    Err(XlErr),
    Array(Vec<Vec<Expr>>),
    Ref(Box<RefTok>),
    Name(Option<String>, String),
    Func(Rc<str>, Vec<Expr>),
    Neg(Box<Expr>),
    Plus(Box<Expr>),
    Percent(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
    Isect(Box<Expr>, Box<Expr>),
    Union(Vec<Expr>),
    Spill(Box<Expr>),
    At(Box<Expr>),
    Missing,
    Struct(String),
    Ext(String),
    /// call of a value: LAMBDA(x,x+1)(2)
    Call(Box<Expr>, Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Cat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Function names written with a prefix in files (added after the 2007 file format).
pub fn file_prefix(name: &str) -> Option<&'static str> {
    const XLWS: [&str; 2] = ["FILTER", "SORT"];
    if XLWS.contains(&name) {
        return Some("_xlfn._xlws.");
    }
    const NEW: &[&str] = &[
        "ACOT", "ACOTH", "AGGREGATE", "ARABIC", "ARRAYTOTEXT", "BASE", "BETA.DIST", "BETA.INV", "BINOM.DIST",
        "BINOM.DIST.RANGE", "BINOM.INV", "BITAND", "BITLSHIFT", "BITOR", "BITRSHIFT", "BITXOR", "CEILING.MATH",
        "CEILING.PRECISE", "CHISQ.DIST", "CHISQ.DIST.RT", "CHISQ.INV", "CHISQ.INV.RT", "CHISQ.TEST",
        "CHOOSECOLS", "CHOOSEROWS", "COMBINA", "CONCAT", "CONFIDENCE.NORM", "CONFIDENCE.T", "COT", "COTH",
        "COVARIANCE.P", "COVARIANCE.S", "CSC", "CSCH", "DAYS", "DECIMAL", "DROP", "ERF.PRECISE",
        "ERFC.PRECISE", "EXPAND", "EXPON.DIST", "F.DIST", "F.DIST.RT", "F.INV", "F.INV.RT", "F.TEST",
        "FILTERXML", "FLOOR.MATH", "FLOOR.PRECISE", "FORECAST.ETS", "FORECAST.ETS.CONFINT", "FORECAST.ETS.SEASONALITY",
        "FORECAST.ETS.STAT", "FORECAST.LINEAR", "FORMULATEXT", "GAMMA", "GAMMA.DIST",
        "GAMMA.INV", "GAMMALN.PRECISE", "GAUSS", "HSTACK", "HYPGEOM.DIST", "IFNA", "IFS", "IMCOSH", "IMCOT",
        "IMCSC", "IMCSCH", "IMSEC", "IMSECH", "IMSINH", "IMTAN", "ISFORMULA", "ISOWEEKNUM", "LET",
        "LOGNORM.DIST", "LOGNORM.INV", "MAXIFS", "MINIFS", "MODE.MULT", "MODE.SNGL", "MUNIT",
        "NEGBINOM.DIST", "NORM.DIST", "NORM.INV", "NORM.S.DIST", "NORM.S.INV", "NUMBERVALUE",
        "PDURATION", "PERCENTILE.EXC", "PERCENTILE.INC", "PERCENTRANK.EXC", "PERCENTRANK.INC",
        "PERMUTATIONA", "PHI", "POISSON.DIST", "QUARTILE.EXC", "QUARTILE.INC", "RANDARRAY", "RANK.AVG",
        "RANK.EQ", "RRI", "SEC", "SECH", "SEQUENCE", "SHEET", "SHEETS", "SKEW.P", "SORTBY", "STDEV.P",
        "STDEV.S", "SWITCH", "T.DIST", "T.DIST.2T", "T.DIST.RT", "T.INV", "T.INV.2T", "T.TEST", "TAKE",
        "TEXTAFTER", "TEXTBEFORE", "TEXTJOIN", "TEXTSPLIT", "TOCOL",
    ];
    const NEW2: &[&str] = &[
        "TOROW", "UNICHAR", "UNICODE", "UNIQUE", "VALUETOTEXT", "VAR.P", "VAR.S", "VSTACK", "WEIBULL.DIST",
        "WRAPCOLS", "WRAPROWS", "XLOOKUP", "XMATCH", "XOR", "Z.TEST", "LAMBDA",
    ];
    if NEW.contains(&name) || NEW2.contains(&name) {
        return Some("_xlfn.");
    }
    None
}

/// Strip _xlfn. / _xlws. / _xlpm. prefixes that files use.
pub fn strip_prefixes(name: &str) -> &str {
    let mut s = name;
    for p in ["_xlfn.", "_XLFN.", "_xlws.", "_XLWS.", "_xlpm.", "_XLPM."] {
        if let Some(x) = s.strip_prefix(p) {
            s = x;
        }
    }
    s
}

pub struct Parser<'a> {
    toks: Vec<&'a Token>,
    pos: usize,
}

pub fn parse(src: &str) -> Result<Expr, String> {
    let toks = tokenize(src)?;
    parse_tokens(&toks)
}

pub fn parse_tokens(toks: &[Token]) -> Result<Expr, String> {
    // keep spaces only where they may be the intersection operator
    let mut keep: Vec<&Token> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        if t.kind == TokKind::Space {
            let prev = keep.last().map(|p| &p.kind);
            let next = toks[i + 1..].iter().find(|x| x.kind != TokKind::Space).map(|x| &x.kind);
            let operand_end = matches!(prev, Some(TokKind::Ref) | Some(TokKind::Name) | Some(TokKind::RParen) | Some(TokKind::Struct));
            let operand_start = matches!(next, Some(TokKind::Ref) | Some(TokKind::Name) | Some(TokKind::LParen) | Some(TokKind::Func) | Some(TokKind::Struct));
            if operand_end && operand_start {
                keep.push(t);
            }
            continue;
        }
        keep.push(t);
    }
    let mut p = Parser { toks: keep, pos: 0 };
    if p.toks.is_empty() {
        return Err("The formula is empty.".into());
    }
    let e = p.expr(0)?;
    if p.pos < p.toks.len() {
        let t = p.toks[p.pos];
        return Err(match t.kind {
            TokKind::RParen => "There's an extra closing parenthesis.".into(),
            _ => format!("Unexpected '{}'.", t.text),
        });
    }
    Ok(e)
}

fn binop(text: &str) -> Option<(BinOp, u8)> {
    Some(match text {
        "=" => (BinOp::Eq, 1),
        "<>" => (BinOp::Ne, 1),
        "<" => (BinOp::Lt, 1),
        "<=" => (BinOp::Le, 1),
        ">" => (BinOp::Gt, 1),
        ">=" => (BinOp::Ge, 1),
        "&" => (BinOp::Cat, 2),
        "+" => (BinOp::Add, 3),
        "-" => (BinOp::Sub, 3),
        "*" => (BinOp::Mul, 4),
        "/" => (BinOp::Div, 4),
        "^" => (BinOp::Pow, 5),
        _ => return None,
    })
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.toks.get(self.pos).copied()
    }
    fn next(&mut self) -> Option<&'a Token> {
        let t = self.toks.get(self.pos).copied();
        self.pos += 1;
        t
    }

    fn expr(&mut self, min_prec: u8) -> Result<Expr, String> {
        let mut lhs = self.unary()?;
        loop {
            let t = match self.peek() {
                Some(t) => t,
                None => break,
            };
            if t.kind != TokKind::Op {
                break;
            }
            let (op, prec) = match binop(&t.text) {
                Some(x) => x,
                None => break,
            };
            if prec < min_prec {
                break;
            }
            self.next();
            // ^ is left-associative in the spreadsheet language
            let rhs = self.expr(prec + 1)?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(t) if t.kind == TokKind::Op && (t.text == "-" || t.text == "+") => {
                self.next();
                let e = self.unary()?;
                Ok(if t.text == "-" { Expr::Neg(Box::new(e)) } else { Expr::Plus(Box::new(e)) })
            }
            _ => self.postfix(),
        }
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let mut e = self.range_expr()?;
        while let Some(t) = self.peek() {
            if t.kind == TokKind::Op && t.text == "%" {
                self.next();
                e = Expr::Percent(Box::new(e));
            } else {
                break;
            }
        }
        Ok(e)
    }

    // ':' binds tightest, then ' ' (intersection)
    fn range_expr(&mut self) -> Result<Expr, String> {
        let mut e = self.isect_operand()?;
        loop {
            match self.peek() {
                Some(t) if t.kind == TokKind::Space => {
                    self.next();
                    let r = self.isect_operand()?;
                    e = Expr::Isect(Box::new(e), Box::new(r));
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn isect_operand(&mut self) -> Result<Expr, String> {
        let mut e = self.primary()?;
        loop {
            match self.peek() {
                Some(t) if t.kind == TokKind::Op && t.text == ":" => {
                    self.next();
                    let r = self.primary()?;
                    e = Expr::Range(Box::new(e), Box::new(r));
                }
                Some(t) if t.kind == TokKind::Spill => {
                    self.next();
                    e = Expr::Spill(Box::new(e));
                }
                Some(t) if t.kind == TokKind::LParen && matches!(e, Expr::Func(..) | Expr::Call(..)) && is_lambda_call(&e) => {
                    self.next();
                    let args = self.args()?;
                    e = Expr::Call(Box::new(e), args);
                }
                _ => break,
            }
        }
        Ok(e)
    }

    fn args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        if let Some(t) = self.peek() {
            if t.kind == TokKind::RParen {
                self.next();
                return Ok(args);
            }
        }
        loop {
            let a = match self.peek() {
                Some(t) if t.kind == TokKind::Comma || t.kind == TokKind::RParen => Expr::Missing,
                Some(_) => self.expr(0)?,
                None => return Err("A closing parenthesis is missing.".into()),
            };
            args.push(a);
            match self.next() {
                Some(t) if t.kind == TokKind::Comma => continue,
                Some(t) if t.kind == TokKind::RParen => break,
                Some(t) => return Err(format!("Unexpected '{}'.", t.text)),
                None => return Err("A closing parenthesis is missing.".into()),
            }
        }
        Ok(args)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        let t = self.next().ok_or("The formula ends too early.")?;
        Ok(match t.kind {
            TokKind::Num => Expr::Num(t.text.parse::<f64>().map_err(|_| format!("'{}' isn't a number.", t.text))?),
            TokKind::Str => {
                let inner = &t.text[1..t.text.len() - 1];
                Expr::Str(Rc::from(inner.replace("\"\"", "\"").as_str()))
            }
            TokKind::Bool => Expr::Bool(t.text.eq_ignore_ascii_case("TRUE")),
            TokKind::Err => Expr::Err(XlErr::parse(&t.text).unwrap_or(XlErr::Value)),
            TokKind::Ref => Expr::Ref(Box::new(parse_ref_tok(&t.text).ok_or("A reference isn't valid.")?)),
            TokKind::Name => {
                let (sheet, _, name) = split_sheet(&t.text);
                Expr::Name(sheet, strip_prefixes(name).to_string())
            }
            TokKind::Struct => Expr::Struct(t.text.clone()),
            TokKind::Ext => Expr::Ext(t.text.clone()),
            TokKind::Func => {
                // skip spaces between the name and '('
                let lp = self.next();
                if !matches!(lp, Some(x) if x.kind == TokKind::LParen) {
                    return Err("A function needs parentheses.".into());
                }
                let name = strip_prefixes(&t.text).to_ascii_uppercase();
                let args = self.args()?;
                Expr::Func(Rc::from(name.as_str()), args)
            }
            TokKind::LParen => {
                let mut items = vec![self.expr(0)?];
                loop {
                    match self.next() {
                        Some(x) if x.kind == TokKind::RParen => break,
                        Some(x) if x.kind == TokKind::Comma => items.push(self.expr(0)?),
                        _ => return Err("A closing parenthesis is missing.".into()),
                    }
                }
                if items.len() == 1 {
                    items.pop().unwrap()
                } else {
                    Expr::Union(items)
                }
            }
            TokKind::LBrace => {
                let mut rows: Vec<Vec<Expr>> = vec![vec![]];
                loop {
                    let item = self.array_item()?;
                    rows.last_mut().unwrap().push(item);
                    match self.next() {
                        Some(x) if x.kind == TokKind::Comma => continue,
                        Some(x) if x.kind == TokKind::Semi => {
                            rows.push(vec![]);
                            continue;
                        }
                        Some(x) if x.kind == TokKind::RBrace => break,
                        _ => return Err("An array constant isn't closed.".into()),
                    }
                }
                let w = rows[0].len();
                if rows.iter().any(|r| r.len() != w) {
                    return Err("The rows of an array constant must be the same length.".into());
                }
                Expr::Array(rows)
            }
            TokKind::At => Expr::At(Box::new(self.isect_operand()?)),
            TokKind::Op if t.text == "-" || t.text == "+" => {
                let e = self.primary()?;
                if t.text == "-" { Expr::Neg(Box::new(e)) } else { Expr::Plus(Box::new(e)) }
            }
            TokKind::RParen => return Err("There's an extra closing parenthesis.".into()),
            TokKind::Comma => Expr::Missing,
            _ => return Err(format!("Unexpected '{}'.", t.text)),
        })
    }

    fn array_item(&mut self) -> Result<Expr, String> {
        let t = self.next().ok_or("An array constant isn't closed.")?;
        Ok(match t.kind {
            TokKind::Num => Expr::Num(t.text.parse().map_err(|_| "Bad number")?),
            TokKind::Str => Expr::Str(Rc::from(t.text[1..t.text.len() - 1].replace("\"\"", "\"").as_str())),
            TokKind::Bool => Expr::Bool(t.text.eq_ignore_ascii_case("TRUE")),
            TokKind::Err => Expr::Err(XlErr::parse(&t.text).unwrap_or(XlErr::Value)),
            TokKind::Op if t.text == "-" || t.text == "+" => {
                let n = self.next().ok_or("Bad array constant")?;
                if n.kind != TokKind::Num {
                    return Err("Array constants can only hold numbers, text, logical values and errors.".into());
                }
                let v: f64 = n.text.parse().map_err(|_| "Bad number")?;
                Expr::Num(if t.text == "-" { -v } else { v })
            }
            _ => return Err("Array constants can only hold numbers, text, logical values and errors.".into()),
        })
    }
}

fn is_lambda_call(e: &Expr) -> bool {
    match e {
        Expr::Func(n, _) => &**n == "LAMBDA",
        Expr::Call(..) => true,
        _ => false,
    }
}

// --------------------------------------------------------------------------- rewriting

/// What to do with the references in a formula.
pub enum Adjust<'a> {
    /// copy/fill: move relative parts by (dr, dc)
    Shift(i64, i64),
    /// rows inserted (n > 0) or deleted (n < 0) at `at` on sheet `sheet`;
    /// `here` is the sheet the formula lives on
    Rows { sheet: &'a str, here: &'a str, at: u32, n: i64 },
    Cols { sheet: &'a str, here: &'a str, at: u32, n: i64 },
    /// cells inserted/deleted in columns c1..=c2 only (Insert Cells, Shift cells down)
    RowsIn { sheet: &'a str, here: &'a str, at: u32, n: i64, c1: u32, c2: u32 },
    /// cells inserted/deleted in rows r1..=r2 only (Shift cells right)
    ColsIn { sheet: &'a str, here: &'a str, at: u32, n: i64, r1: u32, r2: u32 },
    /// a sheet renamed
    RenameSheet(&'a str, &'a str),
    /// a sheet deleted: references to it become #REF!
    DeleteSheet(&'a str),
    /// cut/paste: references into `src` on `sheet` move by (dr, dc)
    Move { sheet: &'a str, here: &'a str, src: crate::cellref::Rect, dr: i64, dc: i64 },
}

fn same(a: Option<&String>, here: &str, target: &str) -> bool {
    let s = a.map(|x| x.as_str()).unwrap_or(here);
    s.eq_ignore_ascii_case(target)
}

/// Rewrite the references in formula text. Returns the new text.
pub fn adjust(src: &str, how: &Adjust) -> String {
    let toks = match tokenize(src) {
        Ok(t) => t,
        Err(_) => return src.to_string(),
    };
    let mut out = String::with_capacity(src.len() + 8);
    for t in &toks {
        match t.kind {
            TokKind::Ref => {
                if let Some(r) = parse_ref_tok(&t.text) {
                    out.push_str(&adjust_ref(r, how));
                } else {
                    out.push_str(&t.text);
                }
            }
            TokKind::Name => {
                if let Adjust::RenameSheet(old, new) = how {
                    let (sheet, _, name) = split_sheet(&t.text);
                    if let Some(sh) = sheet {
                        if sh.eq_ignore_ascii_case(old) {
                            out.push_str(&format!("{}!{}", quote_sheet(new), name));
                            continue;
                        }
                    }
                }
                out.push_str(&t.text);
            }
            _ => out.push_str(&t.text),
        }
    }
    out
}

fn shift_coord(v: u32, d: i64, max: u32) -> Option<u32> {
    let x = v as i64 + d;
    if x < 0 || x >= max as i64 {
        None
    } else {
        Some(x as u32)
    }
}

pub fn adjust_ref(mut r: RefTok, how: &Adjust) -> String {
    if r.kind == RefKind::Err {
        return r.write();
    }
    match how {
        Adjust::Shift(dr, dc) => {
            // a shift past the sheet edge wraps around, as the original does
            let wrap = |v: u32, d: i64, max: u32| -> u32 { ((v as i64 + d).rem_euclid(max as i64)) as u32 };
            if r.kind != RefKind::Cols {
                if !r.ra1 {
                    r.r1 = wrap(r.r1, *dr, MAX_ROWS);
                }
                if !r.ra2 {
                    r.r2 = wrap(r.r2, *dr, MAX_ROWS);
                }
            }
            if r.kind != RefKind::Rows {
                if !r.ca1 {
                    r.c1 = wrap(r.c1, *dc, MAX_COLS);
                }
                if !r.ca2 {
                    r.c2 = wrap(r.c2, *dc, MAX_COLS);
                }
            }
            if r.kind == RefKind::Area || r.kind == RefKind::Cols || r.kind == RefKind::Rows {
                if r.r1 > r.r2 {
                    std::mem::swap(&mut r.r1, &mut r.r2);
                    std::mem::swap(&mut r.ra1, &mut r.ra2);
                }
                if r.c1 > r.c2 {
                    std::mem::swap(&mut r.c1, &mut r.c2);
                    std::mem::swap(&mut r.ca1, &mut r.ca2);
                }
            }
            r.write()
        }
        Adjust::Rows { sheet, here, at, n } => {
            if !same(r.sheet.as_ref(), here, sheet) || r.sheet2.is_some() || r.kind == RefKind::Cols {
                return r.write();
            }
            match adjust_span(r.r1, r.r2, *at, *n, MAX_ROWS) {
                Some((a, b)) => {
                    r.r1 = a;
                    r.r2 = b;
                    r.write()
                }
                None => {
                    r.kind = RefKind::Err;
                    r.write()
                }
            }
        }
        Adjust::RowsIn { sheet, here, at, n, c1, c2 } => {
            if !same(r.sheet.as_ref(), here, sheet) || r.sheet2.is_some() || r.kind == RefKind::Cols || r.kind == RefKind::Rows
                || r.c1 < *c1 || r.c2 > *c2 {
                return r.write();
            }
            adjust_ref(r, &Adjust::Rows { sheet, here, at: *at, n: *n })
        }
        Adjust::ColsIn { sheet, here, at, n, r1, r2 } => {
            if !same(r.sheet.as_ref(), here, sheet) || r.sheet2.is_some() || r.kind == RefKind::Cols || r.kind == RefKind::Rows
                || r.r1 < *r1 || r.r2 > *r2 {
                return r.write();
            }
            adjust_ref(r, &Adjust::Cols { sheet, here, at: *at, n: *n })
        }
        Adjust::Cols { sheet, here, at, n } => {
            if !same(r.sheet.as_ref(), here, sheet) || r.sheet2.is_some() || r.kind == RefKind::Rows {
                return r.write();
            }
            match adjust_span(r.c1, r.c2, *at, *n, MAX_COLS) {
                Some((a, b)) => {
                    r.c1 = a;
                    r.c2 = b;
                    r.write()
                }
                None => {
                    r.kind = RefKind::Err;
                    r.write()
                }
            }
        }
        Adjust::RenameSheet(old, new) => {
            if let Some(s) = &r.sheet {
                if s.eq_ignore_ascii_case(old) {
                    r.sheet = Some(new.to_string());
                }
            }
            if let Some(s) = &r.sheet2 {
                if s.eq_ignore_ascii_case(old) {
                    r.sheet2 = Some(new.to_string());
                }
            }
            r.write()
        }
        Adjust::DeleteSheet(name) => {
            if let Some(s) = &r.sheet {
                if s.eq_ignore_ascii_case(name) && r.sheet2.is_none() {
                    r.kind = RefKind::Err;
                    r.sheet = None;
                }
            }
            r.write()
        }
        Adjust::Move { sheet, here, src, dr, dc } => {
            if !same(r.sheet.as_ref(), here, sheet) || r.sheet2.is_some() {
                return r.write();
            }
            let rect = crate::cellref::Rect::new(r.r1, r.c1, r.r2, r.c2);
            if src.contains_rect(&rect) {
                if let (Some(a), Some(b), Some(c), Some(d)) = (
                    shift_coord(r.r1, *dr, MAX_ROWS), shift_coord(r.r2, *dr, MAX_ROWS),
                    shift_coord(r.c1, *dc, MAX_COLS), shift_coord(r.c2, *dc, MAX_COLS),
                ) {
                    if r.kind != RefKind::Cols {
                        r.r1 = a;
                        r.r2 = b;
                    }
                    if r.kind != RefKind::Rows {
                        r.c1 = c;
                        r.c2 = d;
                    }
                } else {
                    r.kind = RefKind::Err;
                }
            }
            r.write()
        }
    }
}

/// Move a span [a, b] for n lines inserted (n > 0) or deleted (n < 0) at `at`.
/// None when the whole span was deleted.
pub fn adjust_span(a: u32, b: u32, at: u32, n: i64, max: u32) -> Option<(u32, u32)> {
    if n > 0 {
        let n = n as u32;
        let na = if a >= at { a.saturating_add(n) } else { a };
        let nb = if b >= at { b.saturating_add(n) } else { b };
        // spans pushed off the sheet are cut to the last line; whole-sheet spans stay
        if a == 0 && b == max - 1 {
            return Some((a, b));
        }
        if na >= max {
            return None;
        }
        return Some((na, nb.min(max - 1)));
    }
    let n = (-n) as u32;
    let del_end = at + n - 1; // inclusive
    if a == 0 && b == max - 1 {
        return Some((a, b));
    }
    if b < at {
        return Some((a, b));
    }
    if a > del_end {
        return Some((a - n, b - n));
    }
    // overlap
    if a >= at && b <= del_end {
        return None;
    }
    let na = if a < at { a } else { at };
    let nb = if b > del_end { b - n } else { at.saturating_sub(1).max(na) };
    if b <= del_end && a < at {
        return Some((a, at - 1));
    }
    Some((na, nb))
}

/// Spans of references in formula text for colouring while editing:
/// (start char, end char, sheet, r1, c1, r2, c2).
pub fn ref_spans(src: &str) -> Vec<(usize, usize, Option<String>, u32, u32, u32, u32)> {
    let toks = match tokenize(src) {
        Ok(t) => t,
        Err(_) => tokenize_lenient(src),
    };
    let mut out = Vec::new();
    for t in toks {
        if t.kind == TokKind::Ref {
            if let Some(r) = parse_ref_tok(&t.text) {
                if r.kind == RefKind::Err {
                    continue;
                }
                let s = src[..t.start].chars().count();
                let e = s + t.text.chars().count();
                out.push((s, e, r.sheet.clone(), r.r1, r.c1, r.r2, r.c2));
            }
        }
    }
    out
}

/// Tokenize as far as possible (an unfinished formula being typed).
fn tokenize_lenient(src: &str) -> Vec<Token> {
    for cut in (0..=src.len()).rev() {
        if !src.is_char_boundary(cut) {
            continue;
        }
        if let Ok(t) = tokenize(&src[..cut]) {
            return t;
        }
    }
    Vec::new()
}

/// None if the formula parses, else a message.
pub fn check(src: &str) -> Option<String> {
    match parse(src) {
        Ok(_) => None,
        Err(e) => Some(e),
    }
}

/// Text for writing to a file: function names get their _xlfn. prefixes.
pub fn to_file_text(src: &str) -> String {
    let toks = match tokenize(src) {
        Ok(t) => t,
        Err(_) => return src.to_string(),
    };
    let mut out = String::with_capacity(src.len() + 16);
    let mut let_names: Vec<String> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        match t.kind {
            TokKind::Func => {
                let up = t.text.to_ascii_uppercase();
                let base = strip_prefixes(&up);
                if let Some(p) = file_prefix(base) {
                    if !t.text.starts_with("_xl") {
                        out.push_str(p);
                    }
                }
                out.push_str(&t.text);
                if base == "LET" || base == "LAMBDA" {
                    // remember parameter names: LET(name, value, ...) / LAMBDA(x, y, body)
                    let mut depth = 0;
                    let mut arg = 0;
                    let mut expect_name = true;
                    for u in &toks[i + 1..] {
                        match u.kind {
                            TokKind::LParen => depth += 1,
                            TokKind::RParen => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            TokKind::Comma if depth == 1 => {
                                arg += 1;
                                expect_name = if base == "LET" { arg % 2 == 0 } else { true };
                            }
                            TokKind::Name if depth == 1 && expect_name => {
                                let_names.push(u.text.to_ascii_uppercase());
                                expect_name = false;
                            }
                            _ => {}
                        }
                    }
                }
            }
            TokKind::Name => {
                if let_names.contains(&t.text.to_ascii_uppercase()) && !t.text.starts_with("_xl") {
                    out.push_str("_xlpm.");
                }
                out.push_str(&t.text);
            }
            _ => out.push_str(&t.text),
        }
    }
    out
}

/// Text read from a file: prefixes removed for display and editing.
pub fn from_file_text(src: &str) -> String {
    if !src.contains("_xl") && !src.contains("_XL") {
        return src.to_string();
    }
    let toks = match tokenize(src) {
        Ok(t) => t,
        Err(_) => return src.to_string(),
    };
    let mut out = String::with_capacity(src.len());
    for t in &toks {
        match t.kind {
            TokKind::Func | TokKind::Name => out.push_str(strip_prefixes(&t.text)),
            _ => out.push_str(&t.text),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tokens_and_refs() {
        let spans = ref_spans("A1+Sheet1!B2:C3+$D$4");
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1], (3, 15, Some("Sheet1".into()), 1, 1, 2, 2));
        assert!(parse("SUM(A1:B2)").is_ok());
        assert!(parse("SUM(1").is_err());
        assert!(parse("LOG10(100)").is_ok());
        assert!(matches!(parse("A:C").unwrap(), Expr::Ref(_)));
        assert!(matches!(parse("3:5").unwrap(), Expr::Ref(_)));
        assert!(matches!(parse("-2^2").unwrap(), Expr::Bin(BinOp::Pow, _, _)));
        assert!(parse("'My Sheet'!A1*2").is_ok());
        assert!(parse("{1,2;3,4}").is_ok());
        assert!(parse("IF(A1>0,\"yes\",)").is_ok());
        assert!(parse("A1#").is_ok());
        assert!(parse("Table1[Col]").is_ok());
    }
    #[test]
    fn shifting() {
        assert_eq!(adjust("A1+$B$2+B$3+Sheet1!C4", &Adjust::Shift(2, 1)), "B3+$B$2+C$3+Sheet1!D6");
        assert_eq!(adjust("SUM(A1:A10)", &Adjust::Rows { sheet: "S", here: "S", at: 4, n: 2 }), "SUM(A1:A12)");
        assert_eq!(adjust("A5", &Adjust::Rows { sheet: "S", here: "S", at: 4, n: -1 }), "#REF!");
        assert_eq!(adjust("SUM(A1:A10)", &Adjust::Rows { sheet: "S", here: "S", at: 4, n: -2 }), "SUM(A1:A8)");
        assert_eq!(adjust("Old!A1+'Old'!B2", &Adjust::RenameSheet("Old", "New Name")), "'New Name'!A1+'New Name'!B2");
        assert_eq!(to_file_text("XLOOKUP(A1,B:B,C:C)+SUM(1)"), "_xlfn.XLOOKUP(A1,B:B,C:C)+SUM(1)");
        assert_eq!(to_file_text("LET(x,1,x+1)"), "_xlfn.LET(_xlpm.x,1,_xlpm.x+1)");
        assert_eq!(from_file_text("_xlfn._xlws.FILTER(A1:A3,B1:B3)"), "FILTER(A1:A3,B1:B3)");
    }
}
