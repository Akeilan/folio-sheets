//! Math and trigonometry.

use super::*;
use crate::calc::{compare, Eval, Res};
use crate::value::{Array, Value, XlErr};

pub static DEFS: &[Def] = &[
    d!("ABS", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.abs()))),
    d!("ACOS", 1, 1, |ev, a| math1(ev, a, |x| if x.abs() > 1.0 { Value::Err(XlErr::Num) } else { Value::num(x.acos()) })),
    d!("ACOSH", 1, 1, |ev, a| math1(ev, a, |x| if x < 1.0 { Value::Err(XlErr::Num) } else { Value::num(x.acosh()) })),
    d!("ACOT", 1, 1, |ev, a| math1(ev, a, |x| Value::num(std::f64::consts::FRAC_PI_2 - x.atan()))),
    d!("ACOTH", 1, 1, |ev, a| math1(ev, a, |x| if x.abs() <= 1.0 { Value::Err(XlErr::Num) } else { Value::num(0.5 * ((x + 1.0) / (x - 1.0)).ln()) })),
    d!("ASIN", 1, 1, |ev, a| math1(ev, a, |x| if x.abs() > 1.0 { Value::Err(XlErr::Num) } else { Value::num(x.asin()) })),
    d!("ASINH", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.asinh()))),
    d!("ATAN", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.atan()))),
    d!("ATAN2", 2, 2, |ev, a| math2(ev, a, |x, y| if x == 0.0 && y == 0.0 { Value::Err(XlErr::Div0) } else { Value::num(y.atan2(x)) })),
    d!("ATANH", 1, 1, |ev, a| math1(ev, a, |x| if x.abs() >= 1.0 { Value::Err(XlErr::Num) } else { Value::num(x.atanh()) })),
    d!("COS", 1, 1, |ev, a| math1(ev, a, |x| if x.abs() >= 134217728.0 { Value::Err(XlErr::Num) } else { Value::num(x.cos()) })),
    d!("COSH", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.cosh()))),
    d!("COT", 1, 1, |ev, a| math1(ev, a, |x| if x == 0.0 { Value::Err(XlErr::Div0) } else { Value::num(1.0 / x.tan()) })),
    d!("COTH", 1, 1, |ev, a| math1(ev, a, |x| if x == 0.0 { Value::Err(XlErr::Div0) } else { Value::num(1.0 / x.tanh()) })),
    d!("CSC", 1, 1, |ev, a| math1(ev, a, |x| if x == 0.0 { Value::Err(XlErr::Div0) } else { Value::num(1.0 / x.sin()) })),
    d!("CSCH", 1, 1, |ev, a| math1(ev, a, |x| if x == 0.0 { Value::Err(XlErr::Div0) } else { Value::num(1.0 / x.sinh()) })),
    d!("SEC", 1, 1, |ev, a| math1(ev, a, |x| Value::num(1.0 / x.cos()))),
    d!("SECH", 1, 1, |ev, a| math1(ev, a, |x| Value::num(1.0 / x.cosh()))),
    d!("SIN", 1, 1, |ev, a| math1(ev, a, |x| if x.abs() >= 134217728.0 { Value::Err(XlErr::Num) } else { Value::num(x.sin()) })),
    d!("SINH", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.sinh()))),
    d!("TAN", 1, 1, |ev, a| math1(ev, a, |x| if x.abs() >= 134217728.0 { Value::Err(XlErr::Num) } else { Value::num(x.tan()) })),
    d!("TANH", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.tanh()))),
    d!("DEGREES", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.to_degrees()))),
    d!("RADIANS", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.to_radians()))),
    d!("EXP", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.exp()))),
    d!("LN", 1, 1, |ev, a| math1(ev, a, |x| if x <= 0.0 { Value::Err(XlErr::Num) } else { Value::num(x.ln()) })),
    d!("LOG10", 1, 1, |ev, a| math1(ev, a, |x| if x <= 0.0 { Value::Err(XlErr::Num) } else { Value::num(x.log10()) })),
    d!("LOG", 1, 2, log),
    d!("SQRT", 1, 1, |ev, a| math1(ev, a, |x| if x < 0.0 { Value::Err(XlErr::Num) } else { Value::num(x.sqrt()) })),
    d!("SQRTPI", 1, 1, |ev, a| math1(ev, a, |x| if x < 0.0 { Value::Err(XlErr::Num) } else { Value::num((x * std::f64::consts::PI).sqrt()) })),
    d!("PI", 0, 0, |_, _| Res::num(std::f64::consts::PI)),
    d!("INT", 1, 1, |ev, a| math1(ev, a, |x| Value::num(x.floor()))),
    d!("SIGN", 1, 1, |ev, a| math1(ev, a, |x| Value::num(if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }))),
    d!("EVEN", 1, 1, |ev, a| math1(ev, a, |x| { let c = (x.abs() / 2.0).ceil() * 2.0; Value::num(if x < 0.0 { -c } else { c }) })),
    d!("ODD", 1, 1, |ev, a| math1(ev, a, |x| { let mut c = x.abs().ceil(); if c % 2.0 == 0.0 { c += 1.0; } Value::num(if x < 0.0 { -c } else { c }) })),
    d!("FACT", 1, 1, |ev, a| math1(ev, a, |x| if x < 0.0 { Value::Err(XlErr::Num) } else { Value::num(fact(x.floor())) })),
    d!("FACTDOUBLE", 1, 1, |ev, a| math1(ev, a, |x| {
        if x < -1.0 { return Value::Err(XlErr::Num); }
        let mut n = x.floor(); let mut p = 1.0;
        while n > 1.0 { p *= n; n -= 2.0; }
        Value::num(p)
    })),
    d!("GAMMA", 1, 1, |ev, a| math1(ev, a, |x| {
        if x <= 0.0 && x.fract() == 0.0 { return Value::Err(XlErr::Num); }
        Value::num(super::dist::gamma(x))
    })),
    d!("GAMMALN", 1, 1, |ev, a| math1(ev, a, |x| if x <= 0.0 { Value::Err(XlErr::Num) } else { Value::num(super::dist::ln_gamma(x)) })),
    d!("GAMMALN.PRECISE", 1, 1, |ev, a| math1(ev, a, |x| if x <= 0.0 { Value::Err(XlErr::Num) } else { Value::num(super::dist::ln_gamma(x)) })),
    d!("POWER", 2, 2, |ev, a| math2(ev, a, |x, y| crate::calc::pow(x, y))),
    d!("MOD", 2, 2, |ev, a| math2(ev, a, |x, y| {
        if y == 0.0 { return Value::Err(XlErr::Div0); }
        let r = x - y * (x / y).floor();
        Value::num(if crate::calc::num_eq(r, y) { 0.0 } else { r })
    })),
    d!("QUOTIENT", 2, 2, |ev, a| math2(ev, a, |x, y| if y == 0.0 { Value::Err(XlErr::Div0) } else { Value::num((x / y).trunc()) })),
    d!("ROUND", 2, 2, |ev, a| math2(ev, a, |x, n| Value::num(round_half_away(x, n)))),
    d!("ROUNDUP", 2, 2, |ev, a| math2(ev, a, |x, n| Value::num(round_dir(x, n, true)))),
    d!("ROUNDDOWN", 2, 2, |ev, a| math2(ev, a, |x, n| Value::num(round_dir(x, n, false)))),
    d!("TRUNC", 1, 2, trunc),
    d!("MROUND", 2, 2, |ev, a| math2(ev, a, |x, m| {
        if m == 0.0 { return Value::num(0.0); }
        if x * m < 0.0 { return Value::Err(XlErr::Num); }
        Value::num(round_half_away(x / m, 0.0) * m)
    })),
    d!("CEILING", 1, 2, |ev, a| { let n = a.len(); lift(ev, a, move |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let s = if n > 1 { match vnum(ev, &v[1]) { Ok(s) => s, Err(e) => return Value::Err(e) } } else { 1.0 };
        if s == 0.0 { return Value::num(0.0); }
        if x > 0.0 && s < 0.0 { return Value::Err(XlErr::Num); }
        if x < 0.0 && s < 0.0 { return Value::num(-snap(-x, -s, true)); }
        if x < 0.0 { return Value::num(-snap(-x, s, false)); }
        Value::num(snap(x, s, true))
    })}),
    d!("CEILING.PRECISE", 1, 2, ceiling_precise),
    d!("ISO.CEILING", 1, 2, ceiling_precise),
    d!("CEILING.MATH", 1, 3, |ev, a| { let n = a.len(); lift(ev, a, move |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let s = if n > 1 && !v[1].is_empty() { match vnum(ev, &v[1]) { Ok(s) => s.abs(), Err(e) => return Value::Err(e) } } else { 1.0 };
        let mode = if n > 2 { match vnum(ev, &v[2]) { Ok(m) => m, Err(e) => return Value::Err(e) } } else { 0.0 };
        if s == 0.0 { return Value::num(0.0); }
        if x >= 0.0 { Value::num(snap(x, s, true)) } else if mode != 0.0 { Value::num(-snap(-x, s, true)) } else { Value::num(-snap(-x, s, false)) }
    })}),
    d!("FLOOR", 1, 2, |ev, a| { let n = a.len(); lift(ev, a, move |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let s = if n > 1 { match vnum(ev, &v[1]) { Ok(s) => s, Err(e) => return Value::Err(e) } } else { 1.0 };
        if s == 0.0 { return if x == 0.0 { Value::num(0.0) } else { Value::Err(XlErr::Div0) }; }
        if x > 0.0 && s < 0.0 { return Value::Err(XlErr::Num); }
        if x < 0.0 && s < 0.0 { return Value::num(-snap(-x, -s, false)); }
        if x < 0.0 { return Value::num(-snap(-x, s, true)); }
        Value::num(snap(x, s, false))
    })}),
    d!("FLOOR.PRECISE", 1, 2, |ev, a| { let n = a.len(); lift(ev, a, move |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let s = if n > 1 { match vnum(ev, &v[1]) { Ok(s) => s.abs(), Err(e) => return Value::Err(e) } } else { 1.0 };
        if s == 0.0 { return Value::num(0.0); }
        Value::num(if x >= 0.0 { snap(x, s, false) } else { -snap(-x, s, true) })
    })}),
    d!("FLOOR.MATH", 1, 3, |ev, a| { let n = a.len(); lift(ev, a, move |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let s = if n > 1 && !v[1].is_empty() { match vnum(ev, &v[1]) { Ok(s) => s.abs(), Err(e) => return Value::Err(e) } } else { 1.0 };
        let mode = if n > 2 { match vnum(ev, &v[2]) { Ok(m) => m, Err(e) => return Value::Err(e) } } else { 0.0 };
        if s == 0.0 { return Value::num(0.0); }
        if x >= 0.0 { Value::num(snap(x, s, false)) } else if mode != 0.0 { Value::num(-snap(-x, s, false)) } else { Value::num(-snap(-x, s, true)) }
    })}),
    d!("GCD", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        let mut g = 0.0f64;
        for x in v { if x < 0.0 || x >= 9007199254740992.0 { return Res::err(XlErr::Num); } g = gcd(g, x.trunc()); }
        Res::num(g)
    }),
    d!("LCM", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        let mut l = 1.0f64;
        for x in v { let x = x.trunc(); if x < 0.0 { return Res::err(XlErr::Num); } if x == 0.0 { return Res::num(0.0); } l = l / gcd(l, x) * x; }
        Res::num(l)
    }),
    d!("COMBIN", 2, 2, |ev, a| math2(ev, a, |n, k| {
        let (n, k) = (n.trunc(), k.trunc());
        if n < 0.0 || k < 0.0 || k > n { return Value::Err(XlErr::Num); }
        Value::num(combin(n, k))
    })),
    d!("COMBINA", 2, 2, |ev, a| math2(ev, a, |n, k| {
        let (n, k) = (n.trunc(), k.trunc());
        if n < 0.0 || k < 0.0 || (n == 0.0 && k > 0.0) { return Value::Err(XlErr::Num); }
        if n == 0.0 && k == 0.0 { return Value::num(1.0); }
        Value::num(combin(n + k - 1.0, k))
    })),
    d!("MULTINOMIAL", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        let mut s = 0.0; let mut p = 1.0;
        for x in v { let x = x.trunc(); if x < 0.0 { return Res::err(XlErr::Num); } s += x; p *= fact(x); }
        Res::num((fact(s) / p).round())
    }),
    d!("SUM", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); Res::num(ksum(&v)) }),
    d!("SUMSQ", 1, MANY, |ev, a| { let v = tri!(collect(ev, a, Mode::Plain)); Res::num(v.iter().map(|x| x * x).sum()) }),
    d!("PRODUCT", 1, MANY, |ev, a| {
        let v = tri!(collect(ev, a, Mode::Plain));
        if v.is_empty() { return Res::num(0.0); }
        Res::num(v.iter().product())
    }),
    d!("SUMPRODUCT", 1, MANY, sumproduct),
    d!("SUMX2MY2", 2, 2, |ev, a| pairs(ev, a, |x, y| x * x - y * y)),
    d!("SUMX2PY2", 2, 2, |ev, a| pairs(ev, a, |x, y| x * x + y * y)),
    d!("SUMXMY2", 2, 2, |ev, a| pairs(ev, a, |x, y| (x - y) * (x - y))),
    d!("SUMIF", 2, 3, sumif),
    d!("SUMIFS", 3, MANY, sumifs),
    d!("SUBTOTAL", 2, MANY, subtotal),
    d!("AGGREGATE", 3, MANY, aggregate),
    d!("SERIESSUM", 4, 4, |ev, a| {
        let x = tri!(num(ev, &a[0])); let n = tri!(num(ev, &a[1])); let m = tri!(num(ev, &a[2]));
        let co = tri!(collect(ev, &a[3..4], Mode::Plain));
        let mut s = 0.0;
        for (i, c) in co.iter().enumerate() { s += c * x.powf(n + i as f64 * m); }
        Res::num(s)
    }),
    d!("RAND", 0, 0, |ev, _| Res::num(ev.ctx.rand()), vol),
    d!("RANDBETWEEN", 2, 2, |ev, a| {
        let lo = tri!(num(ev, &a[0])).ceil(); let hi = tri!(num(ev, &a[1])).floor();
        if lo > hi { return Res::err(XlErr::Num); }
        let r = ev.ctx.rand();
        Res::num(lo + (r * (hi - lo + 1.0)).floor())
    }, vol),
    d!("RANDARRAY", 0, 5, |ev, a| {
        let rows = tri!(opt_num(ev, a, 0, 1.0)) as i64; let cols = tri!(opt_num(ev, a, 1, 1.0)) as i64;
        let lo = tri!(opt_num(ev, a, 2, 0.0)); let hi = tri!(opt_num(ev, a, 3, 1.0)); let whole = tri!(opt_bool(ev, a, 4, false));
        if rows < 1 || cols < 1 || lo > hi { return Res::err(XlErr::Value); }
        if rows * cols > 10_000_000 { return Res::err(XlErr::Num); }
        let mut data = Vec::new();
        for _ in 0..rows * cols {
            let r = ev.ctx.rand();
            data.push(Value::num(if whole { (lo.ceil() + (r * (hi.floor() - lo.ceil() + 1.0)).floor()).min(hi.floor()) } else { lo + r * (hi - lo) }));
        }
        arr_val(Array::new(rows as usize, cols as usize, data))
    }, vol),
    d!("SEQUENCE", 1, 4, |ev, a| {
        let rows = tri!(opt_num(ev, a, 0, 1.0)).trunc() as i64; let cols = tri!(opt_num(ev, a, 1, 1.0)).trunc() as i64;
        let start = tri!(opt_num(ev, a, 2, 1.0)); let step = tri!(opt_num(ev, a, 3, 1.0));
        if rows < 1 || cols < 1 { return Res::err(XlErr::Calc); }
        if rows * cols > 10_000_000 { return Res::err(XlErr::Num); }
        let data = (0..rows * cols).map(|i| Value::num(start + step * i as f64)).collect();
        arr_val(Array::new(rows as usize, cols as usize, data))
    }),
    d!("MUNIT", 1, 1, |ev, a| {
        let n = tri!(num(ev, &a[0])).trunc() as i64;
        if n < 1 { return Res::err(XlErr::Value); }
        let n = n as usize;
        let mut data = vec![Value::num(0.0); n * n];
        for i in 0..n { data[i * n + i] = Value::num(1.0); }
        arr_val(Array::new(n, n, data))
    }),
    d!("MMULT", 2, 2, mmult),
    d!("MDETERM", 1, 1, |ev, a| {
        let m = tri!(matrix(ev, &a[0]));
        if m.is_empty() || m.len() != m[0].len() { return Res::err(XlErr::Value); }
        Res::num(det(m))
    }),
    d!("MINVERSE", 1, 1, |ev, a| {
        let m = tri!(matrix(ev, &a[0]));
        let n = m.len();
        if n == 0 || n != m[0].len() { return Res::err(XlErr::Value); }
        match inverse(m) {
            Some(inv) => arr_val(Array::new(n, n, inv.into_iter().flatten().map(Value::num).collect())),
            None => Res::err(XlErr::Num),
        }
    }),
    d!("ROMAN", 1, 2, roman),
    d!("ARABIC", 1, 1, arabic),
    d!("BASE", 2, 3, base),
    d!("DECIMAL", 2, 2, decimal),
];

