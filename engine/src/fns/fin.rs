//! Financial functions: time value of money, cash flows, depreciation, securities.

use super::date::{days360, yearfrac};
use super::*;
use crate::calc::{Eval, Res};
use crate::dates::{days_in_month, serial_to_ymd, ymd_to_serial};
use crate::value::{Value, XlErr};

type F = fn(&Eval, &[f64]) -> Result<f64, XlErr>;

/// Scalar numeric function with defaults for its optional arguments, lifted over arrays.
fn fl(ev: &mut Eval, a: &[Expr], defs: &[f64], f: F) -> Res {
    let n = defs.len();
    let vals: Vec<Value> = (0..n).map(|i| if is_missing(a, i) { Value::num(defs[i]) } else { ev.eval_value(&a[i]) }).collect();
    Res::V(lift_values(ev, &vals, move |ev, v| {
        let mut x = Vec::with_capacity(n);
        for vv in v {
            match vnum(ev, vv) {
                Ok(k) => x.push(k),
                Err(e) => return Value::Err(e),
            }
        }
        match f(ev, &x) {
            Ok(r) if r.is_finite() => Value::num(r),
            Ok(_) => Value::Err(XlErr::Num),
            Err(e) => Value::Err(e),
        }
    }))
}

pub static DEFS: &[Def] = &[
    d!("PV", 3, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0], |_, x| Ok(pv(x[0], x[1], x[2], x[3], flag(x[4]))))),
    d!("FV", 3, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0], |_, x| Ok(fv(x[0], x[1], x[2], x[3], flag(x[4]))))),
    d!("PMT", 3, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0], |_, x| {
        if x[1] == 0.0 { return Err(XlErr::Num); }
        Ok(pmt(x[0], x[1], x[2], x[3], flag(x[4])))
    })),
    d!("NPER", 3, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0], |_, x| nper(x[0], x[1], x[2], x[3], flag(x[4])))),
    d!("RATE", 3, 6, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.1], |_, x| rate(x[0], x[1], x[2], x[3], flag(x[4]), x[5]))),
    d!("IPMT", 4, 6, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0], |_, x| ipmt(x[0], x[1], x[2], x[3], x[4], flag(x[5])))),
    d!("PPMT", 4, 6, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0, 0.0], |_, x| {
        let i = ipmt(x[0], x[1], x[2], x[3], x[4], flag(x[5]))?;
        Ok(pmt(x[0], x[2], x[3], x[4], flag(x[5])) - i)
    })),
    d!("CUMIPMT", 6, 6, |ev, a| fl(ev, a, &[0.0; 6], |_, x| cum(x, true))),
    d!("CUMPRINC", 6, 6, |ev, a| fl(ev, a, &[0.0; 6], |_, x| cum(x, false))),
    d!("ISPMT", 4, 4, |ev, a| fl(ev, a, &[0.0; 4], |_, x| {
        if x[2] == 0.0 { return Err(XlErr::Div0); }
        Ok(x[3] * x[0] * (x[1] / x[2] - 1.0))
    })),
    d!("PDURATION", 3, 3, |ev, a| fl(ev, a, &[0.0; 3], |_, x| {
        if x[0] <= 0.0 || x[1] <= 0.0 || x[2] <= 0.0 { return Err(XlErr::Num); }
        Ok((x[2].ln() - x[1].ln()) / (1.0 + x[0]).ln())
    })),
    d!("RRI", 3, 3, |ev, a| fl(ev, a, &[0.0; 3], |_, x| {
        if x[0] <= 0.0 { return Err(XlErr::Num); }
        if x[1] == 0.0 { return Err(XlErr::Num); }
        Ok((x[2] / x[1]).powf(1.0 / x[0]) - 1.0)
    })),
    d!("EFFECT", 2, 2, |ev, a| fl(ev, a, &[0.0; 2], |_, x| {
        let p = x[1].trunc();
        if x[0] <= 0.0 || p < 1.0 { return Err(XlErr::Num); }
        Ok((1.0 + x[0] / p).powf(p) - 1.0)
    })),
    d!("NOMINAL", 2, 2, |ev, a| fl(ev, a, &[0.0; 2], |_, x| {
        let p = x[1].trunc();
        if x[0] <= 0.0 || p < 1.0 { return Err(XlErr::Num); }
        Ok(((1.0 + x[0]).powf(1.0 / p) - 1.0) * p)
    })),
    d!("DOLLARDE", 2, 2, |ev, a| fl(ev, a, &[0.0; 2], |_, x| dollar(x[0], x[1], true))),
    d!("DOLLARFR", 2, 2, |ev, a| fl(ev, a, &[0.0; 2], |_, x| dollar(x[0], x[1], false))),
    d!("FVSCHEDULE", 2, 2, |ev, a| {
        let p = tri!(num(ev, &a[0]));
        let mut out = p;
        for v in values_of(ev, &a[1]) {
            match v {
                Value::Num(r) => out *= 1.0 + r,
                Value::Empty => {}
                Value::Err(e) => return Res::err(e),
                _ => return Res::err(XlErr::Value),
            }
        }
        Res::num(out)
    }),
    d!("NPV", 2, MANY, |ev, a| {
        let r = tri!(num(ev, &a[0]));
        let v = tri!(collect(ev, &a[1..], Mode::Plain));
        if r == -1.0 { return Res::err(XlErr::Div0); }
        Res::num(npv(r, &v))
    }),
    d!("IRR", 1, 2, |ev, a| {
        let v = tri!(collect(ev, &a[..1], Mode::Plain));
        let g = tri!(opt_num(ev, a, 1, 0.1));
        match irr(&v, g) { Some(x) => Res::num(x), None => Res::err(XlErr::Num) }
    }),
    d!("MIRR", 3, 3, |ev, a| {
        let v = tri!(collect(ev, &a[..1], Mode::Plain));
        let fr = tri!(num(ev, &a[1]));
        let rr = tri!(num(ev, &a[2]));
        let n = v.len() as f64;
        let pos: Vec<f64> = v.iter().map(|x| x.max(0.0)).collect();
        let neg: Vec<f64> = v.iter().map(|x| x.min(0.0)).collect();
        let npv_pos = npv(rr, &pos);
        let npv_neg = npv(fr, &neg);
        if npv_neg == 0.0 || npv_pos == 0.0 || n < 2.0 { return Res::err(XlErr::Div0); }
        let r = (-npv_pos * (1.0 + rr).powf(n) / (npv_neg * (1.0 + fr))).powf(1.0 / (n - 1.0)) - 1.0;
        if r.is_finite() { Res::num(r) } else { Res::err(XlErr::Num) }
    }),
    d!("XNPV", 3, 3, |ev, a| {
        let r = tri!(num(ev, &a[0]));
        let (v, d) = tri!(flows(ev, &a[1], &a[2]));
        if r <= -1.0 { return Res::err(XlErr::Num); }
        Res::num(xnpv(r, &v, &d))
    }),
    d!("XIRR", 2, 3, |ev, a| {
        let (v, d) = tri!(flows(ev, &a[0], &a[1]));
        let g = tri!(opt_num(ev, a, 2, 0.1));
        if !v.iter().any(|x| *x > 0.0) || !v.iter().any(|x| *x < 0.0) { return Res::err(XlErr::Num); }
        match solve(|r| xnpv(r, &v, &d), g, -0.999999) { Some(x) => Res::num(x), None => Res::err(XlErr::Num) }
    }),
    // ---- depreciation
    d!("SLN", 3, 3, |ev, a| fl(ev, a, &[0.0; 3], |_, x| {
        if x[2] == 0.0 { return Err(XlErr::Div0); }
        Ok((x[0] - x[1]) / x[2])
    })),
    d!("SYD", 4, 4, |ev, a| fl(ev, a, &[0.0; 4], |_, x| {
        let (c, s, l, p) = (x[0], x[1], x[2], x[3]);
        if l <= 0.0 || p <= 0.0 || p > l || s < 0.0 { return Err(XlErr::Num); }
        Ok((c - s) * (l - p + 1.0) * 2.0 / (l * (l + 1.0)))
    })),
    d!("DB", 4, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 12.0], |_, x| db(x[0], x[1], x[2], x[3], x[4]))),
    d!("DDB", 4, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 2.0], |_, x| ddb(x[0], x[1], x[2], x[3], x[4]))),
    d!("VDB", 5, 7, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0], |_, x| vdb(x[0], x[1], x[2], x[3], x[4], x[5], x[6] != 0.0))),
    // ---- securities
    d!("TBILLPRICE", 3, 3, |ev, a| fl(ev, a, &[0.0; 3], |ev, x| {
        let dsm = tbill_days(ev, x[0], x[1])?;
        if x[2] <= 0.0 { return Err(XlErr::Num); }
        let p = 100.0 * (1.0 - x[2] * dsm / 360.0);
        if p <= 0.0 { return Err(XlErr::Num); }
        Ok(p)
    })),
    d!("TBILLYIELD", 3, 3, |ev, a| fl(ev, a, &[0.0; 3], |ev, x| {
        let dsm = tbill_days(ev, x[0], x[1])?;
        if x[2] <= 0.0 { return Err(XlErr::Num); }
        Ok((100.0 - x[2]) / x[2] * 360.0 / dsm)
    })),
    d!("TBILLEQ", 3, 3, |ev, a| fl(ev, a, &[0.0; 3], |ev, x| {
        let dsm = tbill_days(ev, x[0], x[1])?;
        let disc = x[2];
        if disc <= 0.0 { return Err(XlErr::Num); }
        if dsm <= 182.0 {
            return Ok(365.0 * disc / (360.0 - disc * dsm));
        }
        let price = 100.0 * (1.0 - disc * dsm / 360.0);
        if price <= 0.0 { return Err(XlErr::Num); }
        let t = dsm / 365.0;
        Ok((-t + (t * t - (2.0 * t - 1.0) * (1.0 - 100.0 / price)).sqrt()) / (t - 0.5))
    })),
    d!("DISC", 4, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0], |ev, x| {
        let yf = span(ev, x[0], x[1], x[4])?;
        if x[2] <= 0.0 || x[3] <= 0.0 { return Err(XlErr::Num); }
        Ok((x[3] - x[2]) / x[3] / yf)
    })),
    d!("INTRATE", 4, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0], |ev, x| {
        let yf = span(ev, x[0], x[1], x[4])?;
        if x[2] <= 0.0 || x[3] <= 0.0 { return Err(XlErr::Num); }
        Ok((x[3] - x[2]) / x[2] / yf)
    })),
    d!("RECEIVED", 4, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0], |ev, x| {
        let yf = span(ev, x[0], x[1], x[4])?;
        if x[2] <= 0.0 || x[3] <= 0.0 { return Err(XlErr::Num); }
        let d = 1.0 - x[3] * yf;
        if d <= 0.0 { return Err(XlErr::Num); }
        Ok(x[2] / d)
    })),
    d!("PRICEDISC", 4, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0], |ev, x| {
        let yf = span(ev, x[0], x[1], x[4])?;
        if x[2] <= 0.0 || x[3] <= 0.0 { return Err(XlErr::Num); }
        Ok(x[3] * (1.0 - x[2] * yf))
    })),
    d!("YIELDDISC", 4, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 0.0], |ev, x| {
        let yf = span(ev, x[0], x[1], x[4])?;
        if x[2] <= 0.0 || x[3] <= 0.0 { return Err(XlErr::Num); }
        Ok((x[3] - x[2]) / x[2] / yf)
    })),
    d!("ACCRINTM", 4, 5, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 1000.0, 0.0], |ev, x| {
        let yf = span(ev, x[0], x[1], x[4])?;
        if x[2] <= 0.0 || x[3] <= 0.0 { return Err(XlErr::Num); }
        Ok(x[3] * x[2] * yf)
    })),
    d!("ACCRINT", 6, 8, |ev, a| fl(ev, a, &[0.0, 0.0, 0.0, 0.0, 1000.0, 0.0, 0.0, 1.0], |ev, x| accrint(ev, x))),
    d!("PRICEMAT", 5, 6, |ev, a| fl(ev, a, &[0.0; 6], |ev, x| {
        let (s, m, i, r, y, b) = (x[0].floor(), x[1].floor(), x[2].floor(), x[3], x[4], x[5]);
        if s >= m || r < 0.0 || y < 0.0 { return Err(XlErr::Num); }
        let dim = span(ev, i, m, b)?;
        let a = span(ev, i, s, b)?;
        let dsm = span(ev, s, m, b)?;
        Ok((100.0 + dim * r * 100.0) / (1.0 + dsm * y) - a * r * 100.0)
    })),
    d!("YIELDMAT", 5, 6, |ev, a| fl(ev, a, &[0.0; 6], |ev, x| {
        let (s, m, i, r, p, b) = (x[0].floor(), x[1].floor(), x[2].floor(), x[3], x[4], x[5]);
        if s >= m || r < 0.0 || p <= 0.0 { return Err(XlErr::Num); }
        let dim = span(ev, i, m, b)?;
        let a = span(ev, i, s, b)?;
        let dsm = span(ev, s, m, b)?;
        let t = p / 100.0 + a * r;
        Ok(((1.0 + dim * r) - t) / t / dsm)
    })),
    d!("COUPDAYBS", 3, 4, |ev, a| fl(ev, a, &[0.0; 4], |ev, x| Ok(coupon(ev, x[0], x[1], x[2], x[3])?.a))),
    d!("COUPDAYS", 3, 4, |ev, a| fl(ev, a, &[0.0; 4], |ev, x| Ok(coupon(ev, x[0], x[1], x[2], x[3])?.e))),
    d!("COUPDAYSNC", 3, 4, |ev, a| fl(ev, a, &[0.0; 4], |ev, x| Ok(coupon(ev, x[0], x[1], x[2], x[3])?.dsc))),
    d!("COUPNCD", 3, 4, |ev, a| fl(ev, a, &[0.0; 4], |ev, x| Ok(coupon(ev, x[0], x[1], x[2], x[3])?.ncd))),
    d!("COUPPCD", 3, 4, |ev, a| fl(ev, a, &[0.0; 4], |ev, x| Ok(coupon(ev, x[0], x[1], x[2], x[3])?.pcd))),
    d!("COUPNUM", 3, 4, |ev, a| fl(ev, a, &[0.0; 4], |ev, x| Ok(coupon(ev, x[0], x[1], x[2], x[3])?.n))),
    d!("PRICE", 6, 7, |ev, a| fl(ev, a, &[0.0; 7], |ev, x| {
        let c = coupon(ev, x[0], x[1], x[5], x[6])?;
        if x[2] < 0.0 || x[3] < 0.0 || x[4] <= 0.0 { return Err(XlErr::Num); }
        Ok(price(&c, x[2], x[3], x[4], x[5]))
    })),
    d!("YIELD", 6, 7, |ev, a| fl(ev, a, &[0.0; 7], |ev, x| {
        let c = coupon(ev, x[0], x[1], x[5], x[6])?;
        let (r, pr, red, f) = (x[2], x[3], x[4], x[5]);
        if r < 0.0 || pr <= 0.0 || red <= 0.0 { return Err(XlErr::Num); }
        if c.n <= 1.0 {
            let dsr = c.e - c.a;
            let t = pr / 100.0 + c.a / c.e * r / f;
            return Ok(((red / 100.0 + r / f) - t) / t * f * c.e / dsr);
        }
        solve(|y| price(&c, r, y, red, f) - pr, if r > 0.0 { r } else { 0.05 }, -f + 1e-9).ok_or(XlErr::Num)
    })),
    d!("DURATION", 5, 6, |ev, a| fl(ev, a, &[0.0; 6], |ev, x| {
        let c = coupon(ev, x[0], x[1], x[4], x[5])?;
        if x[2] < 0.0 || x[3] < 0.0 { return Err(XlErr::Num); }
        Ok(duration(&c, x[2], x[3], x[4]))
    })),
    d!("MDURATION", 5, 6, |ev, a| fl(ev, a, &[0.0; 6], |ev, x| {
        let c = coupon(ev, x[0], x[1], x[4], x[5])?;
        if x[2] < 0.0 || x[3] < 0.0 { return Err(XlErr::Num); }
        Ok(duration(&c, x[2], x[3], x[4]) / (1.0 + x[3] / x[4]))
    })),
];

