//! Statistics (the distributions live in dist.rs).

use super::math::ksum;
use super::*;
use crate::calc::{Eval, Res};
use crate::value::{Array, Value, XlErr};

pub static DEFS: &[Def] = &[
    d!("AVERAGE", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); mean(&v).map(Res::num).unwrap_or_else(Res::err) }),
    d!("AVERAGEA", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::A)); mean(&v).map(Res::num).unwrap_or_else(Res::err) }),
    d!("AVERAGEIF", 2, 3, averageif),
    d!("AVERAGEIFS", 3, MANY, |ev, a| ifs_agg(ev, a, |v| mean(v))),
    d!("MAXIFS", 3, MANY, |ev, a| ifs_agg(ev, a, |v| Ok(v.iter().copied().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x)))).unwrap_or(0.0)))),
    d!("MINIFS", 3, MANY, |ev, a| ifs_agg(ev, a, |v| Ok(v.iter().copied().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x)))).unwrap_or(0.0)))),
    d!("COUNT", 1, MANY, count),
    d!("COUNTA", 1, MANY, counta),
    d!("COUNTBLANK", 1, 1, countblank),
    d!("COUNTIF", 2, 2, countif),
    d!("COUNTIFS", 2, MANY, |ev, a| { let (m, _, _) = tri!(ifs_mask(ev, a)); Res::num(m.iter().filter(|x| **x).count() as f64) }),
    d!("MAX", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); Res::num(v.iter().copied().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x)))).unwrap_or(0.0)) }),
    d!("MAXA", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::A)); Res::num(v.iter().copied().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x)))).unwrap_or(0.0)) }),
    d!("MIN", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); Res::num(v.iter().copied().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x)))).unwrap_or(0.0)) }),
    d!("MINA", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::A)); Res::num(v.iter().copied().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x)))).unwrap_or(0.0)) }),
    d!("MEDIAN", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); median(&v).map(Res::num).unwrap_or_else(Res::err) }),
    d!("MODE", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); mode_sngl(&v).map(Res::num).unwrap_or_else(Res::err) }),
    d!("MODE.SNGL", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); mode_sngl(&v).map(Res::num).unwrap_or_else(Res::err) }),
    d!("MODE.MULT", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        let m = mode_mult(&v);
        if m.is_empty() { return Res::err(XlErr::NA); }
        let n = m.len();
        arr_val(Array::new(n, 1, m.into_iter().map(Value::num).collect()))
    }),
    d!("STDEV", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); stdev(&v, true).map(Res::num).unwrap_or_else(Res::err) }),
    d!("STDEV.S", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); stdev(&v, true).map(Res::num).unwrap_or_else(Res::err) }),
    d!("STDEVA", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::A)); stdev(&v, true).map(Res::num).unwrap_or_else(Res::err) }),
    d!("STDEVP", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); stdev(&v, false).map(Res::num).unwrap_or_else(Res::err) }),
    d!("STDEV.P", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); stdev(&v, false).map(Res::num).unwrap_or_else(Res::err) }),
    d!("STDEVPA", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::A)); stdev(&v, false).map(Res::num).unwrap_or_else(Res::err) }),
    d!("VAR", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); var(&v, true).map(Res::num).unwrap_or_else(Res::err) }),
    d!("VAR.S", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); var(&v, true).map(Res::num).unwrap_or_else(Res::err) }),
    d!("VARA", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::A)); var(&v, true).map(Res::num).unwrap_or_else(Res::err) }),
    d!("VARP", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); var(&v, false).map(Res::num).unwrap_or_else(Res::err) }),
    d!("VAR.P", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); var(&v, false).map(Res::num).unwrap_or_else(Res::err) }),
    d!("VARPA", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::A)); var(&v, false).map(Res::num).unwrap_or_else(Res::err) }),
    d!("AVEDEV", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        let m = tri!(mean(&v));
        Res::num(v.iter().map(|x| (x - m).abs()).sum::<f64>() / v.len() as f64)
    }),
    d!("DEVSQ", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        if v.is_empty() { return Res::err(XlErr::Num); }
        let m = ksum(&v) / v.len() as f64;
        Res::num(v.iter().map(|x| (x - m) * (x - m)).sum())
    }),
    d!("GEOMEAN", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        if v.is_empty() || v.iter().any(|x| *x <= 0.0) { return Res::err(XlErr::Num); }
        Res::num((v.iter().map(|x| x.ln()).sum::<f64>() / v.len() as f64).exp())
    }),
    d!("HARMEAN", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        if v.is_empty() || v.iter().any(|x| *x <= 0.0) { return Res::err(XlErr::Num); }
        Res::num(v.len() as f64 / v.iter().map(|x| 1.0 / x).sum::<f64>())
    }),
    d!("KURT", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        let n = v.len() as f64;
        if n < 4.0 { return Res::err(XlErr::Div0); }
        let m = ksum(&v) / n;
        let s = tri!(stdev(&v, true));
        if s == 0.0 { return Res::err(XlErr::Div0); }
        let s4: f64 = v.iter().map(|x| ((x - m) / s).powi(4)).sum();
        Res::num(n * (n + 1.0) / ((n - 1.0) * (n - 2.0) * (n - 3.0)) * s4 - 3.0 * (n - 1.0).powi(2) / ((n - 2.0) * (n - 3.0)))
    }),
    d!("SKEW", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        let n = v.len() as f64;
        if n < 3.0 { return Res::err(XlErr::Div0); }
        let m = ksum(&v) / n;
        let s = tri!(stdev(&v, true));
        if s == 0.0 { return Res::err(XlErr::Div0); }
        Res::num(n / ((n - 1.0) * (n - 2.0)) * v.iter().map(|x| ((x - m) / s).powi(3)).sum::<f64>())
    }),
    d!("SKEW.P", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        let n = v.len() as f64;
        if n < 1.0 { return Res::err(XlErr::Div0); }
        let m = ksum(&v) / n;
        let s = tri!(stdev(&v, false));
        if s == 0.0 { return Res::err(XlErr::Div0); }
        Res::num(v.iter().map(|x| ((x - m) / s).powi(3)).sum::<f64>() / n)
    }),
    d!("LARGE", 2, 2, |ev, a| large_small(ev, a, true)),
    d!("SMALL", 2, 2, |ev, a| large_small(ev, a, false)),
    d!("PERCENTILE", 2, 2, |ev, a| pct(ev, a, true)),
    d!("PERCENTILE.INC", 2, 2, |ev, a| pct(ev, a, true)),
    d!("PERCENTILE.EXC", 2, 2, |ev, a| pct(ev, a, false)),
    d!("QUARTILE", 2, 2, |ev, a| quart(ev, a, true)),
    d!("QUARTILE.INC", 2, 2, |ev, a| quart(ev, a, true)),
    d!("QUARTILE.EXC", 2, 2, |ev, a| quart(ev, a, false)),
    d!("PERCENTRANK", 2, 3, |ev, a| prank(ev, a, true)),
    d!("PERCENTRANK.INC", 2, 3, |ev, a| prank(ev, a, true)),
    d!("PERCENTRANK.EXC", 2, 3, |ev, a| prank(ev, a, false)),
    d!("RANK", 2, 3, |ev, a| rank(ev, a, false)),
    d!("RANK.EQ", 2, 3, |ev, a| rank(ev, a, false)),
    d!("RANK.AVG", 2, 3, |ev, a| rank(ev, a, true)),
    d!("TRIMMEAN", 2, 2, |ev, a| {
        let mut v = tri!(collect(ev, &a[..1], Mode::Plain));
        let p = tri!(num(ev, &a[1]));
        if !(0.0..1.0).contains(&p) || v.is_empty() { return Res::err(XlErr::Num); }
        v.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let k = ((v.len() as f64 * p) / 2.0).floor() as usize;
        let t = &v[k..v.len() - k];
        Res::num(ksum(t) / t.len() as f64)
    }),
    d!("STANDARDIZE", 3, 3, |ev, a| {
        let x = tri!(num(ev, &a[0])); let m = tri!(num(ev, &a[1])); let s = tri!(num(ev, &a[2]));
        if s <= 0.0 { return Res::err(XlErr::Num); }
        Res::num((x - m) / s)
    }),
    d!("FISHER", 1, 1, |ev, a| math1(ev, a, |x| if x.abs() >= 1.0 { Value::Err(XlErr::Num) } else { Value::num(0.5 * ((1.0 + x) / (1.0 - x)).ln()) })),
    d!("FISHERINV", 1, 1, |ev, a| math1(ev, a, |y| Value::num(((2.0 * y).exp() - 1.0) / ((2.0 * y).exp() + 1.0)))),
    d!("PERMUT", 2, 2, |ev, a| math2(ev, a, |n, k| {
        let (n, k) = (n.trunc(), k.trunc());
        if n < 0.0 || k < 0.0 || k > n { return Value::Err(XlErr::Num); }
        let mut p = 1.0; let mut i = 0.0;
        while i < k { p *= n - i; i += 1.0; }
        Value::num(p)
    })),
    d!("PERMUTATIONA", 2, 2, |ev, a| math2(ev, a, |n, k| {
        let (n, k) = (n.trunc(), k.trunc());
        if n < 0.0 || k < 0.0 { return Value::Err(XlErr::Num); }
        Value::num(n.powf(k))
    })),
    d!("CORREL", 2, 2, |ev, a| two(ev, a, correl)),
    d!("PEARSON", 2, 2, |ev, a| two(ev, a, correl)),
    d!("RSQ", 2, 2, |ev, a| two(ev, a, |x, y| correl(x, y).map(|r| r * r))),
    d!("COVAR", 2, 2, |ev, a| two(ev, a, |x, y| covar(x, y, false))),
    d!("COVARIANCE.P", 2, 2, |ev, a| two(ev, a, |x, y| covar(x, y, false))),
    d!("COVARIANCE.S", 2, 2, |ev, a| two(ev, a, |x, y| covar(x, y, true))),
    d!("SLOPE", 2, 2, |ev, a| two(ev, a, |y, x| linfit(x, y).map(|f| f.0))),
    d!("INTERCEPT", 2, 2, |ev, a| two(ev, a, |y, x| linfit(x, y).map(|f| f.1))),
    d!("STEYX", 2, 2, |ev, a| two(ev, a, |y, x| {
        let n = x.len() as f64;
        if n < 3.0 { return Err(XlErr::Div0); }
        let (b, c) = linfit(x, y)?;
        let ss: f64 = x.iter().zip(y).map(|(xi, yi)| (yi - (b * xi + c)).powi(2)).sum();
        Ok((ss / (n - 2.0)).sqrt())
    })),
    d!("FORECAST", 3, 3, forecast),
    d!("FORECAST.LINEAR", 3, 3, forecast),
    d!("FREQUENCY", 2, 2, |ev, a| {
        let data = tri!(collect(ev, &a[..1], Mode::Plain));
        let bins = tri!(collect(ev, &a[1..2], Mode::Plain));
        let mut sorted: Vec<(usize, f64)> = bins.iter().copied().enumerate().collect();
        sorted.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
        let mut counts = vec![0.0; bins.len() + 1];
        for d in data {
            match sorted.iter().find(|(_, b)| d <= *b) {
                Some((i, _)) => counts[*i] += 1.0,
                None => counts[bins.len()] += 1.0,
            }
        }
        let n = counts.len();
        arr_val(Array::new(n, 1, counts.into_iter().map(Value::num).collect()))
    }),
    d!("PROB", 3, 4, |ev, a| {
        let x = tri!(collect(ev, &a[..1], Mode::Plain));
        let p = tri!(collect(ev, &a[1..2], Mode::Plain));
        if x.len() != p.len() { return Res::err(XlErr::NA); }
        let s: f64 = p.iter().sum();
        if p.iter().any(|q| *q < 0.0 || *q > 1.0) || (s - 1.0).abs() > 1e-9 { return Res::err(XlErr::Num); }
        let lo = tri!(num(ev, &a[2]));
        let hi = if a.len() > 3 { tri!(num(ev, &a[3])) } else { lo };
        Res::num(x.iter().zip(&p).filter(|(xi, _)| **xi >= lo && **xi <= hi).map(|(_, pi)| pi).sum())
    }),
    d!("LINEST", 1, 4, |ev, a| linest(ev, a, false)),
    d!("LOGEST", 1, 4, |ev, a| linest(ev, a, true)),
    d!("TREND", 1, 4, |ev, a| trend(ev, a, false)),
    d!("GROWTH", 1, 4, |ev, a| trend(ev, a, true)),
];

