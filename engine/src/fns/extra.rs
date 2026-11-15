//! Functions added in Backlog 6: regular expressions, GROUPBY and PIVOTBY, PERCENTOF,
//! TRIMRANGE, ENCODEURL, EUROCONVERT, FILTERXML, GETPIVOTDATA and IMAGE.

use super::*;
use crate::calc::{Area, Eval, Res};
use crate::value::{Array, Value, XlErr};

pub static DEFS: &[Def] = &[
    d!("REGEXTEST", 2, 3, regextest),
    d!("REGEXEXTRACT", 2, 4, regexextract),
    d!("REGEXREPLACE", 3, 5, regexreplace),
    d!("PERCENTOF", 2, 2, percentof),
    d!("TRIMRANGE", 1, 3, trimrange),
    d!("ENCODEURL", 1, 1, |ev, a| lift(ev, a, |_, v| match vtext(&v[0]) {
        Ok(s) => Value::string(encode_url(&s)),
        Err(e) => Value::Err(e),
    })),
    d!("EUROCONVERT", 3, 5, euroconvert),
    d!("GROUPBY", 3, 8, groupby),
    d!("PIVOTBY", 4, 11, pivotby),
    d!("FILTERXML", 2, 2, filterxml),
    d!("GETPIVOTDATA", 2, MANY, getpivotdata),
    // IMAGE fetches pictures from the web; this program never goes on the network
    d!("IMAGE", 1, 5, |_ev, _a| Res::err(XlErr::Blocked)),
];

// --------------------------------------------------------------------------- regular expressions

/// The pattern, compiled; case_sensitivity 1 ignores case, as in the original.
fn regex(pat: &str, ignore_case: bool) -> Result<fancy_regex::Regex, XlErr> {
    let full = if ignore_case { format!("(?i){}", pat) } else { pat.to_string() };
    let mut b = fancy_regex::RegexBuilder::new(&full);
    b.backtrack_limit(1_000_000);
    b.build().map_err(|_| XlErr::Value)
}

fn case_arg(v: &Value) -> Result<bool, XlErr> {
    match v {
        Value::Empty => Ok(false),
        _ => Ok(crate::calc::to_num(v, false)? != 0.0),
    }
}

fn regextest(ev: &mut Eval, a: &[Expr]) -> Res {
    lift(ev, a, |_, v| {
        let (t, p) = match (vtext(&v[0]), vtext(&v[1])) {
            (Ok(t), Ok(p)) => (t, p),
            (Err(e), _) | (_, Err(e)) => return Value::Err(e),
        };
        let ic = match v.get(2).map(case_arg).unwrap_or(Ok(false)) { Ok(x) => x, Err(e) => return Value::Err(e) };
        match regex(&p, ic) {
            Ok(r) => Value::Bool(r.is_match(&t).unwrap_or(false)),
            Err(e) => Value::Err(e),
        }
    })
}

fn regexextract(ev: &mut Eval, a: &[Expr]) -> Res {
    let t = tri!(text(ev, &a[0]));
    let p = tri!(text(ev, &a[1]));
    let mode = tri!(opt_num(ev, a, 2, 0.0)).trunc() as i64;
    let ic = tri!(opt_num(ev, a, 3, 0.0)) != 0.0;
    let r = match regex(&p, ic) { Ok(r) => r, Err(e) => return Res::err(e) };
    match mode {
        0 => match r.find(&t) {
            Ok(Some(m)) => Res::V(Value::string(m.as_str().to_string())),
            _ => Res::err(XlErr::NA),
        },
        1 => {
            let all: Vec<Value> = r.find_iter(&t).filter_map(|m| m.ok()).map(|m| Value::string(m.as_str().to_string())).collect();
            if all.is_empty() { return Res::err(XlErr::NA); }
            let n = all.len();
            arr_val(Array::new(n, 1, all))
        }
        2 => match r.captures(&t) {
            Ok(Some(c)) if c.len() > 1 => {
                let g: Vec<Value> = (1..c.len()).map(|i| c.get(i).map(|m| Value::string(m.as_str().to_string())).unwrap_or(Value::string(String::new()))).collect();
                let n = g.len();
                arr_val(Array::new(1, n, g))
            }
            Ok(Some(_)) => Res::err(XlErr::NA),
            _ => Res::err(XlErr::NA),
        },
        _ => Res::err(XlErr::Value),
    }
}