fn flag(x: f64) -> bool {
    x != 0.0
}

pub fn pv(r: f64, n: f64, p: f64, f: f64, t: bool) -> f64 {
    if r == 0.0 {
        return -(f + p * n);
    }
    let g = (1.0 + r).powf(n);
    -(f + p * (1.0 + r * if t { 1.0 } else { 0.0 }) * (g - 1.0) / r) / g
}

pub fn fv(r: f64, n: f64, p: f64, v: f64, t: bool) -> f64 {
    if r == 0.0 {
        return -(v + p * n);
    }
    let g = (1.0 + r).powf(n);
    -(v * g + p * (1.0 + r * if t { 1.0 } else { 0.0 }) * (g - 1.0) / r)
}

pub fn pmt(r: f64, n: f64, v: f64, f: f64, t: bool) -> f64 {
    if r == 0.0 {
        return -(v + f) / n;
    }
    let g = (1.0 + r).powf(n);
    -r * (v * g + f) / ((1.0 + r * if t { 1.0 } else { 0.0 }) * (g - 1.0))
}

fn nper(r: f64, p: f64, v: f64, f: f64, t: bool) -> Result<f64, XlErr> {
    if r == 0.0 {
        if p == 0.0 {
            return Err(XlErr::Num);
        }
        return Ok(-(v + f) / p);
    }
    let k = p * (1.0 + r * if t { 1.0 } else { 0.0 });
    let x = (k - f * r) / (k + v * r);
    if x <= 0.0 || r <= -1.0 {
        return Err(XlErr::Num);
    }
    Ok(x.ln() / (1.0 + r).ln())
}