pub fn mean(v: &[f64]) -> Result<f64, XlErr> {
    if v.is_empty() {
        return Err(XlErr::Div0);
    }
    Ok(ksum(v) / v.len() as f64)
}

pub fn var(v: &[f64], sample: bool) -> Result<f64, XlErr> {
    let n = v.len() as f64;
    if n < 1.0 || (sample && n < 2.0) {
        return Err(XlErr::Div0);
    }
    let m = ksum(v) / n;
    let ss: f64 = v.iter().map(|x| (x - m) * (x - m)).sum();
    Ok(ss / if sample { n - 1.0 } else { n })
}

pub fn stdev(v: &[f64], sample: bool) -> Result<f64, XlErr> {
    var(v, sample).map(|x| x.sqrt())
}

pub fn median(v: &[f64]) -> Result<f64, XlErr> {
    if v.is_empty() {
        return Err(XlErr::Num);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = s.len();
    Ok(if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) / 2.0 })
}

pub fn mode_sngl(v: &[f64]) -> Result<f64, XlErr> {
    mode_mult(v).first().copied().ok_or(XlErr::NA)
}

/// Most frequent values, in the order they first appear.
pub fn mode_mult(v: &[f64]) -> Vec<f64> {
    let mut counts: Vec<(f64, usize, usize)> = Vec::new();
    for (i, x) in v.iter().enumerate() {
        if let Some(c) = counts.iter_mut().find(|c| c.0 == *x) {
            c.1 += 1;
        } else {
            counts.push((*x, 1, i));
        }
    }
    let best = counts.iter().map(|c| c.1).max().unwrap_or(0);
    if best < 2 {
        return vec![];
    }
    counts.iter().filter(|c| c.1 == best).map(|c| c.0).collect()
}