fn log(ev: &mut Eval, a: &[Expr]) -> Res {
    let n = a.len();
    lift(ev, a, move |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let b = if n > 1 { match vnum(ev, &v[1]) { Ok(b) => b, Err(e) => return Value::Err(e) } } else { 10.0 };
        if x <= 0.0 || b <= 0.0 { return Value::Err(XlErr::Num); }
        if b == 1.0 { return Value::Err(XlErr::Div0); }
        Value::num(x.ln() / b.ln())
    })
}

fn trunc(ev: &mut Eval, a: &[Expr]) -> Res {
    let n = a.len();
    lift(ev, a, move |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let d = if n > 1 { match vnum(ev, &v[1]) { Ok(d) => d, Err(e) => return Value::Err(e) } } else { 0.0 };
        Value::num(round_dir(x, d, false))
    })
}

fn ceiling_precise(ev: &mut Eval, a: &[Expr]) -> Res {
    let n = a.len();
    lift(ev, a, move |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let s = if n > 1 { match vnum(ev, &v[1]) { Ok(s) => s.abs(), Err(e) => return Value::Err(e) } } else { 1.0 };
        if s == 0.0 { return Value::num(0.0); }
        Value::num(if x >= 0.0 { snap(x, s, true) } else { -snap(-x, s, false) })
    })
}