fn rate(n: f64, p: f64, v: f64, f: f64, t: bool, guess: f64) -> Result<f64, XlErr> {
    if n <= 0.0 {
        return Err(XlErr::Num);
    }
    let tt = if t { 1.0 } else { 0.0 };
    let g = |r: f64| -> f64 {
        if r.abs() < 1e-12 {
            return v + p * n + f;
        }
        let q = (1.0 + r).powf(n);
        v * q + p * (1.0 + r * tt) * (q - 1.0) / r + f
    };
    solve(g, guess, -1.0 + 1e-10).ok_or(XlErr::Num)
}

/// Newton's method with a numeric derivative; falls back to scanning for a sign change
/// and bisecting. `lo` is the smallest rate the function is defined for.
fn solve(f: impl Fn(f64) -> f64, guess: f64, lo: f64) -> Option<f64> {
    let mut x = guess;
    for _ in 0..100 {
        let y = f(x);
        if !y.is_finite() {
            break;
        }
        if y.abs() < 1e-10 {
            return Some(x);
        }
        let h = 1e-7 * x.abs().max(1e-3);
        let d = (f(x + h) - f(x - h)) / (2.0 * h);
        if d == 0.0 || !d.is_finite() {
            break;
        }
        let nx = x - y / d;
        if !nx.is_finite() {
            break;
        }
        let nx = if nx <= lo { (x + lo) / 2.0 } else { nx };
        if (nx - x).abs() < 1e-12 * nx.abs().max(1.0) {
            return if f(nx).abs() < 1e-6 { Some(nx) } else { None };
        }
        x = nx;
    }
    // scan for a bracket
    let mut pts = vec![lo + 1e-9];
    let mut s = -0.99;
    while s < 10.0 {
        if s > lo {
            pts.push(s);
        }
        s += 0.01;
    }
    let mut prev: Option<(f64, f64)> = None;
    for &p in &pts {
        let y = f(p);
        if !y.is_finite() {
            prev = None;
            continue;
        }
        if let Some((px, py)) = prev {
            if py.signum() != y.signum() {
                let (mut a, mut b, mut fa) = (px, p, py);
                for _ in 0..200 {
                    let m = (a + b) / 2.0;
                    let fm = f(m);
                    if fm.signum() == fa.signum() {
                        a = m;
                        fa = fm;
                    } else {
                        b = m;
                    }
                }
                return Some((a + b) / 2.0);
            }
        }
        prev = Some((p, y));
    }
    None
}

