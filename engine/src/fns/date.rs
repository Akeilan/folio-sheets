//! Date and time functions, and the day-count bases the financial functions share.

use super::*;
use crate::calc::{Eval, Res};
use crate::dates::{days_in_month, is_leap, serial_to_ymd, weekday, ymd_to_serial};
use crate::value::{Value, XlErr};

fn ymd(ev: &Eval, x: f64) -> Result<(i64, u32, u32), XlErr> {
    serial_to_ymd(x, ev.ctx.date1904()).ok_or(XlErr::Num)
}

fn serial(ev: &Eval, y: i64, m: i64, d: i64) -> Result<f64, XlErr> {
    ymd_to_serial(y, m, d, ev.ctx.date1904()).ok_or(XlErr::Num)
}

fn date_arg(ev: &Eval, v: &Value) -> Result<f64, XlErr> {
    let x = vnum(ev, v)?;
    if x < 0.0 {
        return Err(XlErr::Num);
    }
    Ok(x.floor())
}

pub static DEFS: &[Def] = &[
    d!("DATE", 3, 3, |ev, a| lift(ev, a, |ev, v| {
        let y = match vnum(ev, &v[0]) { Ok(x) => x.trunc() as i64, Err(e) => return Value::Err(e) };
        let m = match vnum(ev, &v[1]) { Ok(x) => x.trunc() as i64, Err(e) => return Value::Err(e) };
        let d = match vnum(ev, &v[2]) { Ok(x) => x.trunc() as i64, Err(e) => return Value::Err(e) };
        if !(0..10000).contains(&y) { return Value::Err(XlErr::Num); }
        match serial(ev, y, m, d) { Ok(s) => Value::num(s), Err(e) => Value::Err(e) }
    })),
    d!("TIME", 3, 3, |ev, a| lift(ev, a, |ev, v| {
        let h = match vnum(ev, &v[0]) { Ok(x) => x.trunc(), Err(e) => return Value::Err(e) };
        let m = match vnum(ev, &v[1]) { Ok(x) => x.trunc(), Err(e) => return Value::Err(e) };
        let s = match vnum(ev, &v[2]) { Ok(x) => x.trunc(), Err(e) => return Value::Err(e) };
        let total = h * 3600.0 + m * 60.0 + s;
        if total < 0.0 || h > 32767.0 { return Value::Err(XlErr::Num); }
        Value::num((total / 86400.0).fract())
    })),
    d!("YEAR", 1, 1, |ev, a| lift(ev, a, |ev, v| match date_arg(ev, &v[0]).and_then(|x| ymd(ev, x)) { Ok(d) => Value::num(d.0 as f64), Err(e) => Value::Err(e) })),
    d!("MONTH", 1, 1, |ev, a| lift(ev, a, |ev, v| match date_arg(ev, &v[0]).and_then(|x| ymd(ev, x)) { Ok(d) => Value::num(d.1 as f64), Err(e) => Value::Err(e) })),
    d!("DAY", 1, 1, |ev, a| lift(ev, a, |ev, v| match date_arg(ev, &v[0]).and_then(|x| ymd(ev, x)) { Ok(d) => Value::num(d.2 as f64), Err(e) => Value::Err(e) })),
    d!("HOUR", 1, 1, |ev, a| lift(ev, a, |ev, v| match vnum(ev, &v[0]) { Ok(x) if x >= 0.0 => Value::num(crate::dates::hms(x).0 as f64), Ok(_) => Value::Err(XlErr::Num), Err(e) => Value::Err(e) })),
    d!("MINUTE", 1, 1, |ev, a| lift(ev, a, |ev, v| match vnum(ev, &v[0]) { Ok(x) if x >= 0.0 => Value::num(crate::dates::hms(x).1 as f64), Ok(_) => Value::Err(XlErr::Num), Err(e) => Value::Err(e) })),
    d!("SECOND", 1, 1, |ev, a| lift(ev, a, |ev, v| match vnum(ev, &v[0]) { Ok(x) if x >= 0.0 => Value::num(crate::dates::hms(x).2 as f64), Ok(_) => Value::Err(XlErr::Num), Err(e) => Value::Err(e) })),
    d!("NOW", 0, 0, |ev, _| Res::num(ev.ctx.now()), vol),
    d!("TODAY", 0, 0, |ev, _| Res::num(ev.ctx.now().floor()), vol),
    d!("DATEVALUE", 1, 1, |ev, a| lift(ev, a, |ev, v| {
        let s = match vtext(&v[0]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        match crate::input::parse_number_text(&s, ev.d1904()) {
            Some((x, Some(f))) if f.contains('d') || f.contains('y') || f.contains("mmm") => Value::num(x.floor()),
            _ => Value::Err(XlErr::Value),
        }
    })),
    d!("TIMEVALUE", 1, 1, |ev, a| lift(ev, a, |ev, v| {
        let s = match vtext(&v[0]) { Ok(s) => s, Err(e) => return Value::Err(e) };
        match crate::input::parse_number_text(&s, ev.d1904()) {
            Some((x, Some(f))) if f.contains('h') => Value::num(x.fract()),
            _ => Value::Err(XlErr::Value),
        }
    })),
    d!("WEEKDAY", 1, 2, |ev, a| {
        let n = a.len();
        lift(ev, a, move |ev, v| {
            let x = match date_arg(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
            let t = if n > 1 { match vnum(ev, &v[1]) { Ok(t) => t.trunc() as i64, Err(e) => return Value::Err(e) } } else { 1 };
            let w = weekday(x, ev.d1904()) as i64; // 1 = Sunday
            let r = match t {
                1 => w,
                2 => (w + 5) % 7 + 1,
                3 => (w + 5) % 7,
                11..=17 => { let start = t - 10; ((w - 1 - start).rem_euclid(7)) + 1 } // start: 1 = Monday
                _ => return Value::Err(XlErr::Num),
            };
            Value::num(r as f64)
        })
    }),
    d!("WEEKNUM", 1, 2, |ev, a| {
        let n = a.len();
        lift(ev, a, move |ev, v| {
            let x = match date_arg(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
            let t = if n > 1 { match vnum(ev, &v[1]) { Ok(t) => t.trunc() as i64, Err(e) => return Value::Err(e) } } else { 1 };
            if t == 21 { return Value::num(isoweek(ev, x) as f64); }
            // first day of the week: 1 Sunday, 2/11 Monday, 12 Tue ... 17 Sun
            let start = match t { 1 | 17 => 1, 2 | 11 => 2, 12 => 3, 13 => 4, 14 => 5, 15 => 6, 16 => 7, _ => return Value::Err(XlErr::Num) };
            let (y, _, _) = match ymd(ev, x) { Ok(d) => d, Err(e) => return Value::Err(e) };
            let jan1 = match serial(ev, y, 1, 1) { Ok(s) => s, Err(e) => return Value::Err(e) };
            let wd = weekday(jan1, ev.d1904()) as i64; // 1 Sunday
            let offset = (wd - start).rem_euclid(7);
            Value::num(((x - jan1) as i64 + offset).div_euclid(7) as f64 + 1.0)
        })
    }),
    d!("ISOWEEKNUM", 1, 1, |ev, a| lift(ev, a, |ev, v| match date_arg(ev, &v[0]) { Ok(x) => Value::num(isoweek(ev, x) as f64), Err(e) => Value::Err(e) })),
    d!("EDATE", 2, 2, |ev, a| lift(ev, a, |ev, v| {
        let x = match date_arg(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let m = match vnum(ev, &v[1]) { Ok(m) => m.trunc() as i64, Err(e) => return Value::Err(e) };
        let (y, mo, d) = match ymd(ev, x) { Ok(d) => d, Err(e) => return Value::Err(e) };
        let tm = y * 12 + mo as i64 - 1 + m;
        let (ny, nm) = (tm.div_euclid(12), tm.rem_euclid(12) as u32 + 1);
        let nd = d.min(days_in_month(ny, nm));
        match serial(ev, ny, nm as i64, nd as i64) { Ok(s) => Value::num(s), Err(e) => Value::Err(e) }
    })),
    d!("EOMONTH", 2, 2, |ev, a| lift(ev, a, |ev, v| {
        let x = match date_arg(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let m = match vnum(ev, &v[1]) { Ok(m) => m.trunc() as i64, Err(e) => return Value::Err(e) };
        let (y, mo, _) = match ymd(ev, x) { Ok(d) => d, Err(e) => return Value::Err(e) };
        let tm = y * 12 + mo as i64 - 1 + m;
        let (ny, nm) = (tm.div_euclid(12), tm.rem_euclid(12) as u32 + 1);
        match serial(ev, ny, nm as i64, days_in_month(ny, nm) as i64) { Ok(s) => Value::num(s), Err(e) => Value::Err(e) }
    })),
    d!("DAYS", 2, 2, |ev, a| lift(ev, a, |ev, v| {
        let e = match date_arg(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let s = match date_arg(ev, &v[1]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        Value::num(e - s)
    })),
    d!("DAYS360", 2, 3, |ev, a| {
        let n = a.len();
        lift(ev, a, move |ev, v| {
            let s = match date_arg(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
            let e = match date_arg(ev, &v[1]) { Ok(x) => x, Err(e) => return Value::Err(e) };
            let eu = if n > 2 { crate::calc::to_bool(&v[2]).unwrap_or(false) } else { false };
            match (ymd(ev, s), ymd(ev, e)) {
                (Ok(a), Ok(b)) => Value::num(days360(a, b, eu)),
                _ => Value::Err(XlErr::Num),
            }
        })
    }),
    d!("DATEDIF", 3, 3, |ev, a| {
        let s = tri!(num(ev, &a[0])).floor();
        let e = tri!(num(ev, &a[1])).floor();
        let unit = tri!(text(ev, &a[2])).to_uppercase();
        if s > e { return Res::err(XlErr::Num); }
        let (y1, m1, d1) = tri!(ymd(ev, s));
        let (y2, m2, d2) = tri!(ymd(ev, e));
        let months = (y2 - y1) * 12 + m2 as i64 - m1 as i64 - if d2 < d1 { 1 } else { 0 };
        Res::num(match unit.as_str() {
            "D" => e - s,
            "M" => months as f64,
            "Y" => (months / 12) as f64,
            "YM" => (months % 12) as f64,
            "MD" => {
                if d2 >= d1 { (d2 - d1) as f64 } else {
                    let (py, pm) = if m2 == 1 { (y2 - 1, 12) } else { (y2, m2 - 1) };
                    (days_in_month(py, pm) as i64 - d1 as i64 + d2 as i64).max(0) as f64
                }
            }
            "YD" => {
                let mut y = y2;
                let mut start = tri!(serial(ev, y, m1 as i64, d1 as i64));
                if start > e { y -= 1; start = tri!(serial(ev, y, m1 as i64, d1 as i64)); }
                e - start
            }
            _ => return Res::err(XlErr::Num),
        })
    }),
    d!("YEARFRAC", 2, 3, |ev, a| {
        let s = tri!(num(ev, &a[0])).floor();
        let e = tri!(num(ev, &a[1])).floor();
        let b = tri!(opt_num(ev, a, 2, 0.0)).trunc() as i64;
        if !(0..=4).contains(&b) || s < 0.0 || e < 0.0 { return Res::err(XlErr::Num); }
        match yearfrac(ev, s, e, b) { Ok(v) => Res::num(v), Err(e) => Res::err(e) }
    }),
    d!("NETWORKDAYS", 2, 3, |ev, a| networkdays(ev, a, false)),
    d!("NETWORKDAYS.INTL", 2, 4, |ev, a| networkdays(ev, a, true)),
    d!("WORKDAY", 2, 3, |ev, a| workday(ev, a, false)),
    d!("WORKDAY.INTL", 2, 4, |ev, a| workday(ev, a, true)),
];

fn isoweek(ev: &Eval, x: f64) -> i64 {
    // weekday with Monday = 1
    let wd = ((weekday(x, ev.d1904()) as i64 + 5) % 7) + 1;
    let thursday = x + (4 - wd) as f64;
    let (y, _, _) = serial_to_ymd(thursday, ev.d1904()).unwrap_or((1900, 1, 1));
    let jan1 = ymd_to_serial(y, 1, 1, ev.d1904()).unwrap_or(1.0);
    ((thursday - jan1) / 7.0).floor() as i64 + 1
}

pub fn days360(a: (i64, u32, u32), b: (i64, u32, u32), european: bool) -> f64 {
    let (y1, m1, mut d1) = (a.0, a.1 as i64, a.2 as i64);
    let (y2, m2, mut d2) = (b.0, b.1 as i64, b.2 as i64);
    if european {
        d1 = d1.min(30);
        d2 = d2.min(30);
    } else {
        let last_feb = |y: i64, m: i64, d: i64| m == 2 && d == days_in_month(y, 2) as i64;
        if last_feb(y1, m1, d1) {
            d1 = 30;
        }
        if d1 == 31 {
            d1 = 30;
        }
        if d2 == 31 && d1 >= 30 {
            d2 = 30;
        }
    }
    ((y2 - y1) * 360 + (m2 - m1) * 30 + (d2 - d1)) as f64
}

/// Fraction of a year between two dates under a day-count basis (0: 30/360 US,
/// 1: actual/actual, 2: actual/360, 3: actual/365, 4: 30/360 European).
pub fn yearfrac(ev: &Eval, s: f64, e: f64, basis: i64) -> Result<f64, XlErr> {
    let (s, e) = if s > e { (e, s) } else { (s, e) };
    let a = ymd(ev, s)?;
    let b = ymd(ev, e)?;
    Ok(match basis {
        0 => {
            // US 30/360 as the original computes YEARFRAC
            let (mut d1, mut d2) = (a.2 as i64, b.2 as i64);
            let feb_end = |y: i64, m: u32, d: i64| m == 2 && d == days_in_month(y, 2) as i64;
            if feb_end(a.0, a.1, d1) && feb_end(b.0, b.1, d2) {
                d2 = 30;
            }
            if feb_end(a.0, a.1, d1) {
                d1 = 30;
            }
            if d2 == 31 && d1 >= 30 {
                d2 = 30;
            }
            if d1 == 31 {
                d1 = 30;
            }
            (((b.0 - a.0) * 360 + (b.1 as i64 - a.1 as i64) * 30 + (d2 - d1)) as f64) / 360.0
        }
        1 => {
            let days = e - s;
            if a.0 == b.0 {
                days / if is_leap(a.0) { 366.0 } else { 365.0 }
            } else if b.0 == a.0 + 1 && (b.1, b.2) <= (a.1, a.2) {
                // within one year: leap if the span contains 29 Feb
                let mut leap = false;
                if is_leap(a.0) && (a.1 < 3) {
                    leap = true;
                }
                if is_leap(b.0) && (b.1 > 2 || (b.1 == 2 && b.2 == 29)) {
                    leap = true;
                }
                days / if leap { 366.0 } else { 365.0 }
            } else {
                let years = (b.0 - a.0 + 1) as f64;
                let start = ymd_to_serial(a.0, 1, 1, ev.d1904()).unwrap_or(s);
                let end = ymd_to_serial(b.0 + 1, 1, 1, ev.d1904()).unwrap_or(e);
                days / ((end - start) / years)
            }
        }
        2 => (e - s) / 360.0,
        3 => (e - s) / 365.0,
        4 => days360(a, b, true) / 360.0,
        _ => return Err(XlErr::Num),
    })
}

/// Weekend days: [Mon..Sun] true = weekend.
fn weekend_mask(ev: &mut Eval, a: &[Expr], idx: usize, intl: bool) -> Result<[bool; 7], XlErr> {
    let mut m = [false, false, false, false, false, true, true];
    if !intl || is_missing(a, idx) {
        return Ok(m);
    }
    let v = ev.eval_scalar(&a[idx]);
    match v {
        Value::Str(s) => {
            if s.len() != 7 || !s.chars().all(|c| c == '0' || c == '1') || &*s == "1111111" {
                return Err(XlErr::Value);
            }
            for (i, c) in s.chars().enumerate() {
                m[i] = c == '1';
            }
        }
        v => {
            let n = vnum(ev, &v)?.trunc() as i64;
            m = [false; 7];
            match n {
                1..=7 => {
                    // 1 Sat+Sun, 2 Sun+Mon, ... 7 Fri+Sat
                    let first = (n + 4) % 7; // index of the first weekend day (Mon = 0)
                    m[first as usize] = true;
                    m[((first + 1) % 7) as usize] = true;
                }
                11..=17 => m[((n - 11 + 6) % 7) as usize] = true,
                _ => return Err(XlErr::Num),
            }
        }
    }
    Ok(m)
}

fn holidays(ev: &mut Eval, a: &[Expr], idx: usize) -> Result<Vec<f64>, XlErr> {
    if is_missing(a, idx) {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for v in values_of(ev, &a[idx]) {
        match v {
            Value::Num(n) => out.push(n.floor()),
            Value::Err(e) => return Err(e),
            Value::Empty => {}
            _ => return Err(XlErr::Value),
        }
    }
    Ok(out)
}

fn is_workday(ev: &Eval, x: f64, mask: &[bool; 7], hol: &[f64]) -> bool {
    let mon0 = ((weekday(x, ev.d1904()) as usize) + 5) % 7;
    !mask[mon0] && !hol.contains(&x)
}

fn networkdays(ev: &mut Eval, a: &[Expr], intl: bool) -> Res {
    let s = tri!(num(ev, &a[0])).floor();
    let e = tri!(num(ev, &a[1])).floor();
    let mask = tri!(weekend_mask(ev, a, 2, intl));
    let hol = tri!(holidays(ev, a, if intl { 3 } else { 2 }));
    let (lo, hi, sign) = if s <= e { (s, e, 1.0) } else { (e, s, -1.0) };
    let mut n = 0.0;
    let mut x = lo;
    while x <= hi {
        if is_workday(ev, x, &mask, &hol) {
            n += 1.0;
        }
        x += 1.0;
    }
    Res::num(n * sign)
}

fn workday(ev: &mut Eval, a: &[Expr], intl: bool) -> Res {
    let s = tri!(num(ev, &a[0])).floor();
    let days = tri!(num(ev, &a[1])).trunc();
    let mask = tri!(weekend_mask(ev, a, 2, intl));
    let hol = tri!(holidays(ev, a, if intl { 3 } else { 2 }));
    let step = if days >= 0.0 { 1.0 } else { -1.0 };
    let mut left = days.abs();
    let mut x = s;
    while left > 0.0 {
        x += step;
        if is_workday(ev, x, &mask, &hol) {
            left -= 1.0;
        }
        if x < 0.0 || x > 2958465.0 {
            return Res::err(XlErr::Num);
        }
    }
    Res::num(x)
}
