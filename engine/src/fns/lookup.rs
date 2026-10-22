//! Lookup, reference and dynamic-array functions.

use super::*;
use crate::calc::{compare, Area, Eval, Res};
use crate::cellref::{col_name, Rect, MAX_COLS, MAX_ROWS};
use crate::value::{Array, Value, XlErr};
use std::cmp::Ordering;
use std::rc::Rc;

pub static DEFS: &[Def] = &[
    d!("VLOOKUP", 3, 4, |ev, a| vhlookup(ev, a, true)),
    d!("HLOOKUP", 3, 4, |ev, a| vhlookup(ev, a, false)),
    d!("LOOKUP", 2, 3, lookup),
    d!("MATCH", 2, 3, matchf),
    d!("XMATCH", 2, 4, xmatch),
    d!("XLOOKUP", 3, 6, xlookup),
    d!("INDEX", 1, 4, index),
    d!("CHOOSE", 2, MANY, choose),
    d!("OFFSET", 3, 5, offset, vol),
    d!("INDIRECT", 1, 2, indirect, vol),
    d!("ROW", 0, 1, |ev, a| rowcol(ev, a, true)),
    d!("COLUMN", 0, 1, |ev, a| rowcol(ev, a, false)),
    d!("ROWS", 1, 1, |ev, a| dims(ev, a, true)),
    d!("COLUMNS", 1, 1, |ev, a| dims(ev, a, false)),
    d!("AREAS", 1, 1, |ev, a| match ev.eval(&a[0]) { Res::R(x) => Res::num(x.len() as f64), Res::V(Value::Err(e)) => Res::err(e), _ => Res::err(XlErr::Value) }),
    d!("ADDRESS", 2, 5, address),
    d!("TRANSPOSE", 1, 1, |ev, a| {
        let arr = tri!(array_of(ev, &a[0]));
        let mut data = Vec::with_capacity(arr.data.len());
        for c in 0..arr.cols { for r in 0..arr.rows { data.push(arr.get(r, c).clone()); } }
        arr_val(Array::new(arr.cols, arr.rows, data))
    }),
    d!("FORMULATEXT", 1, 1, |ev, a| match ev.eval(&a[0]) {
        Res::R(x) => match ev.ctx.formula_text(x[0].sheet, x[0].rect.r1, x[0].rect.c1) { Some(t) => Res::V(Value::string(t)), None => Res::err(XlErr::NA) },
        Res::V(Value::Err(e)) => Res::err(e),
        _ => Res::err(XlErr::NA),
    }),
    d!("HYPERLINK", 1, 2, |ev, a| { if a.len() > 1 { Res::V(ev.eval_scalar(&a[1])) } else { Res::V(ev.eval_scalar(&a[0])) } }),
    d!("FILTER", 2, 3, filter),
    d!("SORT", 1, 4, sort),
    d!("SORTBY", 2, MANY, sortby),
    d!("UNIQUE", 1, 3, unique),
    d!("TAKE", 2, 3, |ev, a| take_drop(ev, a, true)),
    d!("DROP", 2, 3, |ev, a| take_drop(ev, a, false)),
    d!("CHOOSEROWS", 2, MANY, |ev, a| choose_rc(ev, a, true)),
    d!("CHOOSECOLS", 2, MANY, |ev, a| choose_rc(ev, a, false)),
    d!("VSTACK", 1, MANY, |ev, a| stack(ev, a, true)),
    d!("HSTACK", 1, MANY, |ev, a| stack(ev, a, false)),
    d!("TOCOL", 1, 3, |ev, a| tocolrow(ev, a, true)),
    d!("TOROW", 1, 3, |ev, a| tocolrow(ev, a, false)),
    d!("WRAPROWS", 2, 3, |ev, a| wrap(ev, a, true)),
    d!("WRAPCOLS", 2, 3, |ev, a| wrap(ev, a, false)),
    d!("EXPAND", 2, 4, |ev, a| {
        let arr = tri!(array_of(ev, &a[0]));
        let rows = if is_missing(a, 1) { arr.rows } else { tri!(num(ev, &a[1])).trunc() as usize };
        let cols = if is_missing(a, 2) { arr.cols } else { tri!(num(ev, &a[2])).trunc() as usize };
        if rows < arr.rows || cols < arr.cols { return Res::err(XlErr::Value); }
        let pad = if is_missing(a, 3) { Value::Err(XlErr::NA) } else { ev.eval_scalar(&a[3]) };
        let mut data = Vec::with_capacity(rows * cols);
        for r in 0..rows { for c in 0..cols { data.push(if r < arr.rows && c < arr.cols { arr.get(r, c).clone() } else { pad.clone() }); } }
        arr_val(Array::new(rows, cols, data))
    }),
];

