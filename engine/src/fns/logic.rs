//! Logical, information, LET and LAMBDA functions.

use super::*;
use crate::calc::{Eval, Lambda, Res};
use crate::value::{Array, Value, XlErr};
use std::rc::Rc;

pub static DEFS: &[Def] = &[
    d!("TRUE", 0, 0, |_, _| Res::V(Value::Bool(true))),
    d!("FALSE", 0, 0, |_, _| Res::V(Value::Bool(false))),
    d!("IF", 1, 3, iff),
    d!("IFS", 2, MANY, ifs),
    d!("IFERROR", 2, 2, |ev, a| iferr(ev, a, false)),
    d!("IFNA", 2, 2, |ev, a| iferr(ev, a, true)),
    d!("NOT", 1, 1, |ev, a| lift(ev, a, |_, v| match crate::calc::to_bool(&v[0]) { Ok(b) => Value::Bool(!b), Err(e) => Value::Err(e) })),
    d!("AND", 1, MANY, |ev, a| andor(ev, a, 0)),
    d!("OR", 1, MANY, |ev, a| andor(ev, a, 1)),
    d!("XOR", 1, MANY, |ev, a| andor(ev, a, 2)),
    d!("SWITCH", 3, MANY, switch),
    d!("LET", 3, MANY, letf),
    d!("LAMBDA", 1, MANY, lambda),
    d!("ISOMITTED", 1, 1, |ev, a| {
        if let Expr::Name(None, n) = &a[0] {
            if let Some((_, r)) = ev.locals.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case(n)) {
                return Res::V(Value::Bool(matches!(r, Res::V(Value::Empty))));
            }
        }
        Res::V(Value::Bool(false))
    }),
    d!("MAP", 2, MANY, map),
    d!("REDUCE", 3, 3, |ev, a| reduce(ev, a, false)),
    d!("SCAN", 3, 3, |ev, a| reduce(ev, a, true)),
    d!("BYROW", 2, 2, |ev, a| byrc(ev, a, true)),
    d!("BYCOL", 2, 2, |ev, a| byrc(ev, a, false)),
    d!("MAKEARRAY", 3, 3, makearray),
    // ---- information
    d!("ISBLANK", 1, 1, |ev, a| is(ev, a, |v| matches!(v, Value::Empty))),
    d!("ISERR", 1, 1, |ev, a| is(ev, a, |v| matches!(v, Value::Err(e) if *e != XlErr::NA))),
    d!("ISERROR", 1, 1, |ev, a| is(ev, a, |v| matches!(v, Value::Err(_)))),
    d!("ISNA", 1, 1, |ev, a| is(ev, a, |v| matches!(v, Value::Err(XlErr::NA)))),
    d!("ISLOGICAL", 1, 1, |ev, a| is(ev, a, |v| matches!(v, Value::Bool(_)))),
    d!("ISNUMBER", 1, 1, |ev, a| is(ev, a, |v| matches!(v, Value::Num(_)))),
    d!("ISTEXT", 1, 1, |ev, a| is(ev, a, |v| matches!(v, Value::Str(_)))),
    d!("ISNONTEXT", 1, 1, |ev, a| is(ev, a, |v| !matches!(v, Value::Str(_)))),
    d!("ISEVEN", 1, 1, |ev, a| math1(ev, a, |x| Value::Bool(x.abs().trunc() % 2.0 == 0.0))),
    d!("ISODD", 1, 1, |ev, a| math1(ev, a, |x| Value::Bool(x.abs().trunc() % 2.0 == 1.0))),
    d!("ISREF", 1, 1, |ev, a| Res::V(Value::Bool(matches!(ev.eval(&a[0]), Res::R(_))))),
    d!("ISFORMULA", 1, 1, |ev, a| match ev.eval(&a[0]) {
        Res::R(x) => Res::V(Value::Bool(ev.ctx.formula_text(x[0].sheet, x[0].rect.r1, x[0].rect.c1).is_some())),
        Res::V(Value::Err(e)) => Res::err(e),
        _ => Res::err(XlErr::Value),
    }),
    d!("N", 1, 1, |ev, a| { let v = ev.eval_scalar(&a[0]); Res::V(match v { Value::Num(n) => Value::Num(n), Value::Bool(b) => Value::num(if b { 1.0 } else { 0.0 }), Value::Err(e) => Value::Err(e), _ => Value::num(0.0) }) }),
    d!("NA", 0, 0, |_, _| Res::err(XlErr::NA)),
    d!("ERROR.TYPE", 1, 1, |ev, a| { let v = ev.eval_scalar(&a[0]); match v { Value::Err(e) => Res::num(e.code()), _ => Res::err(XlErr::NA) } }),
    d!("TYPE", 1, 1, |ev, a| {
        let r = ev.eval(&a[0]);
        let v = match r { Res::R(areas) => { if areas.len() > 1 || areas[0].rect.count() > 1 { return Res::num(64.0); } ev.res_value(Res::R(areas)) } Res::V(v) => v, Res::L(_) => return Res::num(128.0) };
        Res::num(match v { Value::Num(_) | Value::Empty => 1.0, Value::Str(_) => 2.0, Value::Bool(_) => 4.0, Value::Err(_) => 16.0, Value::Arr(_) => 64.0 })
    }),
    d!("SHEET", 0, 1, |ev, a| {
        if a.is_empty() || matches!(a[0], Expr::Missing) { return Res::num((ev.sheet + 1) as f64); }
        match ev.eval(&a[0]) {
            Res::R(x) => Res::num((x[0].sheet + 1) as f64),
            Res::V(Value::Str(s)) => match ev.ctx.sheet_index(&s) { Some(i) => Res::num((i + 1) as f64), None => Res::err(XlErr::NA) },
            Res::V(Value::Err(e)) => Res::err(e),
            _ => Res::err(XlErr::Value),
        }
    }),
    d!("SHEETS", 0, 1, |ev, a| {
        if a.is_empty() || matches!(a[0], Expr::Missing) { return Res::num(ev.ctx.sheet_count() as f64); }
        match ev.eval(&a[0]) {
            Res::R(x) => { let s: std::collections::BTreeSet<usize> = x.iter().map(|a| a.sheet).collect(); Res::num(s.len() as f64) }
            Res::V(Value::Err(e)) => Res::err(e),
            _ => Res::err(XlErr::Value),
        }
    }),
    d!("CELL", 1, 2, cell),
    d!("INFO", 1, 1, |ev, a| {
        let t = tri!(text(ev, &a[0])).to_lowercase();
        Res::V(match t.as_str() {
            "directory" => Value::string(std::env::var("HOME").unwrap_or_default() + "/"),
            "numfile" => Value::num(ev.ctx.sheet_count() as f64),
            "origin" => Value::str("$A:$A$1"),
            "osversion" => Value::str("Linux"),
            "recalc" => Value::str("Automatic"),
            "release" => Value::str("16.0"),
            "system" => Value::str("pcdos"),
            _ => Value::Err(XlErr::Value),
        })
    }, vol),
];