fn ipmt(r: f64, per: f64, n: f64, v: f64, f: f64, t: bool) -> Result<f64, XlErr> {
    if per < 1.0 || per > n {
        return Err(XlErr::Num);
    }
    let p = pmt(r, n, v, f, t);
    let i = if per == 1.0 {
        if t { 0.0 } else { -v }
    } else if t {
        fv(r, per - 2.0, p, v, true) - p
    } else {
        fv(r, per - 1.0, p, v, false)
    };
    Ok(i * r)
}

fn cum(x: &[f64], interest: bool) -> Result<f64, XlErr> {
    let (r, n, v, s, e, t) = (x[0], x[1].trunc(), x[2], x[3].trunc(), x[4].trunc(), x[5]);
    if r <= 0.0 || n <= 0.0 || v <= 0.0 || s < 1.0 || e < s || e > n || (t != 0.0 && t != 1.0) {
        return Err(XlErr::Num);
    }
    let tt = t != 0.0;
    let p = pmt(r, n, v, 0.0, tt);
    let mut sum = 0.0;
    let mut per = s;
    while per <= e {
        let i = ipmt(r, per, n, v, 0.0, tt)?;
        sum += if interest { i } else { p - i };
        per += 1.0;
    }
    Ok(sum)
}

fn dollar(x: f64, f: f64, de: bool) -> Result<f64, XlErr> {
    let f = f.trunc();
    if f < 0.0 {
        return Err(XlErr::Num);
    }
    if f == 0.0 {
        return Err(XlErr::Div0);
    }
    let p = 10f64.powf(f.log10().ceil());
    let int = x.trunc();
    let frac = x - int;
    Ok(if de { int + frac * p / f } else { int + frac * f / p })
}