// --------------------------------------------------------------------------- matching

/// Equal for lookups: same kind, numbers equal, text equal ignoring case (optionally with wildcards).
fn eq_lookup(target: &Value, v: &Value, wild: bool) -> bool {
    match (target, v) {
        (Value::Num(a), Value::Num(b)) => crate::calc::num_eq(*a, *b),
        (Value::Str(a), Value::Str(b)) => {
            if wild && (a.contains('*') || a.contains('?') || a.contains('~')) {
                wildcard_match(&a.to_lowercase(), &b.to_lowercase())
            } else {
                a.to_lowercase() == b.to_lowercase()
            }
        }
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Err(a), Value::Err(b)) => a == b,
        (Value::Empty, Value::Empty) => true,
        (Value::Str(a), Value::Empty) => a.is_empty(),
        _ => false,
    }
}

fn same_kind(a: &Value, b: &Value) -> bool {
    matches!((a, b), (Value::Num(_), Value::Num(_)) | (Value::Str(_), Value::Str(_)) | (Value::Bool(_), Value::Bool(_)))
}

/// Position of a value in a vector. mode 0 exact (wildcards), 1 largest <= (sorted
/// ascending), -1 smallest >= (sorted descending).
pub fn find_pos(v: &[Value], target: &Value, mode: i32) -> Option<usize> {
    match mode {
        0 => v.iter().position(|x| eq_lookup(target, x, true)),
        1 => {
            // binary search as the original does on ascending data
            let (mut lo, mut hi) = (0i64, v.len() as i64 - 1);
            let mut best: Option<usize> = None;
            while lo <= hi {
                let mid = (lo + hi) / 2;
                let mut m = mid;
                // step over values of another kind
                while m >= lo && !same_kind(&v[m as usize], target) {
                    m -= 1;
                }
                if m < lo {
                    lo = mid + 1;
                    continue;
                }
                match compare(&v[m as usize], target) {
                    Ordering::Greater => hi = m - 1,
                    _ => {
                        best = Some(m as usize);
                        lo = mid + 1;
                    }
                }
            }
            best
        }
        _ => {
            let mut best: Option<usize> = None;
            for (i, x) in v.iter().enumerate() {
                if !same_kind(x, target) {
                    continue;
                }
                match compare(x, target) {
                    Ordering::Less => break,
                    _ => best = Some(i),
                }
            }
            best
        }
    }
}

fn line(arr: &Array, i: usize, row: bool) -> Vec<Value> {
    if row {
        (0..arr.cols).map(|c| arr.get(i, c).clone()).collect()
    } else {
        (0..arr.rows).map(|r| arr.get(r, i).clone()).collect()
    }
}

fn vector(arr: &Array) -> Result<Vec<Value>, XlErr> {
    if arr.rows == 1 {
        Ok(arr.data.clone())
    } else if arr.cols == 1 {
        Ok(arr.data.clone())
    } else {
        Err(XlErr::NA)
    }
}

fn vhlookup(ev: &mut Eval, a: &[Expr], vertical: bool) -> Res {
    let target = ev.eval_value(&a[0]);
    let table = tri!(array_of(ev, &a[1]));
    let approx = tri!(opt_bool(ev, a, 3, true));
    let idx_v = ev.eval_value(&a[2]);
    let keys = line(&table, 0, !vertical);
    let one = |ev: &mut Eval, t: &Value, idx: &Value| -> Value {
        if let Value::Err(e) = t {
            return Value::Err(*e);
        }
        let k = match vnum(ev, idx) {
            Ok(k) => k.trunc() as i64,
            Err(e) => return Value::Err(e),
        };
        let width = if vertical { table.cols } else { table.rows } as i64;
        if k < 1 {
            return Value::Err(XlErr::Value);
        }
        if k > width {
            return Value::Err(XlErr::Ref);
        }
        let pos = if approx { find_pos(&keys, t, 1) } else { find_pos(&keys, t, 0) };
        match pos {
            Some(p) => {
                let v = if vertical { table.get(p, k as usize - 1) } else { table.get(k as usize - 1, p) };
                match v {
                    Value::Empty => Value::num(0.0),
                    v => v.clone(),
                }
            }
            None => Value::Err(XlErr::NA),
        }
    };
    let vals = vec![target, idx_v];
    Res::V(lift_values(ev, &vals, |ev, v| one(ev, &v[0], &v[1])))
}