fn iff(ev: &mut Eval, a: &[Expr]) -> Res {
    let c = ev.eval_value(&a[0]);
    let pick = |ev: &mut Eval, b: bool| -> Res {
        let i = if b { 1 } else { 2 };
        match a.get(i) {
            None => Res::V(Value::Bool(false)),
            Some(Expr::Missing) => Res::num(0.0),
            Some(e) => ev.eval(e),
        }
    };
    match c {
        Value::Arr(arr) => {
            let t = if a.len() > 1 { ev.eval_value(&a[1]) } else { Value::Bool(true) };
            let f = if a.len() > 2 { ev.eval_value(&a[2]) } else { Value::Bool(false) };
            let t = if matches!(a.get(1), Some(Expr::Missing)) { Value::num(0.0) } else { t };
            let f = if matches!(a.get(2), Some(Expr::Missing)) { Value::num(0.0) } else { f };
            let cond = Value::Arr(arr);
            let vals = vec![cond, t, f];
            Res::V(lift_values(ev, &vals, |_, v| match crate::calc::to_bool(&v[0]) {
                Ok(true) => v[1].clone(),
                Ok(false) => v[2].clone(),
                Err(e) => Value::Err(e),
            }))
        }
        v => match crate::calc::to_bool(&v) {
            Ok(b) => pick(ev, b),
            Err(e) => Res::err(e),
        },
    }
}

fn ifs(ev: &mut Eval, a: &[Expr]) -> Res {
    if a.len() % 2 != 0 {
        return Res::err(XlErr::NA);
    }
    for p in a.chunks(2) {
        let c = ev.eval_scalar(&p[0]);
        match crate::calc::to_bool(&c) {
            Ok(true) => return ev.eval(&p[1]),
            Ok(false) => {}
            Err(e) => return Res::err(e),
        }
    }
    Res::err(XlErr::NA)
}