/// x >= 0 to a multiple of s >= 0, up or down, tolerant of binary noise.
pub fn snap(x: f64, s: f64, up: bool) -> f64 {
    let q = x / s;
    let r = q.round();
    if (q - r).abs() < 1e-12 * q.abs().max(1.0) {
        return r * s;
    }
    if up { q.ceil() * s } else { q.floor() * s }
}

/// Decimal rounding with halves away from zero (on the 15-digit value, so 2.675 -> 2.68).
pub fn round_half_away(x: f64, n: f64) -> f64 {
    let n = n.trunc() as i32;
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    if n >= 0 {
        let (ip, fp) = crate::numfmt::fixed(x.abs(), n.min(30) as usize);
        let s = if fp.is_empty() { ip } else { format!("{ip}.{fp}") };
        let v: f64 = s.parse().unwrap_or(0.0);
        return if x < 0.0 { -v } else { v };
    }
    let p = 10f64.powi(-n);
    let q = crate::numfmt::fixed(x.abs() / p, 0).0.parse::<f64>().unwrap_or(0.0) * p;
    if x < 0.0 { -q } else { q }
}

pub fn round_dir(x: f64, n: f64, up: bool) -> f64 {
    let n = n.trunc() as i32;
    let p = 10f64.powi(n);
    let y = x.abs() * p;
    // remove binary noise first (0.1*3 = 0.30000000000000004)
    let y15: f64 = format!("{:.12e}", y).parse().unwrap_or(y);
    let r = if up { y15.ceil() } else { y15.floor() };
    let v = r / p;
    if x < 0.0 { -v } else { v }
}