fn lookup(ev: &mut Eval, a: &[Expr]) -> Res {
    let target = ev.eval_scalar(&a[0]);
    let arr = tri!(array_of(ev, &a[1]));
    let (keys, result) = if a.len() > 2 {
        let res = tri!(array_of(ev, &a[2]));
        let k = if arr.rows == 1 || arr.cols == 1 { arr.data.clone() } else if arr.cols > arr.rows { line(&arr, 0, true) } else { line(&arr, 0, false) };
        (k, tri!(vector(&res)))
    } else if arr.cols > arr.rows {
        (line(&arr, 0, true), line(&arr, arr.rows - 1, true))
    } else {
        (line(&arr, 0, false), line(&arr, arr.cols - 1, false))
    };
    match find_pos(&keys, &target, 1) {
        Some(p) => Res::V(result.get(p).cloned().unwrap_or(Value::Err(XlErr::NA))),
        None => Res::err(XlErr::NA),
    }
}

fn matchf(ev: &mut Eval, a: &[Expr]) -> Res {
    let target = ev.eval_value(&a[0]);
    let arr = tri!(array_of(ev, &a[1]));
    let keys = tri!(vector(&arr));
    let mode = tri!(opt_num(ev, a, 2, 1.0)) as i32;
    let mode = mode.signum();
    let one = |t: &Value| -> Value {
        if let Value::Err(e) = t {
            return Value::Err(*e);
        }
        match find_pos(&keys, t, mode) {
            Some(p) => Value::num((p + 1) as f64),
            None => Value::Err(XlErr::NA),
        }
    };
    Res::V(match target {
        Value::Arr(t) => Value::Arr(Rc::new(Array::new(t.rows, t.cols, t.data.iter().map(one).collect()))),
        t => one(&t),
    })
}

/// XMATCH/XLOOKUP search: match_mode 0 exact, -1 exact or next smaller, 1 exact or next
/// larger, 2 wildcard; search_mode 1 first, -1 last, 2 binary ascending, -2 binary descending.
fn xfind(keys: &[Value], t: &Value, match_mode: i32, search: i32) -> Option<usize> {
    let order: Box<dyn Iterator<Item = usize>> = if search == -1 || search == -2 { Box::new((0..keys.len()).rev()) } else { Box::new(0..keys.len()) };
    let mut best: Option<usize> = None;
    for i in order {
        let k = &keys[i];
        match match_mode {
            0 => {
                if eq_lookup(t, k, false) {
                    return Some(i);
                }
            }
            2 => {
                if eq_lookup(t, k, true) {
                    return Some(i);
                }
            }
            -1 | 1 => {
                if eq_lookup(t, k, false) {
                    return Some(i);
                }
                if !same_kind(k, t) {
                    continue;
                }
                let o = compare(k, t);
                let ok = if match_mode == -1 { o == Ordering::Less } else { o == Ordering::Greater };
                if ok {
                    best = match best {
                        None => Some(i),
                        Some(b) => {
                            let better = compare(k, &keys[b]);
                            if (match_mode == -1 && better == Ordering::Greater) || (match_mode == 1 && better == Ordering::Less) { Some(i) } else { Some(b) }
                        }
                    };
                }
            }
            _ => {}
        }
    }
    best
}

fn xmatch(ev: &mut Eval, a: &[Expr]) -> Res {
    let target = ev.eval_value(&a[0]);
    let arr = tri!(array_of(ev, &a[1]));
    let keys = tri!(vector(&arr));
    let mm = tri!(opt_num(ev, a, 2, 0.0)) as i32;
    let sm = tri!(opt_num(ev, a, 3, 1.0)) as i32;
    if !(-1..=2).contains(&mm) || ![1, -1, 2, -2].contains(&sm) {
        return Res::err(XlErr::Value);
    }
    let one = |t: &Value| -> Value {
        match xfind(&keys, t, mm, sm) {
            Some(p) => Value::num((p + 1) as f64),
            None => Value::Err(XlErr::NA),
        }
    };
    Res::V(match target {
        Value::Arr(t) => Value::Arr(Rc::new(Array::new(t.rows, t.cols, t.data.iter().map(one).collect()))),
        t => one(&t),
    })
}

fn xlookup(ev: &mut Eval, a: &[Expr]) -> Res {
    let target = ev.eval_scalar(&a[0]);
    let la = tri!(array_of(ev, &a[1]));
    let ra = tri!(array_of(ev, &a[2]));
    let mm = tri!(opt_num(ev, a, 4, 0.0)) as i32;
    let sm = tri!(opt_num(ev, a, 5, 1.0)) as i32;
    let vertical = la.cols == 1;
    if la.rows != 1 && la.cols != 1 {
        return Res::err(XlErr::Value);
    }
    if (vertical && ra.rows != la.rows) || (!vertical && ra.cols != la.cols) {
        return Res::err(XlErr::Value);
    }
    if let Value::Err(e) = target {
        return Res::err(e);
    }
    match xfind(&la.data, &target, mm, sm) {
        Some(p) => {
            if vertical {
                let row: Vec<Value> = (0..ra.cols).map(|c| ra.get(p, c).clone()).collect();
                let n = row.len();
                arr_val(Array::new(1, n, row))
            } else {
                let col: Vec<Value> = (0..ra.rows).map(|r| ra.get(r, p).clone()).collect();
                let n = col.len();
                arr_val(Array::new(n, 1, col))
            }
        }
        None => {
            if is_missing(a, 3) {
                Res::err(XlErr::NA)
            } else {
                let v = ev.eval(&a[3]);
                match v {
                    Res::R(_) => {
                        let x = ev.res_value(v);
                        Res::V(x)
                    }
                    other => other,
                }
            }
        }
    }
}