/// REGEXREPLACE(text, pattern, replacement, [occurrence], [case]): $1 / ${name} in the
/// replacement name groups; occurrence 0 replaces all, n the nth, -n the nth from the end.
fn regexreplace(ev: &mut Eval, a: &[Expr]) -> Res {
    let t = tri!(text(ev, &a[0]));
    let p = tri!(text(ev, &a[1]));
    let rep = tri!(text(ev, &a[2]));
    let occ = tri!(opt_num(ev, a, 3, 0.0)).trunc() as i64;
    let ic = tri!(opt_num(ev, a, 4, 0.0)) != 0.0;
    let r = match regex(&p, ic) { Ok(r) => r, Err(e) => return Res::err(e) };
    let caps: Vec<fancy_regex::Captures> = r.captures_iter(&t).filter_map(|c| c.ok()).collect();
    let n = caps.len() as i64;
    let pick = |i: i64| occ == 0 || (occ > 0 && i + 1 == occ) || (occ < 0 && n + occ == i);
    let mut out = String::new();
    let mut last = 0;
    for (i, c) in caps.iter().enumerate() {
        let m = c.get(0).unwrap();
        if !pick(i as i64) {
            continue;
        }
        out.push_str(&t[last..m.start()]);
        c.expand(&rep, &mut out);
        last = m.end();
    }
    out.push_str(&t[last..]);
    if out.chars().count() > 32767 {
        return Res::err(XlErr::Value);
    }
    Res::V(Value::string(out))
}

// --------------------------------------------------------------------------- small ones

fn percentof(ev: &mut Eval, a: &[Expr]) -> Res {
    let part = tri!(collect(ev, &a[0..1], Mode::Plain));
    let all = tri!(collect(ev, &a[1..2], Mode::Plain));
    let (p, w): (f64, f64) = (part.iter().sum(), all.iter().sum());
    if w == 0.0 {
        return Res::err(XlErr::Div0);
    }
    Res::num(p / w)
}

/// TRIMRANGE(range, [rows], [cols]): the range without its empty rows and columns at the
/// start (1), the end (2) or both (3, the default).
fn trimrange(ev: &mut Eval, a: &[Expr]) -> Res {
    let r = ev.eval(&a[0]);
    let tr = tri!(opt_num(ev, a, 1, 3.0)).trunc() as i64;
    let tc = tri!(opt_num(ev, a, 2, 3.0)).trunc() as i64;
    if !(0..=3).contains(&tr) || !(0..=3).contains(&tc) {
        return Res::err(XlErr::Value);
    }
    let areas = match r {
        Res::R(x) if x.len() == 1 => x,
        Res::R(_) => return Res::err(XlErr::Value),
        Res::V(Value::Err(e)) => return Res::err(e),
        _ => return Res::err(XlErr::Value),
    };
    let ar = areas[0].clone();
    let rc = ev.bound(&ar);
    let filled: Vec<(u32, u32)> = ev.ctx.cells_in(ar.sheet, rc).into_iter().filter(|(_, _, v)| !v.is_empty()).map(|(r, c, _)| (r, c)).collect();
    if filled.is_empty() {
        return Res::err(XlErr::Ref);
    }
    let (minr, maxr) = (filled.iter().map(|x| x.0).min().unwrap(), filled.iter().map(|x| x.0).max().unwrap());
    let (minc, maxc) = (filled.iter().map(|x| x.1).min().unwrap(), filled.iter().map(|x| x.1).max().unwrap());
    let r1 = if tr & 1 != 0 { minr } else { ar.rect.r1 };
    let r2 = if tr & 2 != 0 { maxr } else { ar.rect.r2 };
    let c1 = if tc & 1 != 0 { minc } else { ar.rect.c1 };
    let c2 = if tc & 2 != 0 { maxc } else { ar.rect.c2 };
    Res::R(vec![Area { sheet: ar.sheet, rect: crate::cellref::Rect::new(r1, c1, r2, c2) }])
}

