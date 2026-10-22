//! Text functions.

use super::*;
use crate::calc::{Eval, Res};
use crate::value::{Array, Value, XlErr};

fn t1(ev: &mut Eval, a: &[Expr], f: fn(&str) -> Value) -> Res {
    lift(ev, a, move |_, v| match vtext(&v[0]) {
        Ok(s) => f(&s),
        Err(e) => Value::Err(e),
    })
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

const CP1252: [u32; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F,
    0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

pub static DEFS: &[Def] = &[
    d!("LEN", 1, 1, |ev, a| t1(ev, a, |s| Value::num(s.chars().count() as f64))),
    d!("LENB", 1, 1, |ev, a| t1(ev, a, |s| Value::num(s.chars().count() as f64))),
    d!("LOWER", 1, 1, |ev, a| t1(ev, a, |s| Value::string(s.to_lowercase()))),
    d!("UPPER", 1, 1, |ev, a| t1(ev, a, |s| Value::string(s.to_uppercase()))),
    d!("PROPER", 1, 1, |ev, a| t1(ev, a, |s| {
        let mut out = String::with_capacity(s.len());
        let mut prev_letter = false;
        for c in s.chars() {
            if c.is_alphabetic() {
                if prev_letter { out.extend(c.to_lowercase()) } else { out.extend(c.to_uppercase()) }
                prev_letter = true;
            } else {
                out.push(c);
                prev_letter = false;
            }
        }
        Value::string(out)
    })),
    d!("TRIM", 1, 1, |ev, a| t1(ev, a, |s| Value::string(s.split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")))),
    d!("CLEAN", 1, 1, |ev, a| t1(ev, a, |s| Value::string(s.chars().filter(|c| *c as u32 >= 32).collect()))),
    d!("CODE", 1, 1, |ev, a| t1(ev, a, |s| match s.chars().next() {
        None => Value::Err(XlErr::Value),
        Some(c) => {
            let u = c as u32;
            if u < 128 || (160..256).contains(&u) { return Value::num(u as f64); }
            match CP1252.iter().position(|x| *x == u) { Some(i) => Value::num((128 + i) as f64), None => Value::num(63.0) }
        }
    })),
    d!("UNICODE", 1, 1, |ev, a| t1(ev, a, |s| match s.chars().next() { None => Value::Err(XlErr::Value), Some(c) => Value::num(c as u32 as f64) })),
    d!("CHAR", 1, 1, |ev, a| math1(ev, a, |x| {
        let n = x.trunc() as u32;
        if !(1..=255).contains(&n) { return Value::Err(XlErr::Value); }
        let u = if (128..160).contains(&n) { CP1252[(n - 128) as usize] } else { n };
        Value::string(char::from_u32(u).unwrap_or('?').to_string())
    })),
    d!("UNICHAR", 1, 1, |ev, a| math1(ev, a, |x| {
        let n = x.trunc() as u32;
        match char::from_u32(n) { Some(c) if n > 0 => Value::string(c.to_string()), _ => Value::Err(XlErr::Value) }
    })),
    d!("EXACT", 2, 2, |ev, a| lift(ev, a, |_, v| match (vtext(&v[0]), vtext(&v[1])) {
        (Ok(x), Ok(y)) => Value::Bool(x == y),
        (Err(e), _) | (_, Err(e)) => Value::Err(e),
    })),
    d!("T", 1, 1, |ev, a| { let v = ev.eval_scalar(&a[0]); Res::V(match v { Value::Str(_) | Value::Err(_) => v, _ => Value::str("") }) }),
    d!("LEFT", 1, 2, |ev, a| left_right(ev, a, true)),
    d!("LEFTB", 1, 2, |ev, a| left_right(ev, a, true)),
    d!("RIGHT", 1, 2, |ev, a| left_right(ev, a, false)),
    d!("RIGHTB", 1, 2, |ev, a| left_right(ev, a, false)),
    d!("MID", 3, 3, mid),
    d!("MIDB", 3, 3, mid),
    d!("REPT", 2, 2, |ev, a| lift(ev, a, |ev, v| {
        let s = match vtext(&v[0]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        let n = match vnum(ev, &v[1]) { Ok(n) => n.trunc(), Err(e) => return Value::Err(e) };
        if n < 0.0 || s.chars().count() as f64 * n > 32767.0 { return Value::Err(XlErr::Value); }
        Value::string(s.repeat(n as usize))
    })),
    d!("FIND", 2, 3, |ev, a| find(ev, a, true)),
    d!("FINDB", 2, 3, |ev, a| find(ev, a, true)),
    d!("SEARCH", 2, 3, |ev, a| find(ev, a, false)),
    d!("SEARCHB", 2, 3, |ev, a| find(ev, a, false)),
    d!("REPLACE", 4, 4, replace),
    d!("REPLACEB", 4, 4, replace),
    d!("SUBSTITUTE", 3, 4, substitute),
    d!("CONCATENATE", 1, MANY, |ev, a| lift(ev, a, |_, v| {
        let mut s = String::new();
        for x in v { match vtext(x) { Ok(t) => s.push_str(&t), Err(e) => return Value::Err(e) } }
        if s.chars().count() > 32767 { return Value::Err(XlErr::Value); }
        Value::string(s)
    })),
    d!("CONCAT", 1, MANY, |ev, a| {
        let mut s = String::new();
        for e in a {
            for v in values_of(ev, e) {
                match vtext(&v) { Ok(t) => s.push_str(&t), Err(e) => return Res::err(e) }
            }
        }
        if s.chars().count() > 32767 { return Res::err(XlErr::Value); }
        Res::V(Value::string(s))
    }),
    d!("TEXTJOIN", 3, MANY, |ev, a| {
        let delims: Vec<String> = values_of(ev, &a[0]).iter().map(|v| v.to_string()).collect();
        let ignore = tri!(boolean(ev, &a[1]));
        let mut parts: Vec<String> = Vec::new();
        for e in &a[2..] {
            for v in values_of(ev, e) {
                if let Value::Err(x) = v { return Res::err(x); }
                let t = v.to_string();
                if ignore && t.is_empty() { continue; }
                parts.push(t);
            }
        }
        let mut s = String::new();
        for (i, p) in parts.iter().enumerate() {
            if i > 0 && !delims.is_empty() { s.push_str(&delims[(i - 1) % delims.len()]); }
            s.push_str(p);
        }
        if s.chars().count() > 32767 { return Res::err(XlErr::Value); }
        Res::V(Value::string(s))
    }),
    d!("VALUE", 1, 1, |ev, a| lift(ev, a, |ev, v| match &v[0] {
        Value::Num(n) => Value::num(*n),
        Value::Empty => Value::num(0.0),
        Value::Str(s) => match crate::calc::text_to_number(s, ev.d1904()) { Some(n) => Value::num(n), None => Value::Err(XlErr::Value) },
        Value::Bool(_) => Value::Err(XlErr::Value),
        Value::Err(e) => Value::Err(*e),
        _ => Value::Err(XlErr::Value),
    })),
    d!("NUMBERVALUE", 1, 3, |ev, a| {
        let s = tri!(text(ev, &a[0]));
        let dec = tri!(opt_text(ev, a, 1, "."));
        let grp = tri!(opt_text(ev, a, 2, ","));
        let dc = dec.chars().next().unwrap_or('.');
        let mut t: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        for g in grp.chars() { if g != dc { t = t.replace(g, ""); } }
        if dc != '.' { t = t.replace(dc, "."); }
        let mut pct = 0;
        while t.ends_with('%') { t.pop(); pct += 1; }
        if t.is_empty() { return Res::num(0.0); }
        match t.parse::<f64>() { Ok(v) => Res::num(v / 100f64.powi(pct)), Err(_) => Res::err(XlErr::Value) }
    }),
    d!("TEXT", 2, 2, |ev, a| lift(ev, a, |ev, v| {
        let code = match vtext(&v[1]) { Ok(c) => c, Err(e) => return Value::Err(e) };
        let f = crate::numfmt::get(&code);
        match &v[0] {
            Value::Err(e) => Value::Err(*e),
            Value::Str(s) => match crate::calc::text_to_number(s, ev.d1904()) {
                Some(n) => Value::string(f.number(n, ev.d1904(), 11).text),
                None => Value::string(f.text(s).text),
            },
            Value::Bool(b) => Value::string(f.text(if *b { "TRUE" } else { "FALSE" }).text),
            x => match vnum(ev, x) { Ok(n) => Value::string(f.number(n, ev.d1904(), 11).text), Err(e) => Value::Err(e) },
        }
    })),
    d!("FIXED", 1, 3, |ev, a| {
        let n = a.len();
        lift(ev, a, move |ev, v| {
            let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
            let d = if n > 1 && !v[1].is_empty() { match vnum(ev, &v[1]) { Ok(d) => d.trunc(), Err(e) => return Value::Err(e) } } else { 2.0 };
            let nocomma = if n > 2 { crate::calc::to_bool(&v[2]).unwrap_or(false) } else { false };
            if d > 127.0 { return Value::Err(XlErr::Value); }
            let r = super::math::round_half_away(x, d);
            let dd = d.max(0.0) as usize;
            let code = format!("{}{}", if nocomma { "0" } else { "#,##0" }, if dd > 0 { format!(".{}", "0".repeat(dd)) } else { String::new() });
            Value::string(crate::numfmt::format_number(r, &code, false))
        })
    }),
    d!("DOLLAR", 1, 2, |ev, a| {
        let n = a.len();
        lift(ev, a, move |ev, v| {
            let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
            let d = if n > 1 { match vnum(ev, &v[1]) { Ok(d) => d.trunc(), Err(e) => return Value::Err(e) } } else { 2.0 };
            let r = super::math::round_half_away(x, d);
            let dd = d.max(0.0) as usize;
            let body = format!("#,##0{}", if dd > 0 { format!(".{}", "0".repeat(dd)) } else { String::new() });
            let code = format!("\"$\"{b}_);\\(\"$\"{b}\\)", b = body);
            Value::string(crate::numfmt::format_number(r, &code, false).trim_end().to_string())
        })
    }),
    d!("VALUETOTEXT", 1, 2, |ev, a| {
        let strict = tri!(opt_num(ev, a, 1, 0.0)) == 1.0;
        let v = ev.eval_value(&a[0]);
        let f = move |x: &Value| -> Value {
            match x {
                Value::Str(s) if strict => Value::string(format!("\"{}\"", s.replace('"', "\"\""))),
                x => Value::string(x.to_string()),
            }
        };
        Res::V(match v {
            Value::Arr(arr) => Value::Arr(std::rc::Rc::new(Array::new(arr.rows, arr.cols, arr.data.iter().map(f).collect()))),
            x => f(&x),
        })
    }),
    d!("ARRAYTOTEXT", 1, 2, |ev, a| {
        let strict = tri!(opt_num(ev, a, 1, 0.0)) == 1.0;
        let arr = tri!(array_of(ev, &a[0]));
        if strict { return Res::V(Value::string(crate::value::array_text(&arr))); }
        Res::V(Value::string(arr.data.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")))
    }),
    d!("TEXTBEFORE", 2, 6, |ev, a| text_around(ev, a, true)),
    d!("TEXTAFTER", 2, 6, |ev, a| text_around(ev, a, false)),
    d!("TEXTSPLIT", 2, 6, textsplit),
];

fn left_right(ev: &mut Eval, a: &[Expr], left: bool) -> Res {
    let n = a.len();
    lift(ev, a, move |ev, v| {
        let s = match vtext(&v[0]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        let k = if n > 1 { match vnum(ev, &v[1]) { Ok(k) => k.trunc(), Err(e) => return Value::Err(e) } } else { 1.0 };
        if k < 0.0 {
            return Value::Err(XlErr::Value);
        }
        let cs = chars(&s);
        let k = (k as usize).min(cs.len());
        Value::string(if left { cs[..k].iter().collect() } else { cs[cs.len() - k..].iter().collect() })
    })
}

fn mid(ev: &mut Eval, a: &[Expr]) -> Res {
    lift(ev, a, |ev, v| {
        let s = match vtext(&v[0]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        let st = match vnum(ev, &v[1]) { Ok(x) => x.trunc(), Err(e) => return Value::Err(e) };
        let n = match vnum(ev, &v[2]) { Ok(x) => x.trunc(), Err(e) => return Value::Err(e) };
        if st < 1.0 || n < 0.0 {
            return Value::Err(XlErr::Value);
        }
        let cs = chars(&s);
        let st = (st as usize - 1).min(cs.len());
        let end = (st + n as usize).min(cs.len());
        Value::string(cs[st..end].iter().collect())
    })
}

fn find(ev: &mut Eval, a: &[Expr], case: bool) -> Res {
    let n = a.len();
    lift(ev, a, move |ev, v| {
        let needle = match vtext(&v[0]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        let hay = match vtext(&v[1]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        let start = if n > 2 { match vnum(ev, &v[2]) { Ok(x) => x.trunc(), Err(e) => return Value::Err(e) } } else { 1.0 };
        let hc = chars(&hay);
        if start < 1.0 || start as usize > hc.len() + 1 {
            return Value::Err(XlErr::Value);
        }
        let s0 = start as usize - 1;
        if needle.is_empty() {
            return Value::num(start);
        }
        if case {
            let nc = chars(&needle);
            for i in s0..hc.len() {
                if hc[i..].starts_with(&nc) {
                    return Value::num((i + 1) as f64);
                }
            }
        } else {
            let pat = needle.to_lowercase();
            let wild = pat.contains('*') || pat.contains('?') || pat.contains('~');
            let hl: Vec<char> = hay.to_lowercase().chars().collect();
            for i in s0..hl.len() {
                if wild {
                    // the shortest match starting here
                    for j in i..=hl.len() {
                        let sub: String = hl[i..j].iter().collect();
                        if wildcard_match(&pat, &sub) {
                            return Value::num((i + 1) as f64);
                        }
                    }
                } else {
                    let pc = chars(&pat);
                    if hl[i..].starts_with(&pc) {
                        return Value::num((i + 1) as f64);
                    }
                }
            }
        }
        Value::Err(XlErr::Value)
    })
}

fn replace(ev: &mut Eval, a: &[Expr]) -> Res {
    lift(ev, a, |ev, v| {
        let s = match vtext(&v[0]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        let st = match vnum(ev, &v[1]) { Ok(x) => x.trunc(), Err(e) => return Value::Err(e) };
        let n = match vnum(ev, &v[2]) { Ok(x) => x.trunc(), Err(e) => return Value::Err(e) };
        let new = match vtext(&v[3]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        if st < 1.0 || n < 0.0 {
            return Value::Err(XlErr::Value);
        }
        let cs = chars(&s);
        let a0 = (st as usize - 1).min(cs.len());
        let b0 = (a0 + n as usize).min(cs.len());
        let mut out: String = cs[..a0].iter().collect();
        out.push_str(&new);
        out.extend(cs[b0..].iter());
        Value::string(out)
    })
}

fn substitute(ev: &mut Eval, a: &[Expr]) -> Res {
    let n = a.len();
    lift(ev, a, move |ev, v| {
        let s = match vtext(&v[0]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        let old = match vtext(&v[1]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        let new = match vtext(&v[2]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        if old.is_empty() {
            return Value::string(s);
        }
        if n > 3 {
            let k = match vnum(ev, &v[3]) { Ok(x) => x.trunc(), Err(e) => return Value::Err(e) };
            if k < 1.0 {
                return Value::Err(XlErr::Value);
            }
            let mut count = 0;
            let mut idx = 0;
            while let Some(p) = s[idx..].find(&old) {
                count += 1;
                if count == k as usize {
                    let at = idx + p;
                    return Value::string(format!("{}{}{}", &s[..at], new, &s[at + old.len()..]));
                }
                idx += p + old.len();
            }
            return Value::string(s);
        }
        Value::string(s.replace(&old, &new))
    })
}

fn text_around(ev: &mut Eval, a: &[Expr], before: bool) -> Res {
    let s = tri!(text(ev, &a[0]));
    let delims: Vec<String> = values_of(ev, &a[1]).iter().map(|v| v.to_string()).collect();
    let inst = tri!(opt_num(ev, a, 2, 1.0)).trunc() as i64;
    let nocase = tri!(opt_num(ev, a, 3, 0.0)) != 0.0;
    let match_end = tri!(opt_num(ev, a, 4, 0.0)) != 0.0;
    let if_nf = if is_missing(a, 5) { None } else { Some(ev.eval_scalar(&a[5])) };
    if inst == 0 || s.chars().count() as i64 <= inst.abs() - 1 && !delims.iter().any(|d| d.is_empty()) && false {
        return Res::err(XlErr::Value);
    }
    let hay = if nocase { s.to_lowercase() } else { s.clone() };
    // all delimiter positions (byte offsets in hay, which has the same layout when only ASCII changes case)
    let mut hits: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i <= hay.len() {
        let mut found = None;
        for d in &delims {
            let d2 = if nocase { d.to_lowercase() } else { d.clone() };
            if hay[i..].starts_with(&d2) {
                found = Some(d2.len());
                break;
            }
        }
        if let Some(l) = found {
            hits.push((i, l));
            i += l.max(1);
        } else {
            i += hay[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        }
    }
    if match_end {
        if inst > 0 {
            hits.push((s.len(), 0));
        } else {
            hits.insert(0, (0, 0));
        }
    }
    let pick = if inst > 0 { hits.get(inst as usize - 1) } else { hits.len().checked_sub((-inst) as usize).and_then(|k| hits.get(k)) };
    match pick {
        Some(&(pos, len)) => Res::V(Value::string(if before { s[..pos].to_string() } else { s[pos + len..].to_string() })),
        None => match if_nf {
            Some(v) => Res::V(v),
            None => Res::err(XlErr::NA),
        },
    }
}

fn textsplit(ev: &mut Eval, a: &[Expr]) -> Res {
    let s = tri!(text(ev, &a[0]));
    let col_d: Vec<String> = if is_missing(a, 1) { vec![] } else { values_of(ev, &a[1]).iter().map(|v| v.to_string()).collect() };
    let row_d: Vec<String> = if is_missing(a, 2) { vec![] } else { values_of(ev, &a[2]).iter().map(|v| v.to_string()).collect() };
    let ignore = tri!(opt_bool(ev, a, 3, false));
    let nocase = tri!(opt_num(ev, a, 4, 0.0)) != 0.0;
    let pad = if is_missing(a, 5) { Value::Err(XlErr::NA) } else { ev.eval_scalar(&a[5]) };
    let split = |t: &str, ds: &[String]| -> Vec<String> {
        if ds.is_empty() || ds.iter().all(|d| d.is_empty()) {
            return vec![t.to_string()];
        }
        let hay = if nocase { t.to_lowercase() } else { t.to_string() };
        let mut out = Vec::new();
        let mut last = 0;
        let mut i = 0;
        while i < hay.len() {
            let mut hit = None;
            for d in ds {
                if d.is_empty() {
                    continue;
                }
                let d2 = if nocase { d.to_lowercase() } else { d.clone() };
                if hay[i..].starts_with(&d2) {
                    hit = Some(d2.len());
                    break;
                }
            }
            if let Some(l) = hit {
                out.push(t[last..i].to_string());
                i += l;
                last = i;
            } else {
                i += hay[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            }
        }
        out.push(t[last..].to_string());
        if ignore {
            out.retain(|x| !x.is_empty());
        }
        out
    };
    let rows: Vec<Vec<String>> = split(&s, &row_d).iter().map(|r| split(r, &col_d)).collect();
    let nr = rows.len().max(1);
    let nc = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let mut data = Vec::with_capacity(nr * nc);
    for r in &rows {
        for c in 0..nc {
            data.push(match r.get(c) {
                Some(t) => Value::string(t.clone()),
                None => pad.clone(),
            });
        }
    }
    arr_val(Array::new(nr, nc, data))
}