pub fn fact(n: f64) -> f64 {
    if n > 170.0 {
        return f64::INFINITY;
    }
    let mut p = 1.0;
    let mut i = 2.0;
    while i <= n {
        p *= i;
        i += 1.0;
    }
    p
}

pub fn gcd(a: f64, b: f64) -> f64 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b > 0.5 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

pub fn combin(n: f64, k: f64) -> f64 {
    let k = k.min(n - k);
    let mut r = 1.0;
    let mut i = 0.0;
    while i < k {
        r = r * (n - i) / (i + 1.0);
        i += 1.0;
    }
    r.round()
}

/// Sum with compensation (and the original's snapping of tiny totals to zero).
pub fn ksum(v: &[f64]) -> f64 {
    let mut s = 0.0;
    let mut c = 0.0;
    let mut maxabs = 0.0f64;
    for &x in v {
        maxabs = maxabs.max(x.abs());
        let y = x - c;
        let t = s + y;
        c = (t - s) - y;
        s = t;
    }
    if s != 0.0 && s.abs() < maxabs * 1e-15 {
        0.0
    } else {
        s
    }
}

fn pairs(ev: &mut Eval, a: &[Expr], f: fn(f64, f64) -> f64) -> Res {
    let x = tri!(array_of(ev, &a[0]));
    let y = tri!(array_of(ev, &a[1]));
    if x.data.len() != y.data.len() {
        return Res::err(XlErr::NA);
    }
    let mut s = 0.0;
    for (p, q) in x.data.iter().zip(y.data.iter()) {
        if let Value::Err(e) = p { return Res::err(*e); }
        if let Value::Err(e) = q { return Res::err(*e); }
        if let (Value::Num(p), Value::Num(q)) = (p, q) {
            s += f(*p, *q);
        }
    }
    Res::num(s)
}