fn index(ev: &mut Eval, a: &[Expr]) -> Res {
    let r = ev.eval(&a[0]);
    let row = tri!(opt_num(ev, a, 1, 0.0)).trunc() as i64;
    let col = tri!(opt_num(ev, a, 2, 0.0)).trunc() as i64;
    let area_n = tri!(opt_num(ev, a, 3, 1.0)).trunc() as i64;
    if row < 0 || col < 0 {
        return Res::err(XlErr::Value);
    }
    match r {
        Res::R(areas) => {
            if area_n < 1 || area_n as usize > areas.len() {
                return Res::err(XlErr::Ref);
            }
            let ar = &areas[area_n as usize - 1];
            let rc = ar.rect;
            // one row or column: a single index picks along it
            let (row, col) = if a.len() == 2 || (a.len() >= 3 && matches!(a[2], Expr::Missing) && false) {
                if rc.rows() == 1 { (1, row) } else if rc.cols() == 1 { (row, 1) } else { (row, 0) }
            } else {
                (row, col)
            };
            if row as u32 > rc.rows() || col as u32 > rc.cols() {
                return Res::err(XlErr::Ref);
            }
            let (r1, r2) = if row == 0 { (rc.r1, rc.r2) } else { (rc.r1 + row as u32 - 1, rc.r1 + row as u32 - 1) };
            let (c1, c2) = if col == 0 { (rc.c1, rc.c2) } else { (rc.c1 + col as u32 - 1, rc.c1 + col as u32 - 1) };
            Res::R(vec![Area { sheet: ar.sheet, rect: Rect::new(r1, c1, r2, c2) }])
        }
        r => {
            let arr = tri!(res_array(ev, r));
            let (row, col) = if a.len() == 2 {
                if arr.rows == 1 { (1, row) } else if arr.cols == 1 { (row, 1) } else { (row, 0) }
            } else {
                (row, col)
            };
            if row as usize > arr.rows || col as usize > arr.cols {
                return Res::err(XlErr::Ref);
            }
            if row == 0 && col == 0 {
                return Res::V(Value::Arr(arr));
            }
            if row == 0 {
                let v: Vec<Value> = (0..arr.rows).map(|r| arr.get(r, col as usize - 1).clone()).collect();
                let n = v.len();
                return arr_val(Array::new(n, 1, v));
            }
            if col == 0 {
                let v: Vec<Value> = (0..arr.cols).map(|c| arr.get(row as usize - 1, c).clone()).collect();
                let n = v.len();
                return arr_val(Array::new(1, n, v));
            }
            Res::V(arr.get(row as usize - 1, col as usize - 1).clone())
        }
    }
}

fn choose(ev: &mut Eval, a: &[Expr]) -> Res {
    let i = ev.eval_value(&a[0]);
    match i {
        Value::Arr(idx) => {
            // CHOOSE({1,2}, x, y): element-wise
            let mut opts: Vec<Value> = Vec::new();
            for e in &a[1..] {
                opts.push(ev.eval_value(e));
            }
            let mut data = Vec::new();
            for (k, v) in idx.data.iter().enumerate() {
                let n = match crate::calc::to_num(v, ev.d1904()) {
                    Ok(n) => n.trunc() as usize,
                    Err(e) => {
                        data.push(Value::Err(e));
                        continue;
                    }
                };
                if n < 1 || n > opts.len() {
                    data.push(Value::Err(XlErr::Value));
                    continue;
                }
                data.push(match &opts[n - 1] {
                    Value::Arr(x) => x.get_b(k / idx.cols, k % idx.cols),
                    x => x.clone(),
                });
            }
            Res::V(Value::Arr(Rc::new(Array::new(idx.rows, idx.cols, data))))
        }
        v => {
            let n = match crate::calc::to_num(&v, ev.d1904()) {
                Ok(n) => n.trunc() as usize,
                Err(e) => return Res::err(e),
            };
            if n < 1 || n >= a.len() {
                return Res::err(XlErr::Value);
            }
            ev.eval(&a[n])
        }
    }
}