pub fn encode_url(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

/// The fixed conversion rates of the euro (units per euro) and the precision each currency
/// is calculated to.
const EURO: &[(&str, f64, i32)] = &[
    ("EUR", 1.0, 2), ("BEF", 40.3399, 0), ("LUF", 40.3399, 0), ("DEM", 1.95583, 2), ("ESP", 166.386, 0), ("FRF", 6.55957, 2),
    ("IEP", 0.787564, 2), ("ITL", 1936.27, 0), ("NLG", 2.20371, 2), ("ATS", 13.7603, 2), ("PTE", 200.482, 2), ("FIM", 5.94573, 2),
    ("GRD", 340.75, 2), ("SIT", 239.640, 2), ("CYP", 0.585274, 2), ("MTL", 0.429300, 2), ("SKK", 30.1260, 2), ("EEK", 15.6466, 2),
    ("LVL", 0.702804, 2), ("LTL", 3.45280, 2),
];

fn round_to(x: f64, d: i32) -> f64 {
    super::math::round_half_away(x, d as f64)
}

fn euroconvert(ev: &mut Eval, a: &[Expr]) -> Res {
    let x = tri!(num(ev, &a[0]));
    let src = tri!(text(ev, &a[1])).to_uppercase();
    let dst = tri!(text(ev, &a[2])).to_uppercase();
    let full = tri!(opt_bool(ev, a, 3, false));
    let tri_p = if is_missing(a, 4) { None } else { Some(tri!(num(ev, &a[4])).trunc() as i32) };
    if matches!(tri_p, Some(p) if p < 3) {
        return Res::err(XlErr::Value);
    }
    let find = |c: &str| EURO.iter().find(|e| e.0 == c).copied();
    let (s, d) = match (find(&src), find(&dst)) {
        (Some(s), Some(d)) => (s, d),
        _ => return Res::err(XlErr::Value),
    };
    let r = if s.0 == d.0 {
        x
    } else if s.0 == "EUR" {
        x * d.1
    } else if d.0 == "EUR" {
        x / s.1
    } else {
        // between two national currencies: through the euro
        let mut e = x / s.1;
        if let Some(p) = tri_p {
            // the intermediate euro amount, to that many significant digits
            if e != 0.0 {
                let mag = e.abs().log10().floor() as i32;
                e = round_to(e, p - 1 - mag);
            }
        }
        e * d.1
    };
    Res::num(if full { r } else { round_to(r, d.2) })
}

// --------------------------------------------------------------------------- GROUPBY / PIVOTBY

fn lit(v: &Value) -> Expr {
    match v {
        Value::Num(n) => Expr::Num(*n),
        Value::Str(s) => Expr::Str(s.clone()),
        Value::Bool(b) => Expr::Bool(*b),
        Value::Err(e) => Expr::Err(*e),
        Value::Empty => Expr::Missing,
        Value::Arr(a) => Expr::Array((0..a.rows).map(|r| (0..a.cols).map(|c| lit(a.get(r, c))).collect()).collect()),
    }
}

/// Call the function argument of GROUPBY / PIVOTBY on a column of values: a LAMBDA, or a
/// function named on its own (SUM, AVERAGE, PERCENTOF...).
fn apply_fn(ev: &mut Eval, f: &Expr, vals: Vec<Value>, all: Option<Vec<Value>>) -> Value {
    let n = vals.len();
    let arr = Value::Arr(Rc::new(Array::new(n.max(1), 1, if n == 0 { vec![Value::Empty] } else { vals })));
    let mut args = vec![arr];
    // the whole column goes along only to functions that take it (PERCENTOF, or a LAMBDA of two)
    let two = match f {
        Expr::Name(None, name) => name.eq_ignore_ascii_case("PERCENTOF"),
        _ => match ev.eval(f) { Res::L(l) => l.params.len() >= 2, _ => false },
    };
    let all = if two { all } else { None };
    if let Some(all) = all {
        let m = all.len();
        args.push(Value::Arr(Rc::new(Array::new(m.max(1), 1, if m == 0 { vec![Value::Empty] } else { all }))));
    }
    if let Expr::Name(None, name) = f {
        let up = name.to_ascii_uppercase();
        if let Some(d) = lookup_def(&up) {
            let exprs: Vec<Expr> = args.iter().take(d.max).map(lit).collect();
            let r = call(ev, &up, &exprs);
            return ev.res_value(r);
        }
    }
    match ev.eval(f) {
        Res::L(l) => {
            let k = l.params.len().min(args.len()).max(1);
            let r = ev.call_lambda_values(&l, args.into_iter().take(k).map(Res::V).collect());
            ev.res_value(r)
        }
        Res::V(Value::Err(e)) => Value::Err(e),
        _ => Value::Err(XlErr::Value),
    }
}

fn key_of(v: &Value) -> String {
    match v {
        Value::Str(s) => format!("t{}", s.to_lowercase()),
        Value::Num(n) => format!("n{}", n),
        Value::Bool(b) => format!("b{}", b),
        Value::Empty => "e".into(),
        Value::Err(e) => format!("x{}", e.text()),
        Value::Arr(_) => "a".into(),
    }
}

fn cmp_values(a: &Value, b: &Value) -> std::cmp::Ordering {
    let rank = |v: &Value| match v { Value::Num(_) => 0, Value::Str(_) => 1, Value::Bool(_) => 2, Value::Err(_) => 3, _ => 4 };
    rank(a).cmp(&rank(b)).then_with(|| match (a, b) {
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
        (Value::Str(x), Value::Str(y)) => x.to_lowercase().cmp(&y.to_lowercase()),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    })
}

fn rows_of(a: &Array) -> Vec<Vec<Value>> {
    (0..a.rows).map(|r| (0..a.cols).map(|c| a.get(r, c).clone()).collect()).collect()
}

/// field_headers: None = decide from the data (text above numbers), else 0..=3 as in the original.
fn headers_mode(arg: Option<i64>, vals: &Array) -> (bool, bool) {
    match arg {
        Some(0) => (false, false),
        Some(1) => (true, false),
        Some(2) => (false, true),
        Some(3) => (true, true),
        _ => {
            let first_text = (0..vals.cols).all(|c| matches!(vals.get(0, c), Value::Str(_)));
            let then_num = vals.rows > 1 && (0..vals.cols).any(|c| matches!(vals.get(1, c), Value::Num(_)));
            (first_text && then_num, false)
        }
    }
}

fn opt_int(ev: &mut Eval, a: &[Expr], i: usize) -> Result<Option<i64>, XlErr> {
    if is_missing(a, i) { Ok(None) } else { Ok(Some(num(ev, &a[i])?.trunc() as i64)) }
}

fn filter_of(ev: &mut Eval, a: &[Expr], i: usize, n: usize) -> Result<Option<Vec<bool>>, XlErr> {
    if is_missing(a, i) {
        return Ok(None);
    }
    let f = array_of(ev, &a[i])?;
    if f.data.len() != n {
        return Err(XlErr::Value);
    }
    Ok(Some(f.data.iter().map(|v| crate::calc::to_bool(v).unwrap_or(false)).collect()))
}

struct Group {
    label: Vec<Value>,
    rows: Vec<usize>,
}

fn group_rows(keys: &[Vec<Value>], take: &[usize], depth: usize) -> Vec<Group> {
    let mut order: Vec<String> = Vec::new();
    let mut map: std::collections::HashMap<String, Group> = std::collections::HashMap::new();
    for &i in take {
        let lab: Vec<Value> = keys[i][..depth].to_vec();
        let k: String = lab.iter().map(key_of).collect::<Vec<_>>().join("\u{1}");
        map.entry(k.clone()).or_insert_with(|| {
            order.push(k.clone());
            Group { label: lab, rows: Vec::new() }
        }).rows.push(i);
    }
    order.into_iter().map(|k| map.remove(&k).unwrap()).collect()
}

fn groupby(ev: &mut Eval, a: &[Expr]) -> Res {
    let rf = tri!(array_of(ev, &a[0]));
    let vf = tri!(array_of(ev, &a[1]));
    if rf.rows != vf.rows || rf.rows == 0 {
        return Res::err(XlErr::Value);
    }
    let hdr_arg = tri!(opt_int(ev, a, 3));
    let (has_hdr, show_hdr) = headers_mode(hdr_arg, &vf);
    let total = tri!(opt_int(ev, a, 4)).unwrap_or(1);
    let sort: Vec<i64> = if is_missing(a, 5) { vec![1] } else { tri!(array_of(ev, &a[5])).data.iter().map(|v| crate::calc::to_num(v, false).unwrap_or(1.0) as i64).collect() };
    let start = if has_hdr { 1 } else { 0 };
    let n = rf.rows - start;
    let filt = tri!(filter_of(ev, a, 6, rf.rows - start));
    let keys = rows_of(&rf);
    let vals = rows_of(&vf);
    let take: Vec<usize> = (start..rf.rows).filter(|i| filt.as_ref().map(|f| f[i - start]).unwrap_or(true)).collect();
    let _ = n;
    let depth = rf.cols;
    let mut groups = group_rows(&keys, &take, depth);
    // sort by the columns named (row fields first, then values); negative descends
    let mut results: Vec<Vec<Value>> = Vec::new();
    for g in &groups {
        let mut out = g.label.clone();
        for c in 0..vf.cols {
            let col: Vec<Value> = g.rows.iter().map(|&i| vals[i][c].clone()).collect();
            let all: Vec<Value> = take.iter().map(|&i| vals[i][c].clone()).collect();
            out.push(apply_fn(ev, &a[2], col, Some(all)));
        }
        results.push(out);
    }
    let mut idx: Vec<usize> = (0..groups.len()).collect();
    idx.sort_by(|&x, &y| {
        for s in &sort {
            if *s == 0 { continue; }
            let c = (s.unsigned_abs() as usize).saturating_sub(1);
            if c >= results[x].len() { continue; }
            let o = cmp_values(&results[x][c], &results[y][c]);
            let o = if *s < 0 { o.reverse() } else { o };
            if o != std::cmp::Ordering::Equal { return o; }
        }
        std::cmp::Ordering::Equal
    });
    groups = idx.iter().map(|&i| Group { label: groups[i].label.clone(), rows: groups[i].rows.clone() }).collect();
    let results: Vec<Vec<Value>> = idx.iter().map(|&i| results[i].clone()).collect();
    let width = depth + vf.cols;
    let mut out: Vec<Vec<Value>> = Vec::new();
    if show_hdr || (hdr_arg == Some(2)) {
        let mut h: Vec<Value> = if has_hdr { keys[0].clone() } else { (1..=depth).map(|i| Value::string(format!("Row Field {}", i))).collect() };
        for c in 0..vf.cols {
            h.push(if has_hdr { vals[0][c].clone() } else { Value::string(format!("Value {}", c + 1)) });
        }
        out.push(h);
    }
    let grand = |ev: &mut Eval| -> Vec<Value> {
        let mut t = vec![Value::string("Total".to_string())];
        t.extend(std::iter::repeat(Value::Empty).take(depth - 1));
        for c in 0..vf.cols {
            let col: Vec<Value> = take.iter().map(|&i| vals[i][c].clone()).collect();
            t.push(apply_fn(ev, &a[2], col.clone(), Some(col)));
        }
        t
    };
    if total < 0 {
        let g = grand(ev);
        out.push(g);
    }
    let sub = total.abs() >= 2 && depth >= 2;
    let mut i = 0;
    while i < results.len() {
        if sub {
            // subtotal of the first field's group
            let k0 = key_of(&groups[i].label[0]);
            let mut j = i;
            let mut rows = Vec::new();
            while j < results.len() && key_of(&groups[j].label[0]) == k0 {
                rows.extend(groups[j].rows.iter().copied());
                j += 1;
            }
            let mut st = vec![groups[i].label[0].clone()];
            st.extend(std::iter::repeat(Value::Empty).take(depth - 1));
            for c in 0..vf.cols {
                let col: Vec<Value> = rows.iter().map(|&r| vals[r][c].clone()).collect();
                st.push(apply_fn(ev, &a[2], col, None));
            }
            if total < 0 { out.push(st.clone()); }
            for r in &results[i..j] { out.push(r.clone()); }
            if total > 0 { out.push(st); }
            i = j;
        } else {
            out.push(results[i].clone());
            i += 1;
        }
    }
    if total > 0 {
        let g = grand(ev);
        out.push(g);
    }
    if out.is_empty() {
        return Res::err(XlErr::Calc);
    }
    let h = out.len();
    if !array_size_ok(h, width) {
        return Res::err(XlErr::Num);
    }
    arr_val(Array::new(h, width, out.into_iter().flatten().collect()))
}

fn pivotby(ev: &mut Eval, a: &[Expr]) -> Res {
    let rf = tri!(array_of(ev, &a[0]));
    let cf = tri!(array_of(ev, &a[1]));
    let vf = tri!(array_of(ev, &a[2]));
    if rf.rows != vf.rows || cf.rows != vf.rows || vf.rows == 0 {
        return Res::err(XlErr::Value);
    }
    let hdr_arg = tri!(opt_int(ev, a, 4));
    let (has_hdr, _show) = headers_mode(hdr_arg, &vf);
    let rtot = tri!(opt_int(ev, a, 5)).unwrap_or(1);
    let rsort = tri!(opt_int(ev, a, 6)).unwrap_or(1);
    let ctot = tri!(opt_int(ev, a, 7)).unwrap_or(1);
    let csort = tri!(opt_int(ev, a, 8)).unwrap_or(1);
    let start = if has_hdr { 1 } else { 0 };
    let filt = tri!(filter_of(ev, a, 9, vf.rows - start));
    let take: Vec<usize> = (start..vf.rows).filter(|i| filt.as_ref().map(|f| f[i - start]).unwrap_or(true)).collect();
    let (rk, ck, vals) = (rows_of(&rf), rows_of(&cf), rows_of(&vf));
    let mut rg = group_rows(&rk, &take, rf.cols);
    let mut cg = group_rows(&ck, &take, cf.cols);
    let sort_groups = |g: &mut Vec<Group>, s: i64| {
        if s != 0 {
            g.sort_by(|x, y| {
                let o = x.label.iter().zip(y.label.iter()).map(|(p, q)| cmp_values(p, q)).find(|o| *o != std::cmp::Ordering::Equal).unwrap_or(std::cmp::Ordering::Equal);
                if s < 0 { o.reverse() } else { o }
            });
        }
    };
    sort_groups(&mut rg, rsort);
    sort_groups(&mut cg, csort);
    let nv = vf.cols;
    let lw = rf.cols;
    let hdr_rows = cf.cols + if nv > 1 { 1 } else { 0 };
    let ncols_groups = cg.len() + if ctot != 0 { 1 } else { 0 };
    let width = lw + ncols_groups * nv;
    let mut out: Vec<Vec<Value>> = Vec::new();
    for level in 0..cf.cols {
        let mut row = vec![Value::Empty; lw];
        for g in &cg {
            for _ in 0..nv { row.push(g.label[level].clone()); }
        }
        if ctot != 0 {
            for _ in 0..nv { row.push(if level == 0 { Value::string("Total".to_string()) } else { Value::Empty }); }
        }
        out.push(row);
    }
    if nv > 1 {
        let mut row = vec![Value::Empty; lw];
        for _ in 0..ncols_groups {
            for c in 0..nv { row.push(if has_hdr { vals[0][c].clone() } else { Value::string(format!("Value {}", c + 1)) }); }
        }
        out.push(row);
    }
    let cell = |ev: &mut Eval, rows: &[usize], col: Option<&Group>, c: usize| -> Value {
        let picked: Vec<Value> = rows.iter().filter(|i| col.map(|g| g.rows.contains(i)).unwrap_or(true)).map(|&i| vals[i][c].clone()).collect();
        if picked.is_empty() {
            // no records where this row and column meet: the cell stays empty
            return Value::Empty;
        }
        let all: Vec<Value> = take.iter().map(|&i| vals[i][c].clone()).collect();
        apply_fn(ev, &a[3], picked, Some(all))
    };
    let mut body: Vec<Vec<Value>> = Vec::new();
    for g in &rg {
        let mut row = g.label.clone();
        for col in &cg {
            for c in 0..nv { row.push(cell(ev, &g.rows, Some(col), c)); }
        }
        if ctot != 0 {
            for c in 0..nv { row.push(cell(ev, &g.rows, None, c)); }
        }
        body.push(row);
    }
    if rtot != 0 {
        let mut row = vec![Value::string("Total".to_string())];
        row.extend(std::iter::repeat(Value::Empty).take(lw - 1));
        for col in &cg {
            for c in 0..nv { row.push(cell(ev, &take, Some(col), c)); }
        }
        if ctot != 0 {
            for c in 0..nv { row.push(cell(ev, &take, None, c)); }
        }
        if rtot < 0 { body.insert(0, row); } else { body.push(row); }
    }
    out.extend(body);
    let _ = hdr_rows;
    let h = out.len();
    if !array_size_ok(h, width) {
        return Res::err(XlErr::Num);
    }
    arr_val(Array::new(h, width, out.into_iter().flatten().collect()))
}

// --------------------------------------------------------------------------- FILTERXML

/// The XPath the original takes in practice: /a/b, //b, a/@attr, text(), and predicates
/// [n], [@attr], [@attr='v'], [child='v'].
fn filterxml(ev: &mut Eval, a: &[Expr]) -> Res {
    let x = tri!(text(ev, &a[0]));
    let path = tri!(text(ev, &a[1]));
    let root = match crate::xml::parse(&x) {
        Ok((_, e)) => e,
        Err(_) => return Res::err(XlErr::Value),
    };
    let found = match xpath(&root, &path) {
        Some(v) => v,
        None => return Res::err(XlErr::Value),
    };
    if found.is_empty() {
        return Res::err(XlErr::Value);
    }
    let vals: Vec<Value> = found.into_iter().map(|t| match crate::input::parse_number_text(&t, false) {
        Some((n, _)) if t.trim() == t && !t.is_empty() && t.parse::<f64>().is_ok() => Value::num(n),
        _ => Value::string(t),
    }).collect();
    let n = vals.len();
    arr_val(Array::new(n, 1, vals))
}

enum Step {
    Child(String, Vec<String>),
    Desc(String, Vec<String>),
    Attr(String),
    Text,
}

fn parse_xpath(p: &str) -> Option<Vec<Step>> {
    let mut steps = Vec::new();
    let mut s = p.trim();
    if !s.starts_with('/') {
        s = s.strip_prefix("./").unwrap_or(s);
        let rest = format!("/{}", s);
        return parse_xpath(&rest);
    }
    while !s.is_empty() {
        let desc = s.starts_with("//");
        s = if desc { &s[2..] } else { &s[1..] };
        // the step runs to the next '/' outside brackets
        let mut depth = 0;
        let mut end = s.len();
        let mut quote: Option<char> = None;
        for (i, ch) in s.char_indices() {
            match (quote, ch) {
                (Some(q), c) if c == q => quote = None,
                (Some(_), _) => {}
                (None, '\'') | (None, '"') => quote = Some(ch),
                (None, '[') => depth += 1,
                (None, ']') => depth -= 1,
                (None, '/') if depth == 0 => { end = i; break; }
                _ => {}
            }
        }
        let step = &s[..end];
        s = &s[end..];
        if step == "text()" {
            steps.push(Step::Text);
            continue;
        }
        if let Some(at) = step.strip_prefix('@') {
            steps.push(Step::Attr(at.to_string()));
            continue;
        }
        let (name, preds) = match step.find('[') {
            Some(i) => (step[..i].to_string(), step[i..].split(']').filter(|x| !x.is_empty()).map(|x| x.trim_start_matches('[').to_string()).collect()),
            None => (step.to_string(), Vec::new()),
        };
        if name.is_empty() {
            return None;
        }
        steps.push(if desc { Step::Desc(name, preds) } else { Step::Child(name, preds) });
    }
    Some(steps)
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    t.trim_matches(|c| c == '\'' || c == '"').to_string()
}

fn pred_ok(e: &crate::xml::Element, pred: &str) -> bool {
    if let Some(rest) = pred.strip_prefix('@') {
        return match rest.split_once('=') {
            Some((k, v)) => e.attr(k.trim()).map(|x| x == unquote(v)).unwrap_or(false),
            None => e.attr(rest.trim()).is_some(),
        };
    }
    if let Some((k, v)) = pred.split_once('=') {
        let want = unquote(v);
        return e.elems().any(|c| c.local() == k.trim() && c.text() == want);
    }
    true
}

fn apply_preds<'a>(list: Vec<&'a crate::xml::Element>, preds: &[String]) -> Vec<&'a crate::xml::Element> {
    let mut cur = list;
    for p in preds {
        if let Ok(n) = p.trim().parse::<usize>() {
            cur = cur.get(n.wrapping_sub(1)).copied().into_iter().collect();
        } else if p.trim() == "last()" {
            cur = cur.last().copied().into_iter().collect();
        } else {
            cur.retain(|e| pred_ok(e, p));
        }
    }
    cur
}