fn iferr(ev: &mut Eval, a: &[Expr], na_only: bool) -> Res {
    let r = ev.eval(&a[0]);
    let v = match r {
        Res::R(_) => ev.res_value(r),
        Res::V(v) => v,
        Res::L(l) => return Res::L(l),
    };
    let bad = |x: &Value| if na_only { matches!(x, Value::Err(XlErr::NA)) } else { matches!(x, Value::Err(_)) };
    match v {
        Value::Arr(arr) => {
            if !arr.data.iter().any(bad) {
                return Res::V(Value::Arr(arr));
            }
            let alt = ev.eval_value(&a[1]);
            let data = arr.data.iter().enumerate().map(|(i, x)| if bad(x) {
                match &alt {
                    Value::Arr(al) => al.get_b(i / arr.cols, i % arr.cols),
                    Value::Empty => Value::num(0.0),
                    y => y.clone(),
                }
            } else { x.clone() }).collect();
            Res::V(Value::Arr(Rc::new(Array::new(arr.rows, arr.cols, data))))
        }
        x if bad(&x) => {
            let r = ev.eval(&a[1]);
            match r {
                Res::V(Value::Empty) => Res::num(0.0),
                other => other,
            }
        }
        x => Res::V(x),
    }
}

/// mode 0 AND, 1 OR, 2 XOR
fn andor(ev: &mut Eval, a: &[Expr], mode: u8) -> Res {
    let mut seen = false;
    let mut acc = match mode {
        0 => true,
        _ => false,
    };
    for e in a {
        let r = ev.eval(e);
        let vals: Vec<Value> = match r {
            Res::R(_) => res_values(ev, r).into_iter().filter(|v| matches!(v, Value::Num(_) | Value::Bool(_) | Value::Err(_))).collect(),
            Res::V(Value::Arr(arr)) => arr.data.iter().filter(|v| matches!(v, Value::Num(_) | Value::Bool(_) | Value::Err(_))).cloned().collect(),
            Res::V(Value::Str(s)) => match crate::calc::to_bool(&Value::Str(s)) {
                Ok(b) => vec![Value::Bool(b)],
                Err(e) => return Res::err(e),
            },
            Res::V(Value::Empty) => vec![],
            Res::V(v) => vec![v],
            Res::L(_) => return Res::err(XlErr::Value),
        };
        for v in vals {
            let b = match v {
                Value::Err(e) => return Res::err(e),
                Value::Bool(b) => b,
                Value::Num(n) => n != 0.0,
                _ => continue,
            };
            seen = true;
            acc = match mode {
                0 => acc && b,
                1 => acc || b,
                _ => acc ^ b,
            };
        }
    }
    if !seen {
        return Res::err(XlErr::Value);
    }
    Res::V(Value::Bool(acc))
}

fn switch(ev: &mut Eval, a: &[Expr]) -> Res {
    let v = ev.eval_scalar(&a[0]);
    if let Value::Err(e) = v {
        return Res::err(e);
    }
    let rest = &a[1..];
    let pairs = rest.len() / 2;
    for i in 0..pairs {
        let c = ev.eval_scalar(&rest[2 * i]);
        if crate::calc::compare(&v, &c) == std::cmp::Ordering::Equal && v.type_rank() == c.type_rank() {
            return ev.eval(&rest[2 * i + 1]);
        }
    }
    if rest.len() % 2 == 1 {
        return ev.eval(&rest[rest.len() - 1]);
    }
    Res::err(XlErr::NA)
}

fn letf(ev: &mut Eval, a: &[Expr]) -> Res {
    if a.len() % 2 == 0 {
        return Res::err(XlErr::Value);
    }
    let n0 = ev.locals.len();
    for p in a[..a.len() - 1].chunks(2) {
        let name = match &p[0] {
            Expr::Name(None, n) => n.clone(),
            _ => {
                ev.locals.truncate(n0);
                return Res::err(XlErr::Value);
            }
        };
        let v = ev.eval(&p[1]);
        ev.locals.push((name, v));
    }
    let r = ev.eval(&a[a.len() - 1]);
    ev.locals.truncate(n0);
    r
}

fn lambda(ev: &mut Eval, a: &[Expr]) -> Res {
    let mut params = Vec::new();
    for p in &a[..a.len() - 1] {
        match p {
            Expr::Name(None, n) => params.push(n.clone()),
            _ => return Res::err(XlErr::Value),
        }
    }
    let captured = ev.locals.clone();
    Res::L(Rc::new(Lambda { params, body: a[a.len() - 1].clone(), captured }))
}

fn get_lambda(ev: &mut Eval, e: &Expr) -> Result<Rc<Lambda>, XlErr> {
    match ev.eval(e) {
        Res::L(l) => Ok(l),
        Res::V(Value::Err(e)) => Err(e),
        _ => Err(XlErr::Value),
    }
}

fn map(ev: &mut Eval, a: &[Expr]) -> Res {
    let f = tri!(get_lambda(ev, &a[a.len() - 1]));
    let mut arrays = Vec::new();
    for e in &a[..a.len() - 1] {
        arrays.push(tri!(array_of(ev, e)));
    }
    let (rows, cols) = (arrays[0].rows, arrays[0].cols);
    let mut data = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let args: Vec<Res> = arrays.iter().map(|x| Res::V(x.get_b(r, c))).collect();
            let v = ev.call_lambda_values(&f, args);
            data.push(ev.res_value(v));
        }
    }
    arr_val(Array::new(rows, cols, data))
}