fn sumproduct(ev: &mut Eval, a: &[Expr]) -> Res {
    let mut arrays = Vec::new();
    for e in a {
        let r = ev.eval(e);
        let arr = tri!(res_array(ev, r));
        arrays.push(arr);
    }
    let (rows, cols) = (arrays[0].rows, arrays[0].cols);
    if arrays.iter().any(|x| x.rows != rows || x.cols != cols) {
        return Res::err(XlErr::Value);
    }
    let mut s = 0.0;
    for i in 0..rows * cols {
        let mut p = 1.0;
        for arr in &arrays {
            match &arr.data[i] {
                Value::Num(n) => p *= n,
                Value::Err(e) => return Res::err(*e),
                Value::Bool(_) if arrays.len() == 1 => p = 0.0,
                _ => p = 0.0,
            }
        }
        s += p;
    }
    Res::num(s)
}

/// The sum range of SUMIF: a single cell or a range resized to the criteria range's shape.
pub fn resized(ev: &mut Eval, e: &Expr, rows: usize, cols: usize, like: Option<&crate::calc::Area>) -> Result<Rc<Array>, XlErr> {
    match ev.eval(e) {
        Res::R(areas) if areas.len() == 1 => {
            let a = &areas[0];
            let rc0 = a.rect;
            let r2 = rc0.r1 + rows as u32 - 1;
            let c2 = rc0.c1 + cols as u32 - 1;
            let rc = Rect::new(rc0.r1, rc0.c1, r2.min(crate::cellref::MAX_ROWS - 1), c2.min(crate::cellref::MAX_COLS - 1));
            let _ = like;
            let arr = ev.area_array(a.sheet, rc);
            Ok(Rc::new(arr))
        }
        r => {
            let arr = res_array(ev, r)?;
            if arr.rows != rows || arr.cols != cols {
                return Err(XlErr::Value);
            }
            Ok(arr)
        }
    }
}