pub fn npv(r: f64, v: &[f64]) -> f64 {
    let mut s = 0.0;
    let mut d = 1.0;
    for x in v {
        d *= 1.0 + r;
        s += x / d;
    }
    s
}

pub fn irr(v: &[f64], guess: f64) -> Option<f64> {
    if !v.iter().any(|x| *x > 0.0) || !v.iter().any(|x| *x < 0.0) {
        return None;
    }
    solve(|r| npv(r, v), guess, -1.0 + 1e-10)
}

fn xnpv(r: f64, v: &[f64], d: &[f64]) -> f64 {
    let d0 = d[0];
    v.iter().zip(d).map(|(x, t)| x / (1.0 + r).powf((t - d0) / 365.0)).sum()
}

fn flows(ev: &mut Eval, va: &Expr, da: &Expr) -> Result<(Vec<f64>, Vec<f64>), XlErr> {
    let vs = values_of(ev, va);
    let ds = values_of(ev, da);
    if vs.len() != ds.len() || vs.is_empty() {
        return Err(XlErr::Num);
    }
    let mut v = Vec::new();
    let mut d = Vec::new();
    for (x, t) in vs.iter().zip(&ds) {
        let x = match x {
            Value::Num(n) => *n,
            Value::Err(e) => return Err(*e),
            _ => return Err(XlErr::Value),
        };
        let t = match t {
            Value::Num(n) => n.trunc(),
            Value::Err(e) => return Err(*e),
            _ => return Err(XlErr::Value),
        };
        v.push(x);
        d.push(t);
    }
    if d.iter().any(|t| *t < d[0] || *t < 0.0) {
        return Err(XlErr::Num);
    }
    Ok((v, d))
}