fn descendants<'a>(e: &'a crate::xml::Element, name: &str, out: &mut Vec<&'a crate::xml::Element>) {
    for c in e.elems() {
        if name == "*" || c.local() == name {
            out.push(c);
        }
        descendants(c, name, out);
    }
}

fn xpath(root: &crate::xml::Element, p: &str) -> Option<Vec<String>> {
    let steps = parse_xpath(p)?;
    // a virtual document node above the root element
    let doc = crate::xml::Element { name: String::new(), attrs: vec![], children: vec![crate::xml::Node::Elem(root.clone())] };
    let mut cur: Vec<&crate::xml::Element> = vec![&doc];
    let mut texts: Option<Vec<String>> = None;
    for st in &steps {
        match st {
            Step::Child(n, preds) => {
                let mut next = Vec::new();
                for e in &cur {
                    let kids: Vec<&crate::xml::Element> = e.elems().filter(|c| n == "*" || c.local() == n).collect();
                    next.extend(apply_preds(kids, preds));
                }
                cur = next;
            }
            Step::Desc(n, preds) => {
                let mut next = Vec::new();
                for e in &cur {
                    let mut found = Vec::new();
                    descendants(e, n, &mut found);
                    next.extend(apply_preds(found, preds));
                }
                cur = next;
            }
            Step::Attr(n) => {
                texts = Some(cur.iter().filter_map(|e| e.attr(n).map(|s| s.to_string())).collect());
                cur = Vec::new();
            }
            Step::Text => {
                texts = Some(cur.iter().map(|e| e.text()).collect());
                cur = Vec::new();
            }
        }
    }
    Some(texts.unwrap_or_else(|| cur.iter().map(|e| e.text()).collect()))
}

// --------------------------------------------------------------------------- GETPIVOTDATA

fn getpivotdata(ev: &mut Eval, a: &[Expr]) -> Res {
    let field = tri!(text(ev, &a[0]));
    let at = match ev.eval(&a[1]) {
        Res::R(x) if !x.is_empty() => x[0].clone(),
        Res::V(Value::Err(e)) => return Res::err(e),
        _ => return Res::err(XlErr::Ref),
    };
    if (a.len() - 2) % 2 != 0 {
        return Res::err(XlErr::Ref);
    }
    let mut pairs = Vec::new();
    for k in (2..a.len()).step_by(2) {
        let f = tri!(text(ev, &a[k]));
        let v = ev.eval_scalar(&a[k + 1]);
        if let Value::Err(e) = v {
            return Res::err(e);
        }
        pairs.push((f, v));
    }
    match ev.ctx.pivot_data(at.sheet, at.rect.r1, at.rect.c1, &field, &pairs) {
        Some(v) => Res::V(v),
        None => Res::err(XlErr::Ref),
    }
}