fn sumif(ev: &mut Eval, a: &[Expr]) -> Res {
    let r = ev.eval(&a[0]);
    let first_area = if let Res::R(ar) = &r { ar.first().cloned() } else { None };
    let crit_arr = tri!(res_array(ev, r));
    let crit = ev.eval_scalar(&a[1]);
    let c = Criteria::new(&crit, ev.d1904());
    let sum_arr = if a.len() > 2 && !matches!(a[2], Expr::Missing) {
        tri!(resized(ev, &a[2], crit_arr.rows, crit_arr.cols, first_area.as_ref()))
    } else {
        crit_arr.clone()
    };
    let mut s = Vec::new();
    for (i, v) in crit_arr.data.iter().enumerate() {
        if c.matches(v) {
            match sum_arr.data.get(i) {
                Some(Value::Num(n)) => s.push(*n),
                Some(Value::Err(e)) => return Res::err(*e),
                _ => {}
            }
        }
    }
    Res::num(ksum(&s))
}

fn sumifs(ev: &mut Eval, a: &[Expr]) -> Res {
    let (mask, rows, cols) = tri!(ifs_mask(ev, &a[1..]));
    let sum_arr = tri!(resized(ev, &a[0], rows, cols, None));
    if sum_arr.rows != rows || sum_arr.cols != cols {
        return Res::err(XlErr::Value);
    }
    let mut s = Vec::new();
    for (i, m) in mask.iter().enumerate() {
        if *m {
            match &sum_arr.data[i] {
                Value::Num(n) => s.push(*n),
                Value::Err(e) => return Res::err(*e),
                _ => {}
            }
        }
    }
    Res::num(ksum(&s))
}

/// SUBTOTAL(function_num, ref...): 1-11 include hidden rows, 101-111 skip them; nested
/// SUBTOTALs are skipped.
fn subtotal(ev: &mut Eval, a: &[Expr]) -> Res {
    let f = tri!(num(ev, &a[0])).trunc() as i64;
    let skip_hidden = f > 100;
    let code = if f > 100 { f - 100 } else { f };
    if !(1..=11).contains(&code) {
        return Res::err(XlErr::Value);
    }
    let mut vals = Vec::new();
    let mut counta = 0usize;
    for e in &a[1..] {
        let r = ev.eval(e);
        match r {
            Res::R(areas) => {
                for ar in &areas {
                    let rc = ev.bound(ar);
                    for (rr, cc, v) in ev.ctx.cells_in(ar.sheet, rc) {
                        if skip_hidden && ev.ctx.row_hidden(ar.sheet, rr) {
                            continue;
                        }
                        if ev.ctx.is_subtotal(ar.sheet, rr, cc) {
                            continue;
                        }
                        match v {
                            Value::Num(n) => {
                                vals.push(n);
                                counta += 1;
                            }
                            Value::Err(e) => return Res::err(e),
                            Value::Empty => {}
                            _ => counta += 1,
                        }
                    }
                }
            }
            Res::V(Value::Err(e)) => return Res::err(e),
            _ => return Res::err(XlErr::Value),
        }
    }
    Res::num(match aggregate_code(code, &vals, counta) {
        Ok(v) => v,
        Err(e) => return Res::err(e),
    })
}

pub fn aggregate_code(code: i64, v: &[f64], counta: usize) -> Result<f64, XlErr> {
    use super::stats::*;
    Ok(match code {
        1 => {
            if v.is_empty() {
                return Err(XlErr::Div0);
            }
            ksum(v) / v.len() as f64
        }
        2 => v.len() as f64,
        3 => counta as f64,
        4 => v.iter().copied().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.max(x)))).unwrap_or(0.0),
        5 => v.iter().copied().fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x)))).unwrap_or(0.0),
        6 => {
            if v.is_empty() { 0.0 } else { v.iter().product() }
        }
        7 => stdev(v, true)?,
        8 => stdev(v, false)?,
        9 => ksum(v),
        10 => var(v, true)?,
        11 => var(v, false)?,
        12 => median(v)?,
        13 => mode_sngl(v)?,
        _ => return Err(XlErr::Value),
    })
}