fn db(c: f64, s: f64, l: f64, p: f64, m: f64) -> Result<f64, XlErr> {
    let m = m.trunc();
    if c < 0.0 || s < 0.0 || l <= 0.0 || p <= 0.0 || m < 1.0 || m > 12.0 || p > l + 1.0 {
        return Err(XlErr::Num);
    }
    if c == 0.0 {
        return Ok(0.0);
    }
    let rate = ((1.0 - (s / c).powf(1.0 / l)) * 1000.0).round() / 1000.0;
    let p = p.trunc();
    let mut total = c * rate * m / 12.0;
    if p == 1.0 {
        return Ok(total);
    }
    let mut dep = 0.0;
    let mut i = 2.0;
    while i <= p {
        dep = if i == l + 1.0 { (c - total) * rate * (12.0 - m) / 12.0 } else { (c - total) * rate };
        total += dep;
        i += 1.0;
    }
    Ok(dep)
}

fn ddb(c: f64, s: f64, l: f64, p: f64, f: f64) -> Result<f64, XlErr> {
    if c < 0.0 || s < 0.0 || l <= 0.0 || p <= 0.0 || f <= 0.0 || p > l {
        return Err(XlErr::Num);
    }
    // a part period counts as the whole period it falls in
    let p = p.ceil();
    let mut rate = f / l;
    let old;
    if rate >= 1.0 {
        rate = 1.0;
        old = if p == 1.0 { c } else { 0.0 };
    } else {
        old = c * (1.0 - rate).powf(p - 1.0);
    }
    let new = c * (1.0 - rate).powf(p);
    let dep = if new < s { old - s } else { old - new };
    Ok(dep.max(0.0))
}

fn vdb(c: f64, s: f64, l: f64, start: f64, end: f64, f: f64, no_switch: bool) -> Result<f64, XlErr> {
    if c < 0.0 || s < 0.0 || l <= 0.0 || start < 0.0 || end < start || end > l || f <= 0.0 {
        return Err(XlErr::Num);
    }
    let periods = l.ceil() as usize;
    let mut sched = Vec::with_capacity(periods);
    let mut value = c;
    let mut switched = false;
    let mut sl_amount = 0.0;
    for i in 0..periods {
        let remaining = l - i as f64;
        let ddb_dep = value * f / l;
        let dep = if switched {
            sl_amount
        } else {
            let sl = (value - s) / remaining;
            if !no_switch && sl > ddb_dep {
                switched = true;
                sl_amount = sl;
                sl
            } else {
                ddb_dep
            }
        };
        let dep = dep.min(value - s).max(0.0);
        sched.push(dep);
        value -= dep;
    }
    let mut total = 0.0;
    for (i, dep) in sched.iter().enumerate() {
        let (a, b) = (i as f64, i as f64 + 1.0);
        let lo = a.max(start);
        let hi = b.min(end);
        if hi > lo {
            total += dep * (hi - lo);
        }
    }
    Ok(total)
}