pub fn percentile(v: &[f64], k: f64, inclusive: bool) -> Result<f64, XlErr> {
    if v.is_empty() {
        return Err(XlErr::Num);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = s.len() as f64;
    let rank = if inclusive {
        if !(0.0..=1.0).contains(&k) {
            return Err(XlErr::Num);
        }
        k * (n - 1.0)
    } else {
        if k <= 0.0 || k >= 1.0 {
            return Err(XlErr::Num);
        }
        let r = k * (n + 1.0) - 1.0;
        if r < 0.0 || r > n - 1.0 {
            return Err(XlErr::Num);
        }
        r
    };
    let lo = rank.floor() as usize;
    let f = rank - lo as f64;
    if lo + 1 >= s.len() {
        return Ok(s[s.len() - 1]);
    }
    Ok(s[lo] + f * (s[lo + 1] - s[lo]))
}

pub fn quartile(v: &[f64], q: f64, inclusive: bool) -> Result<f64, XlErr> {
    let q = q.trunc();
    if inclusive {
        if !(0.0..=4.0).contains(&q) {
            return Err(XlErr::Num);
        }
    } else if !(1.0..=3.0).contains(&q) {
        return Err(XlErr::Num);
    }
    percentile(v, q / 4.0, inclusive)
}

fn averageif(ev: &mut Eval, a: &[Expr]) -> Res {
    let r = ev.eval(&a[0]);
    let crit_arr = tri!(res_array(ev, r));
    let crit = ev.eval_scalar(&a[1]);
    let c = Criteria::new(&crit, ev.d1904());
    let avg_arr = if a.len() > 2 && !matches!(a[2], Expr::Missing) {
        tri!(super::math::resized(ev, &a[2], crit_arr.rows, crit_arr.cols, None))
    } else {
        crit_arr.clone()
    };
    let mut s = Vec::new();
    for (i, v) in crit_arr.data.iter().enumerate() {
        if c.matches(v) {
            match avg_arr.data.get(i) {
                Some(Value::Num(n)) => s.push(*n),
                Some(Value::Err(e)) => return Res::err(*e),
                _ => {}
            }
        }
    }
    mean(&s).map(Res::num).unwrap_or_else(Res::err)
}

fn ifs_agg(ev: &mut Eval, a: &[Expr], f: fn(&[f64]) -> Result<f64, XlErr>) -> Res {
    let (mask, rows, cols) = tri!(ifs_mask(ev, &a[1..]));
    let arr = tri!(super::math::resized(ev, &a[0], rows, cols, None));
    let mut s = Vec::new();
    for (i, m) in mask.iter().enumerate() {
        if *m {
            match arr.data.get(i) {
                Some(Value::Num(n)) => s.push(*n),
                Some(Value::Err(e)) => return Res::err(*e),
                _ => {}
            }
        }
    }
    f(&s).map(Res::num).unwrap_or_else(Res::err)
}

fn count(ev: &mut Eval, a: &[Expr]) -> Res {
    let mut n = 0usize;
    for e in a {
        let r = ev.eval(e);
        match r {
            Res::R(areas) => {
                for ar in &areas {
                    let rc = ev.bound(ar);
                    n += ev.ctx.cells_in(ar.sheet, rc).iter().filter(|x| matches!(x.2, Value::Num(_))).count();
                }
            }
            Res::V(Value::Arr(arr)) => n += arr.data.iter().filter(|x| matches!(x, Value::Num(_))).count(),
            Res::V(Value::Num(_)) | Res::V(Value::Bool(_)) => n += 1,
            Res::V(Value::Str(s)) => {
                if crate::calc::text_to_number(&s, ev.d1904()).is_some() {
                    n += 1
                }
            }
            _ => {}
        }
    }
    Res::num(n as f64)
}

fn counta(ev: &mut Eval, a: &[Expr]) -> Res {
    let mut n = 0usize;
    for e in a {
        if matches!(e, Expr::Missing) {
            n += 1;
            continue;
        }
        let r = ev.eval(e);
        match r {
            Res::R(areas) => {
                for ar in &areas {
                    let rc = ev.bound(ar);
                    n += ev.ctx.cells_in(ar.sheet, rc).iter().filter(|x| !matches!(x.2, Value::Empty)).count();
                }
            }
            Res::V(Value::Arr(arr)) => n += arr.data.iter().filter(|x| !matches!(x, Value::Empty)).count(),
            Res::V(Value::Empty) => {}
            _ => n += 1,
        }
    }
    Res::num(n as f64)
}

fn countblank(ev: &mut Eval, a: &[Expr]) -> Res {
    let r = ev.eval(&a[0]);
    match r {
        Res::R(areas) => {
            let mut n = 0u64;
            for ar in &areas {
                let total = ar.rect.count();
                let rc = ar.rect;
                let filled = ev.ctx.cells_in(ar.sheet, rc).iter().filter(|x| !matches!(&x.2, Value::Empty) && !matches!(&x.2, Value::Str(s) if s.is_empty())).count() as u64;
                n += total - filled;
            }
            Res::num(n as f64)
        }
        Res::V(Value::Arr(arr)) => Res::num(arr.data.iter().filter(|x| matches!(x, Value::Empty) || matches!(x, Value::Str(s) if s.is_empty())).count() as f64),
        _ => Res::err(XlErr::Value),
    }
}

fn countif(ev: &mut Eval, a: &[Expr]) -> Res {
    let r = ev.eval(&a[0]);
    let crit = ev.eval_scalar(&a[1]);
    let c = Criteria::new(&crit, ev.d1904());
    match r {
        Res::R(areas) => {
            let mut n = 0u64;
            for ar in &areas {
                // blank cells only matter when the criteria can match a blank
                let blank_matches = c.matches(&Value::Empty);
                let rc = if blank_matches { ar.rect } else { ev.bound(ar) };
                let cells = ev.ctx.cells_in(ar.sheet, rc);
                let hits = cells.iter().filter(|x| c.matches(&x.2)).count() as u64;
                n += hits;
                if blank_matches {
                    let filled = cells.iter().filter(|x| !matches!(x.2, Value::Empty)).count() as u64;
                    n += rc.count() - filled;
                }
            }
            Res::num(n as f64)
        }
        Res::V(Value::Arr(arr)) => Res::num(arr.data.iter().filter(|x| c.matches(x)).count() as f64),
        Res::V(Value::Err(e)) => Res::err(e),
        Res::V(v) => Res::num(if c.matches(&v) { 1.0 } else { 0.0 }),
        _ => Res::err(XlErr::Value),
    }
}

fn large_small(ev: &mut Eval, a: &[Expr], large: bool) -> Res {
    let mut v = tri!(collect(ev, &a[..1], Mode::Plain));
    v.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let kv = ev.eval_value(&a[1]);
    let n = v.len();
    let pick = |k: f64| -> Value {
        let k = k.ceil();
        if k < 1.0 || k as usize > n {
            return Value::Err(XlErr::Num);
        }
        let k = k as usize;
        Value::num(if large { v[n - k] } else { v[k - 1] })
    };
    let d = ev.d1904();
    Res::V(match kv {
        Value::Arr(arr) => Value::Arr(std::rc::Rc::new(Array::new(arr.rows, arr.cols, arr.data.iter().map(|x| match crate::calc::to_num(x, d) {
            Ok(k) => pick(k),
            Err(e) => Value::Err(e),
        }).collect()))),
        x => match crate::calc::to_num(&x, d) {
            Ok(k) => pick(k),
            Err(e) => Value::Err(e),
        },
    })
}

fn pct(ev: &mut Eval, a: &[Expr], inc: bool) -> Res {
    let v = tri!(collect(ev, &a[..1], Mode::Plain));
    let k = tri!(num(ev, &a[1]));
    percentile(&v, k, inc).map(Res::num).unwrap_or_else(Res::err)
}

fn quart(ev: &mut Eval, a: &[Expr], inc: bool) -> Res {
    let v = tri!(collect(ev, &a[..1], Mode::Plain));
    let k = tri!(num(ev, &a[1]));
    quartile(&v, k, inc).map(Res::num).unwrap_or_else(Res::err)
}

fn prank(ev: &mut Eval, a: &[Expr], inc: bool) -> Res {
    let mut v = tri!(collect(ev, &a[..1], Mode::Plain));
    let x = tri!(num(ev, &a[1]));
    let sig = tri!(opt_num(ev, a, 2, 3.0)).trunc();
    if v.is_empty() || sig < 1.0 {
        return Res::err(XlErr::Num);
    }
    v.sort_by(|p, q| p.partial_cmp(q).unwrap());
    let n = v.len();
    if x < v[0] || x > v[n - 1] {
        return Res::err(XlErr::NA);
    }
    let below = v.iter().filter(|y| **y < x).count() as f64;
    let rank = match v.iter().position(|y| *y == x) {
        Some(_) => {
            if inc {
                below / (n as f64 - 1.0).max(1.0)
            } else {
                (below + 1.0) / (n as f64 + 1.0)
            }
        }
        None => {
            // interpolate between the neighbours
            let i = v.iter().position(|y| *y > x).unwrap();
            let (lo, hi) = (v[i - 1], v[i]);
            let f = (x - lo) / (hi - lo);
            if inc {
                ((i - 1) as f64 + f) / (n as f64 - 1.0)
            } else {
                (i as f64 + f) / (n as f64 + 1.0)
            }
        }
    };
    let p = 10f64.powf(sig);
    Res::num((rank * p).floor() / p)
}

fn rank(ev: &mut Eval, a: &[Expr], avg: bool) -> Res {
    let x = tri!(num(ev, &a[0]));
    let v = tri!(collect(ev, &a[1..2], Mode::Plain));
    let asc = tri!(opt_num(ev, a, 2, 0.0)) != 0.0;
    if !v.iter().any(|y| crate::calc::num_eq(*y, x)) {
        return Res::err(XlErr::NA);
    }
    let better = v.iter().filter(|y| if asc { **y < x } else { **y > x }).filter(|y| !crate::calc::num_eq(**y, x)).count() as f64;
    let ties = v.iter().filter(|y| crate::calc::num_eq(**y, x)).count() as f64;
    Res::num(if avg { better + (ties + 1.0) / 2.0 } else { better + 1.0 })
}

/// Paired numbers from two arguments (pairs with a non-number on either side are dropped).
pub fn paired(ev: &mut Eval, a: &Expr, b: &Expr) -> Result<(Vec<f64>, Vec<f64>), XlErr> {
    let x = array_of(ev, a)?;
    let y = array_of(ev, b)?;
    if x.data.len() != y.data.len() {
        return Err(XlErr::NA);
    }
    let mut xs = Vec::new();
    let mut ys = Vec::new();
    for (p, q) in x.data.iter().zip(y.data.iter()) {
        if let Value::Err(e) = p {
            return Err(*e);
        }
        if let Value::Err(e) = q {
            return Err(*e);
        }
        if let (Value::Num(p), Value::Num(q)) = (p, q) {
            xs.push(*p);
            ys.push(*q);
        }
    }
    Ok((xs, ys))
}

fn two(ev: &mut Eval, a: &[Expr], f: fn(&[f64], &[f64]) -> Result<f64, XlErr>) -> Res {
    let (x, y) = tri!(paired(ev, &a[0], &a[1]));
    f(&x, &y).map(Res::num).unwrap_or_else(Res::err)
}

pub fn correl(x: &[f64], y: &[f64]) -> Result<f64, XlErr> {
    let n = x.len() as f64;
    if n < 2.0 {
        return Err(XlErr::Div0);
    }
    let mx = ksum(x) / n;
    let my = ksum(y) / n;
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for i in 0..x.len() {
        sxy += (x[i] - mx) * (y[i] - my);
        sxx += (x[i] - mx).powi(2);
        syy += (y[i] - my).powi(2);
    }
    if sxx == 0.0 || syy == 0.0 {
        return Err(XlErr::Div0);
    }
    Ok(sxy / (sxx * syy).sqrt())
}

pub fn covar(x: &[f64], y: &[f64], sample: bool) -> Result<f64, XlErr> {
    let n = x.len() as f64;
    if n < 1.0 || (sample && n < 2.0) {
        return Err(XlErr::Div0);
    }
    let mx = ksum(x) / n;
    let my = ksum(y) / n;
    let s: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    Ok(s / if sample { n - 1.0 } else { n })
}

/// Least squares y = b*x + c: (b, c)
pub fn linfit(x: &[f64], y: &[f64]) -> Result<(f64, f64), XlErr> {
    let n = x.len() as f64;
    if n < 1.0 {
        return Err(XlErr::Div0);
    }
    let mx = ksum(x) / n;
    let my = ksum(y) / n;
    let sxx: f64 = x.iter().map(|a| (a - mx).powi(2)).sum();
    if sxx == 0.0 {
        return Err(XlErr::Div0);
    }
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let b = sxy / sxx;
    Ok((b, my - b * mx))
}

fn forecast(ev: &mut Eval, a: &[Expr]) -> Res {
    let x = tri!(num(ev, &a[0]));
    let (ys, xs) = tri!(paired(ev, &a[1], &a[2]));
    let (b, c) = tri!(linfit(&xs, &ys));
    Res::num(b * x + c)
}

/// Multiple regression by normal equations. Returns coefficients [m_k..m_1, b].
fn regress(ys: &[f64], xs: &[Vec<f64>], with_const: bool) -> Option<Vec<f64>> {
    let n = ys.len();
    let k = xs.len();
    let p = k + if with_const { 1 } else { 0 };
    let row = |i: usize| -> Vec<f64> {
        let mut r: Vec<f64> = (0..k).map(|j| xs[j][i]).collect();
        if with_const {
            r.push(1.0);
        }
        r
    };
    let mut ata = vec![vec![0.0; p]; p];
    let mut aty = vec![0.0; p];
    for i in 0..n {
        let r = row(i);
        for a in 0..p {
            aty[a] += r[a] * ys[i];
            for b in 0..p {
                ata[a][b] += r[a] * r[b];
            }
        }
    }
    let inv = super::math::inverse(ata)?;
    let coef: Vec<f64> = (0..p).map(|a| (0..p).map(|b| inv[a][b] * aty[b]).sum()).collect();
    Some(coef)
}

fn xs_of(ev: &mut Eval, a: &[Expr], n: usize, idx: usize) -> Result<Vec<Vec<f64>>, XlErr> {
    if a.len() > idx && !matches!(a[idx], Expr::Missing) {
        let arr = array_of(ev, &a[idx])?;
        let nums: Vec<f64> = arr.data.iter().map(|v| if let Value::Num(x) = v { Ok(*x) } else { Err(XlErr::Value) }).collect::<Result<_, _>>()?;
        if arr.rows * arr.cols == n {
            return Ok(vec![nums]);
        }
        // several x columns (or rows)
        if arr.rows == n {
            return Ok((0..arr.cols).map(|c| (0..n).map(|r| nums[r * arr.cols + c]).collect()).collect());
        }
        if arr.cols == n {
            return Ok((0..arr.rows).map(|r| nums[r * arr.cols..(r + 1) * arr.cols].to_vec()).collect());
        }
        return Err(XlErr::Ref);
    }
    Ok(vec![(1..=n).map(|i| i as f64).collect()])
}

fn linest(ev: &mut Eval, a: &[Expr], log: bool) -> Res {
    let ys_arr = tri!(array_of(ev, &a[0]));
    let mut ys: Vec<f64> = Vec::new();
    for v in ys_arr.data.iter() {
        match v {
            Value::Num(x) => ys.push(if log { if *x <= 0.0 { return Res::err(XlErr::Num) } else { x.ln() } } else { *x }),
            _ => return Res::err(XlErr::Value),
        }
    }
    let xs = tri!(xs_of(ev, a, ys.len(), 1));
    let cst = tri!(opt_bool(ev, a, 2, true));
    let stats = tri!(opt_bool(ev, a, 3, false));
    let coef = match regress(&ys, &xs, cst) {
        Some(c) => c,
        None => return Res::err(XlErr::Num),
    };
    let k = xs.len();
    let mut first: Vec<f64> = (0..k).rev().map(|j| coef[j]).collect();
    first.push(if cst { coef[k] } else { 0.0 });
    if log {
        for x in first.iter_mut() {
            *x = x.exp();
        }
    }
    if !stats {
        let n = first.len();
        return arr_val(Array::new(1, n, first.into_iter().map(Value::num).collect()));
    }
    // regression statistics (5 rows)
    let n = ys.len() as f64;
    let pred: Vec<f64> = (0..ys.len()).map(|i| (0..k).map(|j| coef[j] * xs[j][i]).sum::<f64>() + if cst { coef[k] } else { 0.0 }).collect();
    let ssres: f64 = ys.iter().zip(&pred).map(|(y, p)| (y - p).powi(2)).sum();
    let my = if cst { ksum(&ys) / n } else { 0.0 };
    let sstot: f64 = ys.iter().map(|y| (y - my).powi(2)).sum();
    let ssreg = sstot - ssres;
    let df = n - k as f64 - if cst { 1.0 } else { 0.0 };
    let r2 = if sstot == 0.0 { 1.0 } else { ssreg / sstot };
    let sey = if df > 0.0 { (ssres / df).sqrt() } else { 0.0 };
    let f = if df > 0.0 && ssres > 0.0 { (ssreg / k as f64) / (ssres / df) } else { 0.0 };
    let cols = k + 1;
    let mut data = vec![Value::Err(XlErr::NA); 5 * cols];
    for (i, c) in first.iter().enumerate() {
        data[i] = Value::num(*c);
    }
    // standard errors from the inverse of X'X
    let p = k + if cst { 1 } else { 0 };
    let mut ata = vec![vec![0.0; p]; p];
    for i in 0..ys.len() {
        let mut r: Vec<f64> = (0..k).map(|j| xs[j][i]).collect();
        if cst {
            r.push(1.0);
        }
        for x in 0..p {
            for y in 0..p {
                ata[x][y] += r[x] * r[y];
            }
        }
    }
    if let Some(inv) = super::math::inverse(ata) {
        for j in 0..k {
            data[cols + (k - 1 - j)] = Value::num((inv[j][j] * sey * sey).sqrt());
        }
        data[cols + k] = if cst { Value::num((inv[k][k] * sey * sey).sqrt()) } else { Value::Err(XlErr::NA) };
    }
    data[2 * cols] = Value::num(r2);
    data[2 * cols + 1] = Value::num(sey);
    data[3 * cols] = Value::num(f);
    data[3 * cols + 1] = Value::num(df);
    data[4 * cols] = Value::num(ssreg);
    data[4 * cols + 1] = Value::num(ssres);
    arr_val(Array::new(5, cols, data))
}

fn trend(ev: &mut Eval, a: &[Expr], log: bool) -> Res {
    let ys_arr = tri!(array_of(ev, &a[0]));
    let mut ys: Vec<f64> = Vec::new();
    for v in ys_arr.data.iter() {
        match v {
            Value::Num(x) => ys.push(if log { if *x <= 0.0 { return Res::err(XlErr::Num) } else { x.ln() } } else { *x }),
            _ => return Res::err(XlErr::Value),
        }
    }
    let xs = tri!(xs_of(ev, a, ys.len(), 1));
    let cst = tri!(opt_bool(ev, a, 3, true));
    let coef = match regress(&ys, &xs, cst) {
        Some(c) => c,
        None => return Res::err(XlErr::Num),
    };
    let k = xs.len();
    let (newx, rows, cols): (Vec<Vec<f64>>, usize, usize) = if a.len() > 2 && !matches!(a[2], Expr::Missing) {
        let arr = tri!(array_of(ev, &a[2]));
        let nums: Vec<f64> = arr.data.iter().map(|v| if let Value::Num(x) = v { *x } else { 0.0 }).collect();
        if k == 1 {
            (vec![nums], arr.rows, arr.cols)
        } else if arr.cols == k {
            ((0..k).map(|j| (0..arr.rows).map(|r| nums[r * k + j]).collect()).collect(), arr.rows, 1)
        } else {
            ((0..k).map(|j| nums[j * arr.cols..(j + 1) * arr.cols].to_vec()).collect(), 1, arr.cols)
        }
    } else {
        (xs.clone(), ys_arr.rows, ys_arr.cols)
    };
    let n = newx[0].len();
    let out: Vec<Value> = (0..n).map(|i| {
        let mut y = (0..k).map(|j| coef[j] * newx[j][i]).sum::<f64>() + if cst { coef[k] } else { 0.0 };
        if log {
            y = y.exp();
        }
        Value::num(y)
    }).collect();
    arr_val(Array::new(rows, cols, out))
}