fn reduce(ev: &mut Eval, a: &[Expr], scan: bool) -> Res {
    let mut acc = if matches!(a[0], Expr::Missing) { Value::Empty } else { ev.eval_value(&a[0]) };
    let arr = tri!(array_of(ev, &a[1]));
    let f = tri!(get_lambda(ev, &a[2]));
    let mut out = Vec::new();
    for v in arr.data.iter() {
        let r = ev.call_lambda_values(&f, vec![Res::V(acc.clone()), Res::V(v.clone())]);
        acc = ev.res_value(r);
        if scan {
            out.push(acc.clone());
        }
    }
    if scan {
        return arr_val(Array::new(arr.rows, arr.cols, out));
    }
    Res::V(acc)
}

fn byrc(ev: &mut Eval, a: &[Expr], rows: bool) -> Res {
    let arr = tri!(array_of(ev, &a[0]));
    let f = tri!(get_lambda(ev, &a[1]));
    let n = if rows { arr.rows } else { arr.cols };
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let line: Vec<Value> = if rows { (0..arr.cols).map(|c| arr.get(i, c).clone()).collect() } else { (0..arr.rows).map(|r| arr.get(r, i).clone()).collect() };
        let len = line.len();
        let sub = if rows { Array::new(1, len, line) } else { Array::new(len, 1, line) };
        let r = ev.call_lambda_values(&f, vec![Res::V(Value::Arr(Rc::new(sub)))]);
        let v = ev.res_value(r);
        out.push(match v {
            Value::Arr(_) => Value::Err(XlErr::Calc),
            x => x,
        });
    }
    if rows {
        arr_val(Array::new(n, 1, out))
    } else {
        arr_val(Array::new(1, n, out))
    }
}

fn makearray(ev: &mut Eval, a: &[Expr]) -> Res {
    let rows = tri!(num(ev, &a[0])).trunc() as i64;
    let cols = tri!(num(ev, &a[1])).trunc() as i64;
    let f = tri!(get_lambda(ev, &a[2]));
    if rows < 1 || cols < 1 || rows * cols > 1_000_000 {
        return Res::err(XlErr::Value);
    }
    let mut data = Vec::new();
    for r in 1..=rows {
        for c in 1..=cols {
            let v = ev.call_lambda_values(&f, vec![Res::num(r as f64), Res::num(c as f64)]);
            data.push(ev.res_value(v));
        }
    }
    arr_val(Array::new(rows as usize, cols as usize, data))
}

fn is(ev: &mut Eval, a: &[Expr], f: fn(&Value) -> bool) -> Res {
    let r = ev.eval(&a[0]);
    let v = match r {
        Res::R(areas) => {
            if ev.legacy || (areas.len() == 1 && areas[0].rect.count() == 1) {
                ev.intersect_value(&areas)
            } else {
                ev.range_value(&areas)
            }
        }
        Res::V(v) => v,
        Res::L(_) => return Res::V(Value::Bool(false)),
    };
    Res::V(match v {
        Value::Arr(arr) => Value::Arr(Rc::new(Array::new(arr.rows, arr.cols, arr.data.iter().map(|x| Value::Bool(f(x))).collect()))),
        x => Value::Bool(f(&x)),
    })
}

fn cell(ev: &mut Eval, a: &[Expr]) -> Res {
    let what = tri!(text(ev, &a[0])).to_lowercase();
    let (sheet, r, c) = if a.len() > 1 && !matches!(a[1], Expr::Missing) {
        match ev.eval(&a[1]) {
            Res::R(x) => (x[0].sheet, x[0].rect.r1, x[0].rect.c1),
            Res::V(Value::Err(e)) => return Res::err(e),
            _ => return Res::err(XlErr::Value),
        }
    } else {
        (ev.sheet, ev.row, ev.col)
    };
    match what.as_str() {
        "row" => Res::num((r + 1) as f64),
        "col" => Res::num((c + 1) as f64),
        "address" => Res::V(Value::string(format!("${}${}", crate::cellref::col_name(c), r + 1))),
        "contents" => Res::V(ev.ctx.value(sheet, r, c)),
        "type" => {
            let v = ev.ctx.value(sheet, r, c);
            Res::V(Value::str(match v { Value::Empty => "b", Value::Str(_) => "l", _ => "v" }))
        }
        "filename" | "format" | "color" | "parentheses" | "prefix" | "protect" | "width" => Res::V(ev.ctx.cell_info(sheet, r, c, &what)),
        _ => Res::err(XlErr::Value),
    }
}