fn aggregate(ev: &mut Eval, a: &[Expr]) -> Res {
    let f = tri!(num(ev, &a[0])).trunc() as i64;
    let opt = tri!(num(ev, &a[1])).trunc() as i64;
    if !(1..=19).contains(&f) || !(0..=7).contains(&opt) {
        return Res::err(XlErr::Value);
    }
    let skip_hidden = matches!(opt, 1 | 3 | 5 | 7);
    let skip_err = matches!(opt, 2 | 3 | 6 | 7);
    let skip_sub = matches!(opt, 0..=3);
    let mut vals = Vec::new();
    let refs = if f >= 14 { &a[2..3] } else { &a[2..] };
    for e in refs {
        let r = ev.eval(e);
        match r {
            Res::R(areas) => {
                for ar in &areas {
                    let rc = ev.bound(ar);
                    for (rr, cc, v) in ev.ctx.cells_in(ar.sheet, rc) {
                        if skip_hidden && ev.ctx.row_hidden(ar.sheet, rr) {
                            continue;
                        }
                        if skip_sub && ev.ctx.is_subtotal(ar.sheet, rr, cc) {
                            continue;
                        }
                        match v {
                            Value::Num(n) => vals.push(n),
                            Value::Err(e) if !skip_err => return Res::err(e),
                            _ => {}
                        }
                    }
                }
            }
            r => {
                for v in res_values(ev, r) {
                    match v {
                        Value::Num(n) => vals.push(n),
                        Value::Err(e) if !skip_err => return Res::err(e),
                        _ => {}
                    }
                }
            }
        }
    }
    use super::stats::*;
    if f >= 14 {
        let k = if a.len() > 3 { tri!(num(ev, &a[3])) } else { return Res::err(XlErr::Value) };
        return match f {
            14 => kth(&vals, k, true),
            15 => kth(&vals, k, false),
            16 => percentile(&vals, k, true).map(Res::num).unwrap_or_else(Res::err),
            17 => quartile(&vals, k, true).map(Res::num).unwrap_or_else(Res::err),
            18 => percentile(&vals, k, false).map(Res::num).unwrap_or_else(Res::err),
            _ => quartile(&vals, k, false).map(Res::num).unwrap_or_else(Res::err),
        };
    }
    match f {
        3 => Res::num(vals.len() as f64),
        _ => match aggregate_code(f, &vals, vals.len()) {
            Ok(v) => Res::num(v),
            Err(e) => Res::err(e),
        },
    }
}