fn check_basis(b: f64) -> Result<i64, XlErr> {
    let b = b.trunc();
    if !(0.0..=4.0).contains(&b) {
        return Err(XlErr::Num);
    }
    Ok(b as i64)
}

/// Year fraction between settlement and maturity under a basis (both truncated).
fn span(ev: &Eval, s: f64, m: f64, b: f64) -> Result<f64, XlErr> {
    let b = check_basis(b)?;
    let (s, m) = (s.floor(), m.floor());
    if s >= m && s != m {
        return Err(XlErr::Num);
    }
    if s == m {
        return Err(XlErr::Num);
    }
    yearfrac(ev, s, m, b)
}

fn tbill_days(ev: &Eval, s: f64, m: f64) -> Result<f64, XlErr> {
    let _ = ev;
    let (s, m) = (s.floor(), m.floor());
    let d = m - s;
    if d <= 0.0 || d > 365.0 {
        return Err(XlErr::Num);
    }
    Ok(d)
}

type Ymd = (i64, u32, u32);

fn months_back(mat: Ymd, k: i64) -> Ymd {
    let total = mat.0 * 12 + (mat.1 as i64 - 1) - k;
    let y = total.div_euclid(12);
    let m = (total.rem_euclid(12) + 1) as u32;
    let eom = mat.2 == days_in_month(mat.0, mat.1);
    let dim = days_in_month(y, m);
    (y, m, if eom { dim } else { mat.2.min(dim) })
}

pub struct Coupon {
    pub pcd: f64,
    pub ncd: f64,
    pub n: f64,
    /// days from the previous coupon to settlement
    pub a: f64,
    /// days in the coupon period
    pub e: f64,
    /// days from settlement to the next coupon
    pub dsc: f64,
}

fn day_count(basis: i64, a: Ymd, b: Ymd, sa: f64, sb: f64) -> f64 {
    match basis {
        0 => days360(a, b, false),
        4 => days360(a, b, true),
        _ => sb - sa,
    }
}

fn coupon(ev: &Eval, s: f64, m: f64, f: f64, b: f64) -> Result<Coupon, XlErr> {
    let basis = check_basis(b)?;
    let (s, m) = (s.floor(), m.floor());
    if s >= m || !(f == 1.0 || f == 2.0 || f == 4.0) {
        return Err(XlErr::Num);
    }
    let d1904 = ev.d1904();
    let sy = serial_to_ymd(s, d1904).ok_or(XlErr::Num)?;
    let my = serial_to_ymd(m, d1904).ok_or(XlErr::Num)?;
    let step = 12 / f as i64;
    let mut k = 1;
    let mut pcd = months_back(my, step);
    while pcd > sy {
        k += 1;
        pcd = months_back(my, step * k);
    }
    let ncd = months_back(my, step * (k - 1));
    let to_serial = |d: Ymd| ymd_to_serial(d.0, d.1 as i64, d.2 as i64, d1904).ok_or(XlErr::Num);
    let pcd_s = to_serial(pcd)?;
    let ncd_s = to_serial(ncd)?;
    let e = match basis {
        1 => ncd_s - pcd_s,
        3 => 365.0 / f,
        _ => 360.0 / f,
    };
    let a = day_count(basis, pcd, sy, pcd_s, s);
    let dsc = match basis {
        0 | 4 => e - a,
        _ => ncd_s - s,
    };
    Ok(Coupon { pcd: pcd_s, ncd: ncd_s, n: k as f64, a, e, dsc })
}

fn price(c: &Coupon, r: f64, y: f64, red: f64, f: f64) -> f64 {
    let cpn = 100.0 * r / f;
    let ae = c.a / c.e;
    let de = c.dsc / c.e;
    if c.n <= 1.0 {
        return (red + cpn) / (1.0 + de * y / f) - cpn * ae;
    }
    let q = 1.0 + y / f;
    let mut p = red / q.powf(c.n - 1.0 + de);
    for k in 1..=(c.n as i64) {
        p += cpn / q.powf(k as f64 - 1.0 + de);
    }
    p - cpn * ae
}