fn offset(ev: &mut Eval, a: &[Expr]) -> Res {
    let areas = tri!(areas_of(ev, &a[0]));
    if areas.len() != 1 {
        return Res::err(XlErr::Value);
    }
    let base = &areas[0];
    let dr = tri!(num(ev, &a[1])).trunc() as i64;
    let dc = tri!(num(ev, &a[2])).trunc() as i64;
    let h = if is_missing(a, 3) { base.rect.rows() as i64 } else { tri!(num(ev, &a[3])).trunc() as i64 };
    let w = if is_missing(a, 4) { base.rect.cols() as i64 } else { tri!(num(ev, &a[4])).trunc() as i64 };
    if h == 0 || w == 0 {
        return Res::err(XlErr::Ref);
    }
    let r1 = base.rect.r1 as i64 + dr;
    let c1 = base.rect.c1 as i64 + dc;
    let (r1, r2) = if h > 0 { (r1, r1 + h - 1) } else { (r1 + h + 1, r1) };
    let (c1, c2) = if w > 0 { (c1, c1 + w - 1) } else { (c1 + w + 1, c1) };
    if r1 < 0 || c1 < 0 || r2 >= MAX_ROWS as i64 || c2 >= MAX_COLS as i64 {
        return Res::err(XlErr::Ref);
    }
    Res::R(vec![Area { sheet: base.sheet, rect: Rect::new(r1 as u32, c1 as u32, r2 as u32, c2 as u32) }])
}

fn indirect(ev: &mut Eval, a: &[Expr]) -> Res {
    let t = tri!(text(ev, &a[0]));
    let a1 = tri!(opt_bool(ev, a, 1, true));
    let t = t.trim();
    if !a1 {
        return r1c1_ref(ev, t);
    }
    let toks = match crate::formula::tokenize(t) {
        Ok(x) => x,
        Err(_) => return Res::err(XlErr::Ref),
    };
    let toks: Vec<_> = toks.into_iter().filter(|x| x.kind != crate::formula::TokKind::Space).collect();
    match toks.as_slice() {
        [one] if one.kind == crate::formula::TokKind::Ref => match crate::formula::parse_ref_tok(&one.text) {
            Some(r) => ev.eval_ref(&r),
            None => Res::err(XlErr::Ref),
        },
        [one] if one.kind == crate::formula::TokKind::Name => {
            let (sheet, _, name) = crate::formula::split_sheet(&one.text);
            match ev.eval(&Expr::Name(sheet, name.to_string())) {
                r @ Res::R(_) => r,
                _ => Res::err(XlErr::Ref),
            }
        }
        _ => match crate::formula::parse(t) {
            Ok(e @ Expr::Range(..)) => match ev.eval(&e) {
                r @ Res::R(_) => r,
                _ => Res::err(XlErr::Ref),
            },
            _ => Res::err(XlErr::Ref),
        },
    }
}

fn r1c1_ref(ev: &mut Eval, t: &str) -> Res {
    let (sheet, body) = match t.rfind('!') {
        Some(i) => (Some(t[..i].trim_matches('\'').to_string()), &t[i + 1..]),
        None => (None, t),
    };
    let sh = match sheet {
        Some(s) => match ev.ctx.sheet_index(&s) {
            Some(i) => i,
            None => return Res::err(XlErr::Ref),
        },
        None => ev.sheet,
    };
    let part = |s: &str, base_r: u32, base_c: u32| -> Option<(u32, u32)> {
        let u = s.to_ascii_uppercase();
        let ri = u.find('R')?;
        let ci = u.find('C')?;
        if ri != 0 || ci < 1 {
            return None;
        }
        let rs = &u[1..ci];
        let cs = &u[ci + 1..];
        let get = |x: &str, base: u32| -> Option<u32> {
            if x.is_empty() {
                return Some(base);
            }
            if let Some(rel) = x.strip_prefix('[').and_then(|y| y.strip_suffix(']')) {
                let d: i64 = rel.parse().ok()?;
                let v = base as i64 + d;
                return if v < 0 { None } else { Some(v as u32) };
            }
            let v: u32 = x.parse().ok()?;
            if v == 0 { None } else { Some(v - 1) }
        };
        Some((get(rs, base_r)?, get(cs, base_c)?))
    };
    let (a, b) = match body.split_once(':') {
        Some((x, y)) => (x, Some(y)),
        None => (body, None),
    };
    let p1 = match part(a, ev.row, ev.col) {
        Some(p) => p,
        None => return Res::err(XlErr::Ref),
    };
    let p2 = match b.map(|y| part(y, ev.row, ev.col)) {
        Some(Some(p)) => p,
        Some(None) => return Res::err(XlErr::Ref),
        None => p1,
    };
    Res::R(vec![Area { sheet: sh, rect: Rect::new(p1.0, p1.1, p2.0, p2.1) }])
}