fn kth(v: &[f64], k: f64, large: bool) -> Res {
    let k = k.ceil() as usize;
    if k < 1 || k > v.len() {
        return Res::err(XlErr::Num);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Res::num(if large { s[s.len() - k] } else { s[k - 1] })
}

pub fn matrix(ev: &mut Eval, e: &Expr) -> Result<Vec<Vec<f64>>, XlErr> {
    let arr = array_of(ev, e)?;
    let mut m = Vec::with_capacity(arr.rows);
    for r in 0..arr.rows {
        let mut row = Vec::with_capacity(arr.cols);
        for c in 0..arr.cols {
            match arr.get(r, c) {
                Value::Num(n) => row.push(*n),
                Value::Err(e) => return Err(*e),
                _ => return Err(XlErr::Value),
            }
        }
        m.push(row);
    }
    Ok(m)
}

fn mmult(ev: &mut Eval, a: &[Expr]) -> Res {
    let x = tri!(matrix(ev, &a[0]));
    let y = tri!(matrix(ev, &a[1]));
    if x.is_empty() || y.is_empty() || x[0].len() != y.len() {
        return Res::err(XlErr::Value);
    }
    let (n, m, p) = (x.len(), y.len(), y[0].len());
    let mut data = Vec::with_capacity(n * p);
    for i in 0..n {
        for j in 0..p {
            let mut s = 0.0;
            for k in 0..m {
                s += x[i][k] * y[k][j];
            }
            data.push(Value::num(s));
        }
    }
    arr_val(Array::new(n, p, data))
}

pub fn det(mut m: Vec<Vec<f64>>) -> f64 {
    let n = m.len();
    let mut d = 1.0;
    for i in 0..n {
        let mut p = i;
        for r in i + 1..n {
            if m[r][i].abs() > m[p][i].abs() {
                p = r;
            }
        }
        if m[p][i] == 0.0 {
            return 0.0;
        }
        if p != i {
            m.swap(p, i);
            d = -d;
        }
        d *= m[i][i];
        for r in i + 1..n {
            let f = m[r][i] / m[i][i];
            for c in i..n {
                m[r][c] -= f * m[i][c];
            }
        }
    }
    d
}

pub fn inverse(m: Vec<Vec<f64>>) -> Option<Vec<Vec<f64>>> {
    let n = m.len();
    let mut a: Vec<Vec<f64>> = m.into_iter().enumerate().map(|(i, mut r)| {
        r.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
        r
    }).collect();
    for i in 0..n {
        let mut p = i;
        for r in i + 1..n {
            if a[r][i].abs() > a[p][i].abs() {
                p = r;
            }
        }
        if a[p][i].abs() < 1e-300 {
            return None;
        }
        a.swap(p, i);
        let piv = a[i][i];
        for c in 0..2 * n {
            a[i][c] /= piv;
        }
        for r in 0..n {
            if r != i {
                let f = a[r][i];
                if f != 0.0 {
                    for c in 0..2 * n {
                        a[r][c] -= f * a[i][c];
                    }
                }
            }
        }
    }
    Some(a.into_iter().map(|r| r[n..].to_vec()).collect())
}

fn roman(ev: &mut Eval, a: &[Expr]) -> Res {
    let n = tri!(num(ev, &a[0])).trunc() as i64;
    let form = tri!(opt_num(ev, a, 1, 0.0)) as i64;
    if !(0..=3999).contains(&n) || !(0..=4).contains(&form) {
        return Res::err(XlErr::Value);
    }
    if n == 0 {
        return Res::V(Value::str(""));
    }
    // classic form; the concise forms (1-4) allow more subtractive pairs
    let table: &[(i64, &str)] = match form {
        0 => &[(1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"), (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")],
        1 => &[(1000, "M"), (950, "LM"), (900, "CM"), (500, "D"), (450, "LD"), (400, "CD"), (100, "C"), (95, "VC"), (90, "XC"), (50, "L"), (45, "VL"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")],
        2 => &[(1000, "M"), (990, "XM"), (950, "LM"), (900, "CM"), (500, "D"), (490, "XD"), (450, "LD"), (400, "CD"), (100, "C"), (99, "IC"), (95, "VC"), (90, "XC"), (50, "L"), (49, "IL"), (45, "VL"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")],
        3 => &[(1000, "M"), (995, "VM"), (990, "XM"), (950, "LM"), (900, "CM"), (500, "D"), (495, "VD"), (490, "XD"), (450, "LD"), (400, "CD"), (100, "C"), (99, "IC"), (95, "VC"), (90, "XC"), (50, "L"), (49, "IL"), (45, "VL"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")],
        _ => &[(1000, "M"), (999, "IM"), (995, "VM"), (990, "XM"), (950, "LM"), (900, "CM"), (500, "D"), (499, "ID"), (495, "VD"), (490, "XD"), (450, "LD"), (400, "CD"), (100, "C"), (99, "IC"), (95, "VC"), (90, "XC"), (50, "L"), (49, "IL"), (45, "VL"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")],
    };
    let mut n = n;
    let mut s = String::new();
    for (v, t) in table {
        while n >= *v {
            s.push_str(t);
            n -= v;
        }
    }
    Res::V(Value::string(s))
}

fn arabic(ev: &mut Eval, a: &[Expr]) -> Res {
    let t = tri!(text(ev, &a[0])).trim().to_uppercase();
    let (neg, t) = match t.strip_prefix('-') {
        Some(x) => (true, x.to_string()),
        None => (false, t),
    };
    let val = |c: char| -> Option<i64> {
        Some(match c { 'I' => 1, 'V' => 5, 'X' => 10, 'L' => 50, 'C' => 100, 'D' => 500, 'M' => 1000, _ => return None })
    };
    let mut total = 0i64;
    let cs: Vec<char> = t.chars().collect();
    for i in 0..cs.len() {
        let v = match val(cs[i]) { Some(v) => v, None => return Res::err(XlErr::Value) };
        let next = cs.get(i + 1).and_then(|c| val(*c)).unwrap_or(0);
        if v < next { total -= v } else { total += v }
    }
    Res::num(if neg { -total as f64 } else { total as f64 })
}

fn base(ev: &mut Eval, a: &[Expr]) -> Res {
    let n = tri!(num(ev, &a[0])).trunc();
    let radix = tri!(num(ev, &a[1])).trunc() as u32;
    let min_len = tri!(opt_num(ev, a, 2, 0.0)) as usize;
    if n < 0.0 || n >= 9007199254740992.0 || !(2..=36).contains(&radix) {
        return Res::err(XlErr::Num);
    }
    let mut x = n as u64;
    let mut s = Vec::new();
    if x == 0 {
        s.push(b'0');
    }
    while x > 0 {
        let d = (x % radix as u64) as u8;
        s.push(if d < 10 { b'0' + d } else { b'A' + d - 10 });
        x /= radix as u64;
    }
    while s.len() < min_len {
        s.push(b'0');
    }
    s.reverse();
    Res::V(Value::string(String::from_utf8(s).unwrap()))
}

fn decimal(ev: &mut Eval, a: &[Expr]) -> Res {
    let t = tri!(text(ev, &a[0]));
    let radix = tri!(num(ev, &a[1])).trunc() as u32;
    if !(2..=36).contains(&radix) {
        return Res::err(XlErr::Num);
    }
    let mut v = 0f64;
    for c in t.trim().chars() {
        let d = match c.to_digit(36) { Some(d) if d < radix => d, _ => return Res::err(XlErr::Num) };
        v = v * radix as f64 + d as f64;
    }
    Res::num(v)
}

/// Sort helper used by several functions: values in the original's order.
pub fn sort_values(v: &mut [Value]) {
    v.sort_by(compare);
}