fn duration(c: &Coupon, r: f64, y: f64, f: f64) -> f64 {
    let cpn = 100.0 * r / f;
    let de = c.dsc / c.e;
    let q = 1.0 + y / f;
    let (mut num, mut den) = (0.0, 0.0);
    for k in 1..=(c.n as i64) {
        let t = k as f64 - 1.0 + de;
        let cf = if k == c.n as i64 { cpn + 100.0 } else { cpn };
        let pv = cf / q.powf(t);
        num += t * pv;
        den += pv;
    }
    num / den / f
}

fn accrint(ev: &Eval, x: &[f64]) -> Result<f64, XlErr> {
    let (issue, first, settle) = (x[0].floor(), x[1].floor(), x[2].floor());
    let (r, par, f) = (x[3], x[4], x[5].trunc());
    let basis = check_basis(x[6])?;
    let from_issue = x[7] != 0.0;
    if r <= 0.0 || par <= 0.0 || !(f == 1.0 || f == 2.0 || f == 4.0) || issue >= settle {
        return Err(XlErr::Num);
    }
    let start = if !from_issue && settle > first { first } else { issue };
    let d1904 = ev.d1904();
    let fy = serial_to_ymd(first, d1904).ok_or(XlErr::Num)?;
    let to_serial = |d: Ymd| ymd_to_serial(d.0, d.1 as i64, d.2 as i64, d1904).ok_or(XlErr::Num);
    let step = 12 / f as i64;
    // quasi-coupon periods anchored on first_interest, covering [start, settle]
    let mut k: i64 = 0;
    while to_serial(months_back(fy, step * k))? > start {
        k += 1;
    }
    let mut total = 0.0;
    loop {
        let p0 = months_back(fy, step * k);
        let p1 = months_back(fy, step * (k - 1));
        let (s0, s1) = (to_serial(p0)?, to_serial(p1)?);
        if s0 >= settle {
            break;
        }
        let lo = s0.max(start);
        let hi = s1.min(settle);
        if hi > lo {
            let ly = serial_to_ymd(lo, d1904).ok_or(XlErr::Num)?;
            let hy = serial_to_ymd(hi, d1904).ok_or(XlErr::Num)?;
            let days = day_count(basis, ly, hy, lo, hi);
            let nl = match basis {
                1 => s1 - s0,
                3 => 365.0 / f,
                _ => 360.0 / f,
            };
            total += days / nl;
        }
        k -= 1;
        if k < -100000 {
            break;
        }
    }
    Ok(par * r / f * total)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6 * b.abs().max(1.0)
    }
    #[test]
    fn tvm() {
        assert!(close(pmt(0.08 / 12.0, 10.0, 10000.0, 0.0, false), -1037.0320893));
        assert!(close(pv(0.08 / 12.0, 240.0, 500.0, 0.0, false), -59777.14585));
        assert!(close(fv(0.06 / 12.0, 10.0, -200.0, -500.0, true), 2581.403374));
        assert!(close(nper(0.12 / 12.0, -100.0, -1000.0, 10000.0, true).unwrap(), 59.67386567));
        assert!(close(rate(48.0, -200.0, 8000.0, 0.0, false, 0.1).unwrap(), 0.007701472));
        assert!(close(ipmt(0.1 / 12.0, 1.0, 36.0, 8000.0, 0.0, false).unwrap(), -66.66666667));
        assert!(close(irr(&[-70000.0, 12000.0, 15000.0, 18000.0, 21000.0, 26000.0], 0.1).unwrap(), 0.086630948));
        assert!(close(npv(0.1, &[-10000.0, 3000.0, 4200.0, 6800.0]), 1188.443412));
    }
    #[test]
    fn depreciation() {
        assert!(close(db(1_000_000.0, 100_000.0, 6.0, 1.0, 7.0).unwrap(), 186083.3333));
        assert!(close(db(1_000_000.0, 100_000.0, 6.0, 7.0, 7.0).unwrap(), 15845.09833));
        assert!(close(ddb(2400.0, 300.0, 10.0, 1.0, 2.0).unwrap(), 480.0));
        assert!(close(ddb(2400.0, 300.0, 10.0, 10.0, 2.0).unwrap(), 22.1225472));
        assert!(close(vdb(2400.0, 300.0, 10.0, 0.0, 1.0, 2.0, false).unwrap(), 480.0));
    }
}