fn rowcol(ev: &mut Eval, a: &[Expr], row: bool) -> Res {
    if a.is_empty() || matches!(a[0], Expr::Missing) {
        return Res::num(if row { ev.row + 1 } else { ev.col + 1 } as f64);
    }
    let areas = tri!(areas_of(ev, &a[0]));
    let rc = areas[0].rect;
    let n = if row { rc.rows() } else { rc.cols() };
    if n == 1 || ev.legacy {
        return Res::num(if row { rc.r1 + 1 } else { rc.c1 + 1 } as f64);
    }
    let n = n.min(1_048_576) as usize;
    let v: Vec<Value> = (0..n).map(|i| Value::num(if row { rc.r1 } else { rc.c1 } as f64 + i as f64 + 1.0)).collect();
    if row {
        arr_val(Array::new(n, 1, v))
    } else {
        arr_val(Array::new(1, n, v))
    }
}

fn dims(ev: &mut Eval, a: &[Expr], rows: bool) -> Res {
    match ev.eval(&a[0]) {
        Res::R(areas) => {
            if areas.len() != 1 {
                return Res::err(XlErr::Ref);
            }
            Res::num(if rows { areas[0].rect.rows() } else { areas[0].rect.cols() } as f64)
        }
        Res::V(Value::Arr(arr)) => Res::num(if rows { arr.rows } else { arr.cols } as f64),
        Res::V(Value::Err(e)) => Res::err(e),
        Res::V(_) => Res::num(1.0),
        Res::L(_) => Res::err(XlErr::Value),
    }
}

fn address(ev: &mut Eval, a: &[Expr]) -> Res {
    let r = tri!(num(ev, &a[0])).trunc() as i64;
    let c = tri!(num(ev, &a[1])).trunc() as i64;
    let abs = tri!(opt_num(ev, a, 2, 1.0)).trunc() as i64;
    let a1 = tri!(opt_bool(ev, a, 3, true));
    let sheet = if is_missing(a, 4) { None } else { Some(tri!(text(ev, &a[4]))) };
    if r < 1 || c < 1 || r > MAX_ROWS as i64 || c > MAX_COLS as i64 || !(1..=4).contains(&abs) {
        return Res::err(XlErr::Value);
    }
    let (ra, ca) = match abs {
        1 => (true, true),
        2 => (true, false),
        3 => (false, true),
        _ => (false, false),
    };
    let body = if a1 {
        format!("{}{}{}{}", if ca { "$" } else { "" }, col_name(c as u32 - 1), if ra { "$" } else { "" }, r)
    } else {
        format!("R{}C{}", if ra { r.to_string() } else { format!("[{}]", r) }, if ca { c.to_string() } else { format!("[{}]", c) })
    };
    let s = match sheet {
        Some(sh) if !sh.is_empty() => format!("{}!{}", crate::cellref::quote_sheet(&sh), body),
        _ => body,
    };
    Res::V(Value::string(s))
}

fn filter(ev: &mut Eval, a: &[Expr]) -> Res {
    let arr = tri!(array_of(ev, &a[0]));
    let inc = tri!(array_of(ev, &a[1]));
    let d = ev.d1904();
    let by_rows = inc.cols == 1 && inc.rows == arr.rows;
    let by_cols = inc.rows == 1 && inc.cols == arr.cols && !by_rows;
    if !by_rows && !by_cols {
        return Res::err(XlErr::Value);
    }
    let mut keep = Vec::new();
    for v in inc.data.iter() {
        match v {
            Value::Err(e) => return Res::err(*e),
            v => keep.push(crate::calc::to_num(v, d).map(|x| x != 0.0).unwrap_or(false)),
        }
    }
    let mut data = Vec::new();
    let (rows, cols);
    if by_rows {
        let mut n = 0;
        for r in 0..arr.rows {
            if keep[r] {
                n += 1;
                for c in 0..arr.cols {
                    data.push(arr.get(r, c).clone());
                }
            }
        }
        rows = n;
        cols = arr.cols;
    } else {
        let idx: Vec<usize> = (0..arr.cols).filter(|c| keep[*c]).collect();
        for r in 0..arr.rows {
            for c in &idx {
                data.push(arr.get(r, *c).clone());
            }
        }
        rows = arr.rows;
        cols = idx.len();
    }
    if rows == 0 || cols == 0 {
        if is_missing(a, 2) {
            return Res::err(XlErr::Calc);
        }
        return Res::V(ev.eval_value(&a[2]));
    }
    arr_val(Array::new(rows, cols, data))
}

fn sort_cmp(a: &Value, b: &Value) -> Ordering {
    // blanks always last
    match (a, b) {
        (Value::Empty, Value::Empty) => Ordering::Equal,
        (Value::Empty, _) => Ordering::Greater,
        (_, Value::Empty) => Ordering::Less,
        _ => compare(a, b),
    }
}

