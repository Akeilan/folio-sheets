//! Database functions: DSUM, DCOUNT, DGET and friends.

use super::stats::{mean, stdev, var};
use super::*;
use crate::calc::{Eval, Res};
use crate::value::{Value, XlErr};

#[derive(Clone, Copy)]
enum Op {
    Sum,
    Count,
    CountA,
    Average,
    Max,
    Min,
    Product,
    Get,
    Stdev(bool),
    Var(bool),
}

pub static DEFS: &[Def] = &[
    d!("DSUM", 3, 3, |ev, a| dfn(ev, a, Op::Sum)),
    d!("DCOUNT", 2, 3, |ev, a| dfn(ev, a, Op::Count)),
    d!("DCOUNTA", 2, 3, |ev, a| dfn(ev, a, Op::CountA)),
    d!("DAVERAGE", 3, 3, |ev, a| dfn(ev, a, Op::Average)),
    d!("DMAX", 3, 3, |ev, a| dfn(ev, a, Op::Max)),
    d!("DMIN", 3, 3, |ev, a| dfn(ev, a, Op::Min)),
    d!("DPRODUCT", 3, 3, |ev, a| dfn(ev, a, Op::Product)),
    d!("DGET", 3, 3, |ev, a| dfn(ev, a, Op::Get)),
    d!("DSTDEV", 3, 3, |ev, a| dfn(ev, a, Op::Stdev(true))),
    d!("DSTDEVP", 3, 3, |ev, a| dfn(ev, a, Op::Stdev(false))),
    d!("DVAR", 3, 3, |ev, a| dfn(ev, a, Op::Var(true))),
    d!("DVARP", 3, 3, |ev, a| dfn(ev, a, Op::Var(false))),
];

pub(crate) fn header_text(v: &Value) -> String {
    match v {
        Value::Str(s) => s.trim().to_lowercase(),
        Value::Num(n) => crate::numfmt::num_to_text(*n),
        Value::Bool(b) => if *b { "true".into() } else { "false".into() },
        _ => String::new(),
    }
}

/// Text criteria in a criteria range match the start of the entry ("Smith" finds
/// "Smithers"), unlike COUNTIF.
pub(crate) fn db_criteria(v: &Value, d1904: bool) -> Option<Criteria> {
    match v {
        Value::Empty => None,
        Value::Str(s) => {
            let t: &str = s;
            let has_op = t.starts_with(['<', '>', '=']);
            if !has_op && crate::calc::text_to_number(t, d1904).is_none() && !t.is_empty() {
                let up = t.to_ascii_uppercase();
                if up != "TRUE" && up != "FALSE" && XlErr::parse(t).is_none() {
                    return Some(Criteria::new(&Value::string(format!("{}*", t)), d1904));
                }
            }
            Some(Criteria::new(v, d1904))
        }
        _ => Some(Criteria::new(v, d1904)),
    }
}

fn dfn(ev: &mut Eval, a: &[Expr], op: Op) -> Res {
    let db = tri!(array_of(ev, &a[0]));
    if db.rows < 1 {
        return Res::err(XlErr::Value);
    }
    let heads: Vec<String> = (0..db.cols).map(|c| header_text(db.get(0, c))).collect();
    let (field, crit_expr) = if a.len() == 2 { (None, &a[1]) } else { (Some(&a[1]), &a[2]) };
    let col: Option<usize> = match field {
        None | Some(Expr::Missing) => None,
        Some(f) => {
            let v = ev.eval_scalar(f);
            match v {
                Value::Num(n) => {
                    let i = n.trunc() as i64;
                    if i < 1 || i as usize > db.cols {
                        return Res::err(XlErr::Value);
                    }
                    Some(i as usize - 1)
                }
                Value::Str(s) => match heads.iter().position(|h| *h == s.trim().to_lowercase()) {
                    Some(i) => Some(i),
                    None => return Res::err(XlErr::Value),
                },
                Value::Err(e) => return Res::err(e),
                _ => return Res::err(XlErr::Value),
            }
        }
    };
    let cr = tri!(array_of(ev, crit_expr));
    if cr.rows < 1 {
        return Res::err(XlErr::Value);
    }
    let d1904 = ev.d1904();
    // each criteria row: (database column, condition)
    let mut rules: Vec<Vec<(usize, Criteria)>> = Vec::new();
    for r in 1..cr.rows {
        let mut row = Vec::new();
        for c in 0..cr.cols {
            let h = header_text(cr.get(0, c));
            let Some(dc) = heads.iter().position(|x| *x == h) else { continue };
            if let Some(k) = db_criteria(cr.get(r, c), d1904) {
                row.push((dc, k));
            }
        }
        rules.push(row);
    }
    let matches = |r: usize| -> bool {
        if rules.is_empty() {
            return true;
        }
        rules.iter().any(|row| row.iter().all(|(c, k)| k.matches(db.get(r, *c))))
    };
    let mut vals: Vec<Value> = Vec::new();
    let mut nrec = 0usize;
    for r in 1..db.rows {
        if matches(r) {
            nrec += 1;
            if let Some(c) = col {
                vals.push(db.get(r, c).clone());
            }
        }
    }
    let nums: Vec<f64> = vals.iter().filter_map(|v| if let Value::Num(n) = v { Some(*n) } else { None }).collect();
    let r = match op {
        Op::Count => Ok(if col.is_none() { nrec as f64 } else { nums.len() as f64 }),
        Op::CountA => Ok(if col.is_none() { nrec as f64 } else { vals.iter().filter(|v| !matches!(v, Value::Empty)).count() as f64 }),
        Op::Sum => Ok(nums.iter().sum()),
        Op::Product => Ok(nums.iter().product()),
        Op::Average => mean(&nums),
        Op::Max => Ok(nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max)).map(|x| if x.is_finite() { x } else { 0.0 }),
        Op::Min => Ok(nums.iter().cloned().fold(f64::INFINITY, f64::min)).map(|x| if x.is_finite() { x } else { 0.0 }),
        Op::Stdev(s) => stdev(&nums, s),
        Op::Var(s) => var(&nums, s),
        Op::Get => {
            return match vals.len() {
                0 => Res::err(XlErr::Value),
                1 => Res::V(vals.pop().unwrap()),
                _ => Res::err(XlErr::Num),
            }
        }
    };
    match r {
        Ok(x) => Res::num(x),
        Err(e) => Res::err(e),
    }
}