fn sort(ev: &mut Eval, a: &[Expr]) -> Res {
    let arr = tri!(array_of(ev, &a[0]));
    let idx = tri!(opt_num(ev, a, 1, 1.0)).trunc() as usize;
    let order = tri!(opt_num(ev, a, 2, 1.0)).trunc();
    let by_col = tri!(opt_bool(ev, a, 3, false));
    if order != 1.0 && order != -1.0 {
        return Res::err(XlErr::Value);
    }
    let n = if by_col { arr.cols } else { arr.rows };
    let width = if by_col { arr.rows } else { arr.cols };
    if idx < 1 || idx > width {
        return Res::err(XlErr::Value);
    }
    let mut perm: Vec<usize> = (0..n).collect();
    let key = |i: usize| if by_col { arr.get(idx - 1, i).clone() } else { arr.get(i, idx - 1).clone() };
    perm.sort_by(|x, y| {
        let o = sort_cmp(&key(*x), &key(*y));
        if order < 0.0 && !matches!(key(*x), Value::Empty) && !matches!(key(*y), Value::Empty) { o.reverse() } else { o }
    });
    permute(&arr, &perm, by_col)
}

fn permute(arr: &Array, perm: &[usize], by_col: bool) -> Res {
    let mut data = Vec::with_capacity(arr.data.len());
    if by_col {
        for r in 0..arr.rows {
            for c in perm {
                data.push(arr.get(r, *c).clone());
            }
        }
        arr_val(Array::new(arr.rows, perm.len(), data))
    } else {
        for r in perm {
            for c in 0..arr.cols {
                data.push(arr.get(*r, c).clone());
            }
        }
        arr_val(Array::new(perm.len(), arr.cols, data))
    }
}

fn sortby(ev: &mut Eval, a: &[Expr]) -> Res {
    let arr = tri!(array_of(ev, &a[0]));
    let mut keys: Vec<(Rc<Array>, f64)> = Vec::new();
    let mut i = 1;
    while i < a.len() {
        let k = tri!(array_of(ev, &a[i]));
        let ord = tri!(opt_num(ev, a, i + 1, 1.0));
        keys.push((k, ord));
        i += 2;
    }
    let by_col = keys[0].0.rows == 1 && arr.rows != keys[0].0.rows.max(1) || (keys[0].0.rows == 1 && keys[0].0.cols == arr.cols && arr.rows > 1);
    let n = if by_col { arr.cols } else { arr.rows };
    for (k, _) in &keys {
        if k.data.len() != n {
            return Res::err(XlErr::Value);
        }
    }
    let mut perm: Vec<usize> = (0..n).collect();
    perm.sort_by(|x, y| {
        for (k, ord) in &keys {
            let o = sort_cmp(&k.data[*x], &k.data[*y]);
            let o = if *ord < 0.0 { o.reverse() } else { o };
            if o != Ordering::Equal {
                return o;
            }
        }
        Ordering::Equal
    });
    permute(&arr, &perm, by_col)
}

fn unique(ev: &mut Eval, a: &[Expr]) -> Res {
    let arr = tri!(array_of(ev, &a[0]));
    let by_col = tri!(opt_bool(ev, a, 1, false));
    let once = tri!(opt_bool(ev, a, 2, false));
    let n = if by_col { arr.cols } else { arr.rows };
    let lines: Vec<Vec<Value>> = (0..n).map(|i| line(&arr, i, !by_col)).collect();
    let same = |x: &Vec<Value>, y: &Vec<Value>| x.iter().zip(y).all(|(p, q)| eq_lookup(p, q, false) && p.type_rank() == q.type_rank());
    let mut out: Vec<usize> = Vec::new();
    for i in 0..n {
        let count = lines.iter().filter(|l| same(l, &lines[i])).count();
        if once && count > 1 {
            continue;
        }
        if out.iter().any(|j| same(&lines[*j], &lines[i])) {
            continue;
        }
        out.push(i);
    }
    if out.is_empty() {
        return Res::err(XlErr::Calc);
    }
    permute(&arr, &out, by_col)
}

fn take_drop(ev: &mut Eval, a: &[Expr], take: bool) -> Res {
    let arr = tri!(array_of(ev, &a[0]));
    let r = if is_missing(a, 1) { None } else { Some(tri!(num(ev, &a[1])).trunc() as i64) };
    let c = if is_missing(a, 2) { None } else { Some(tri!(num(ev, &a[2])).trunc() as i64) };
    let span = |n: Option<i64>, len: usize| -> (usize, usize) {
        let len_i = len as i64;
        match n {
            None => (0, len),
            Some(n) => {
                if take {
                    if n >= 0 { (0, n.min(len_i) as usize) } else { ((len_i + n).max(0) as usize, len) }
                } else if n >= 0 {
                    (n.min(len_i) as usize, len)
                } else {
                    (0, (len_i + n).max(0) as usize)
                }
            }
        }
    };
    let (r0, r1) = span(r, arr.rows);
    let (c0, c1) = span(c, arr.cols);
    if r1 <= r0 || c1 <= c0 {
        return Res::err(XlErr::Calc);
    }
    let mut data = Vec::new();
    for rr in r0..r1 {
        for cc in c0..c1 {
            data.push(arr.get(rr, cc).clone());
        }
    }
    arr_val(Array::new(r1 - r0, c1 - c0, data))
}

fn choose_rc(ev: &mut Eval, a: &[Expr], rows: bool) -> Res {
    let arr = tri!(array_of(ev, &a[0]));
    let n = if rows { arr.rows } else { arr.cols } as i64;
    let mut idx = Vec::new();
    for e in &a[1..] {
        for v in values_of(ev, e) {
            let k = match crate::calc::to_num(&v, ev.d1904()) {
                Ok(k) => k.trunc() as i64,
                Err(e) => return Res::err(e),
            };
            let k = if k < 0 { n + k + 1 } else { k };
            if k < 1 || k > n {
                return Res::err(XlErr::Value);
            }
            idx.push(k as usize - 1);
        }
    }
    permute(&arr, &idx, !rows)
}

fn stack(ev: &mut Eval, a: &[Expr], vertical: bool) -> Res {
    let mut parts = Vec::new();
    for e in a {
        parts.push(tri!(array_of(ev, e)));
    }
    if vertical {
        let cols = parts.iter().map(|p| p.cols).max().unwrap_or(1);
        let rows: usize = parts.iter().map(|p| p.rows).sum();
        let mut data = Vec::with_capacity(rows * cols);
        for p in &parts {
            for r in 0..p.rows {
                for c in 0..cols {
                    data.push(if c < p.cols { p.get(r, c).clone() } else { Value::Err(XlErr::NA) });
                }
            }
        }
        arr_val(Array::new(rows, cols, data))
    } else {
        let rows = parts.iter().map(|p| p.rows).max().unwrap_or(1);
        let cols: usize = parts.iter().map(|p| p.cols).sum();
        let mut data = Vec::with_capacity(rows * cols);
        for r in 0..rows {
            for p in &parts {
                for c in 0..p.cols {
                    data.push(if r < p.rows { p.get(r, c).clone() } else { Value::Err(XlErr::NA) });
                }
            }
        }
        arr_val(Array::new(rows, cols, data))
    }
}

fn tocolrow(ev: &mut Eval, a: &[Expr], col: bool) -> Res {
    let arr = tri!(array_of(ev, &a[0]));
    let ignore = tri!(opt_num(ev, a, 1, 0.0)).trunc() as i64;
    let by_col = tri!(opt_bool(ev, a, 2, false));
    let mut v = Vec::new();
    let mut push = |x: &Value| {
        let skip = match ignore {
            1 => matches!(x, Value::Empty),
            2 => matches!(x, Value::Err(_)),
            3 => matches!(x, Value::Empty | Value::Err(_)),
            _ => false,
        };
        if !skip {
            v.push(x.clone());
        }
    };
    if by_col {
        for c in 0..arr.cols {
            for r in 0..arr.rows {
                push(arr.get(r, c));
            }
        }
    } else {
        for x in arr.data.iter() {
            push(x);
        }
    }
    if v.is_empty() {
        return Res::err(XlErr::Calc);
    }
    let n = v.len();
    if col {
        arr_val(Array::new(n, 1, v))
    } else {
        arr_val(Array::new(1, n, v))
    }
}

fn wrap(ev: &mut Eval, a: &[Expr], rows: bool) -> Res {
    let arr = tri!(array_of(ev, &a[0]));
    if arr.rows != 1 && arr.cols != 1 {
        return Res::err(XlErr::Value);
    }
    let w = tri!(num(ev, &a[1])).trunc() as usize;
    if w < 1 {
        return Res::err(XlErr::Num);
    }
    let pad = if is_missing(a, 2) { Value::Err(XlErr::NA) } else { ev.eval_scalar(&a[2]) };
    let n = arr.data.len();
    let k = n.div_ceil(w);
    let (nr, nc) = if rows { (k, w) } else { (w, k) };
    let mut data = vec![pad; nr * nc];
    for (i, v) in arr.data.iter().enumerate() {
        let (r, c) = if rows { (i / w, i % w) } else { (i % w, i / w) };
        data[r * nc + c] = v.clone();
    }
    arr_val(Array::new(nr, nc, data))
}
