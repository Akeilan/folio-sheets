//! Engineering functions: number bases, bits, complex numbers, CONVERT, Bessel.

use super::*;
use crate::calc::{Eval, Res};
use crate::numfmt::num_to_text;
use crate::value::{Value, XlErr};

fn base_in(base: u32) -> impl Fn(&mut Eval, &[Value]) -> Value {
    move |_, v| match vtext(&v[0]).and_then(|t| parse_base(&t, base)) {
        Ok(n) => Value::num(n as f64),
        Err(e) => Value::Err(e),
    }
}

pub static DEFS: &[Def] = &[
    d!("BIN2DEC", 1, 1, |ev, a| lift(ev, a, base_in(2))),
    d!("OCT2DEC", 1, 1, |ev, a| lift(ev, a, base_in(8))),
    d!("HEX2DEC", 1, 1, |ev, a| lift(ev, a, base_in(16))),
    d!("BIN2OCT", 1, 2, |ev, a| conv(ev, a, 2, 8)),
    d!("BIN2HEX", 1, 2, |ev, a| conv(ev, a, 2, 16)),
    d!("OCT2BIN", 1, 2, |ev, a| conv(ev, a, 8, 2)),
    d!("OCT2HEX", 1, 2, |ev, a| conv(ev, a, 8, 16)),
    d!("HEX2BIN", 1, 2, |ev, a| conv(ev, a, 16, 2)),
    d!("HEX2OCT", 1, 2, |ev, a| conv(ev, a, 16, 8)),
    d!("DEC2BIN", 1, 2, |ev, a| conv(ev, a, 10, 2)),
    d!("DEC2OCT", 1, 2, |ev, a| conv(ev, a, 10, 8)),
    d!("DEC2HEX", 1, 2, |ev, a| conv(ev, a, 10, 16)),
    d!("BITAND", 2, 2, |ev, a| bits(ev, a, |x, y| Ok(x & y))),
    d!("BITOR", 2, 2, |ev, a| bits(ev, a, |x, y| Ok(x | y))),
    d!("BITXOR", 2, 2, |ev, a| bits(ev, a, |x, y| Ok(x ^ y))),
    d!("BITLSHIFT", 2, 2, |ev, a| shift(ev, a, false)),
    d!("BITRSHIFT", 2, 2, |ev, a| shift(ev, a, true)),
    d!("DELTA", 1, 2, |ev, a| {
        let x = tri!(num(ev, &a[0]));
        let y = tri!(opt_num(ev, a, 1, 0.0));
        Res::num(if x == y { 1.0 } else { 0.0 })
    }),
    d!("GESTEP", 1, 2, |ev, a| {
        let x = tri!(num(ev, &a[0]));
        let y = tri!(opt_num(ev, a, 1, 0.0));
        Res::num(if x >= y { 1.0 } else { 0.0 })
    }),
    d!("CONVERT", 3, 3, |ev, a| lift(ev, a, |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let (from, to) = match (vtext(&v[1]), vtext(&v[2])) { (Ok(f), Ok(t)) => (f, t), (Err(e), _) | (_, Err(e)) => return Value::Err(e) };
        match convert(x, &from, &to) { Some(r) => Value::num(r), None => Value::Err(XlErr::NA) }
    })),
    d!("BESSELJ", 2, 2, |ev, a| bessel(ev, a, bessel_j, false)),
    d!("BESSELY", 2, 2, |ev, a| bessel(ev, a, bessel_y, true)),
    d!("BESSELI", 2, 2, |ev, a| bessel(ev, a, bessel_i, false)),
    d!("BESSELK", 2, 2, |ev, a| bessel(ev, a, bessel_k, true)),
    // ---- complex numbers
    d!("COMPLEX", 2, 3, |ev, a| {
        let re = tri!(num(ev, &a[0]));
        let im = tri!(num(ev, &a[1]));
        let suf = tri!(opt_text(ev, a, 2, "i"));
        let suf = match suf.as_str() { "" | "i" => 'i', "j" => 'j', _ => return Res::err(XlErr::Value) };
        Res::V(Value::string(cfmt(C(re, im), suf)))
    }),
    d!("IMREAL", 1, 1, |ev, a| imnum(ev, a, |z| z.0)),
    d!("IMAGINARY", 1, 1, |ev, a| imnum(ev, a, |z| z.1)),
    d!("IMABS", 1, 1, |ev, a| imnum(ev, a, |z| z.abs())),
    d!("IMARGUMENT", 1, 1, |ev, a| {
        let (z, _) = tri!(carg(ev, &a[0]));
        if z.0 == 0.0 && z.1 == 0.0 { return Res::err(XlErr::Div0); }
        Res::num(z.1.atan2(z.0))
    }),
    d!("IMCONJUGATE", 1, 1, |ev, a| im1(ev, a, |z| Ok(C(z.0, -z.1)))),
    d!("IMEXP", 1, 1, |ev, a| im1(ev, a, |z| Ok(z.exp()))),
    d!("IMLN", 1, 1, |ev, a| im1(ev, a, |z| z.ln())),
    d!("IMLOG10", 1, 1, |ev, a| im1(ev, a, |z| Ok(z.ln()?.scale(1.0 / std::f64::consts::LN_10)))),
    d!("IMLOG2", 1, 1, |ev, a| im1(ev, a, |z| Ok(z.ln()?.scale(1.0 / std::f64::consts::LN_2)))),
    d!("IMSQRT", 1, 1, |ev, a| im1(ev, a, |z| z.pow(0.5))),
    d!("IMSIN", 1, 1, |ev, a| im1(ev, a, |z| Ok(z.sin()))),
    d!("IMCOS", 1, 1, |ev, a| im1(ev, a, |z| Ok(z.cos()))),
    d!("IMTAN", 1, 1, |ev, a| im1(ev, a, |z| z.sin().div(z.cos()))),
    d!("IMCOT", 1, 1, |ev, a| im1(ev, a, |z| z.cos().div(z.sin()))),
    d!("IMSEC", 1, 1, |ev, a| im1(ev, a, |z| C(1.0, 0.0).div(z.cos()))),
    d!("IMCSC", 1, 1, |ev, a| im1(ev, a, |z| C(1.0, 0.0).div(z.sin()))),
    d!("IMSINH", 1, 1, |ev, a| im1(ev, a, |z| Ok(z.sinh()))),
    d!("IMCOSH", 1, 1, |ev, a| im1(ev, a, |z| Ok(z.cosh()))),
    d!("IMSECH", 1, 1, |ev, a| im1(ev, a, |z| C(1.0, 0.0).div(z.cosh()))),
    d!("IMCSCH", 1, 1, |ev, a| im1(ev, a, |z| C(1.0, 0.0).div(z.sinh()))),
    d!("IMPOWER", 2, 2, |ev, a| {
        let (z, s) = tri!(carg(ev, &a[0]));
        let p = tri!(num(ev, &a[1]));
        Res::V(Value::string(cfmt(tri!(z.pow(p)), s)))
    }),
    d!("IMDIV", 2, 2, |ev, a| im2(ev, a, |x, y| x.div(y))),
    d!("IMSUB", 2, 2, |ev, a| im2(ev, a, |x, y| Ok(C(x.0 - y.0, x.1 - y.1)))),
    d!("IMSUM", 1, MANY, |ev, a| imfold(ev, a, C(0.0, 0.0), |x, y| C(x.0 + y.0, x.1 + y.1))),
    d!("IMPRODUCT", 1, MANY, |ev, a| imfold(ev, a, C(1.0, 0.0), |x, y| x.mul(y))),
];

// --------------------------------------------------------------------------- bases

fn bits_of(base: u32) -> u32 {
    match base {
        2 => 10,
        8 => 30,
        _ => 40,
    }
}

fn parse_base(t: &str, base: u32) -> Result<i64, XlErr> {
    let t = t.trim();
    if t.len() > 10 {
        return Err(XlErr::Num);
    }
    if t.is_empty() {
        return Ok(0);
    }
    let n = i64::from_str_radix(t, base).map_err(|_| XlErr::Num)?;
    let bits = bits_of(base);
    // ten digits with the top bit set: two's complement negative
    if t.len() == 10 && n >= 1i64 << (bits - 1) {
        return Ok(n - (1i64 << bits));
    }
    Ok(n)
}

fn to_base(n: i64, base: u32, places: Option<i64>) -> Result<String, XlErr> {
    let bits = bits_of(base);
    let lim = 1i64 << (bits - 1);
    if n < -lim || n >= lim {
        return Err(XlErr::Num);
    }
    if n < 0 {
        let u = (n + (1i64 << bits)) as u64;
        return Ok(radix(u, base).to_uppercase());
    }
    let s = radix(n as u64, base).to_uppercase();
    match places {
        None => Ok(s),
        Some(p) => {
            if p < 0 || (p as usize) < s.len() || p > 10 {
                if p > 10 && (s.len() as i64) <= p {
                    return Err(XlErr::Num);
                }
                return Err(XlErr::Num);
            }
            Ok(format!("{:0>w$}", s, w = p as usize))
        }
    }
}

fn radix(mut n: u64, base: u32) -> String {
    if n == 0 {
        return "0".into();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(std::char::from_digit((n % base as u64) as u32, base).unwrap());
        n /= base as u64;
    }
    out.iter().rev().collect()
}

fn conv(ev: &mut Eval, a: &[Expr], from: u32, to: u32) -> Res {
    let has_places = !is_missing(a, 1);
    let vals: Vec<Value> = (0..2).map(|i| if is_missing(a, i) { Value::Empty } else { ev.eval_value(&a[i]) }).collect();
    Res::V(lift_values(ev, &vals, move |ev, v| {
        let n = if from == 10 {
            match vnum(ev, &v[0]) {
                Ok(x) => {
                    let x = x.trunc();
                    if x.abs() > 1e12 {
                        return Value::Err(XlErr::Num);
                    }
                    x as i64
                }
                Err(e) => return Value::Err(e),
            }
        } else {
            match vtext(&v[0]).and_then(|t| parse_base(&t, from)) {
                Ok(n) => n,
                Err(e) => return Value::Err(e),
            }
        };
        let places = if has_places {
            match vnum(ev, &v[1]) {
                Ok(p) => Some(p.trunc() as i64),
                Err(e) => return Value::Err(e),
            }
        } else {
            None
        };
        match to_base(n, to, places) {
            Ok(s) => Value::string(s),
            Err(e) => Value::Err(e),
        }
    }))
}

const BIT_MAX: f64 = 281474976710655.0; // 2^48 - 1

fn bitnum(ev: &Eval, v: &Value) -> Result<u64, XlErr> {
    let x = vnum(ev, v)?;
    if x < 0.0 || x > BIT_MAX || x.fract() != 0.0 {
        return Err(XlErr::Num);
    }
    Ok(x as u64)
}

fn bits(ev: &mut Eval, a: &[Expr], f: fn(u64, u64) -> Result<u64, XlErr>) -> Res {
    lift(ev, a, move |ev, v| match (bitnum(ev, &v[0]), bitnum(ev, &v[1])) {
        (Ok(x), Ok(y)) => match f(x, y) {
            Ok(r) => Value::num(r as f64),
            Err(e) => Value::Err(e),
        },
        (Err(e), _) | (_, Err(e)) => Value::Err(e),
    })
}

fn shift(ev: &mut Eval, a: &[Expr], right: bool) -> Res {
    lift(ev, a, move |ev, v| {
        let x = match bitnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let s = match vnum(ev, &v[1]) { Ok(s) => s.trunc() as i64, Err(e) => return Value::Err(e) };
        if s.abs() > 53 {
            return Value::Err(XlErr::Num);
        }
        let s = if right { -s } else { s };
        let r = if s >= 0 { (x as u128) << s } else { (x >> (-s)) as u128 };
        if r as f64 > BIT_MAX {
            return Value::Err(XlErr::Num);
        }
        Value::num(r as f64)
    })
}

// --------------------------------------------------------------------------- complex

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct C(pub f64, pub f64);

impl C {
    fn abs(self) -> f64 {
        self.0.hypot(self.1)
    }
    fn scale(self, k: f64) -> C {
        C(self.0 * k, self.1 * k)
    }
    fn mul(self, o: C) -> C {
        C(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn div(self, o: C) -> Result<C, XlErr> {
        let d = o.0 * o.0 + o.1 * o.1;
        if d == 0.0 {
            return Err(XlErr::Num);
        }
        Ok(C((self.0 * o.0 + self.1 * o.1) / d, (self.1 * o.0 - self.0 * o.1) / d))
    }
    fn exp(self) -> C {
        let m = self.0.exp();
        C(m * self.1.cos(), m * self.1.sin())
    }
    fn ln(self) -> Result<C, XlErr> {
        if self.0 == 0.0 && self.1 == 0.0 {
            return Err(XlErr::Num);
        }
        Ok(C(self.abs().ln(), self.1.atan2(self.0)))
    }
    fn pow(self, p: f64) -> Result<C, XlErr> {
        if self.0 == 0.0 && self.1 == 0.0 {
            return if p > 0.0 { Ok(C(0.0, 0.0)) } else { Err(XlErr::Num) };
        }
        let r = self.abs().powf(p);
        let t = self.1.atan2(self.0) * p;
        Ok(C(r * t.cos(), r * t.sin()))
    }
    fn sin(self) -> C {
        C(self.0.sin() * self.1.cosh(), self.0.cos() * self.1.sinh())
    }
    fn cos(self) -> C {
        C(self.0.cos() * self.1.cosh(), -self.0.sin() * self.1.sinh())
    }
    fn sinh(self) -> C {
        C(self.0.sinh() * self.1.cos(), self.0.cosh() * self.1.sin())
    }
    fn cosh(self) -> C {
        C(self.0.cosh() * self.1.cos(), self.0.sinh() * self.1.sin())
    }
}

/// "3+4i", "-2.5j", "i", "1e3-i", "7". The suffix is None for a pure real.
pub fn cparse(s: &str) -> Result<(C, Option<char>), XlErr> {
    let s = s.trim();
    if s.is_empty() {
        return Ok((C(0.0, 0.0), None));
    }
    let suf = s.chars().last().unwrap();
    if suf != 'i' && suf != 'j' {
        let x: f64 = s.parse().map_err(|_| XlErr::Num)?;
        if s.contains(|c: char| c.is_ascii_alphabetic() && c != 'e' && c != 'E') {
            return Err(XlErr::Num);
        }
        return Ok((C(x, 0.0), None));
    }
    let body = &s[..s.len() - 1];
    // split where the imaginary part starts: the last + or - not after an 'e'
    let b = body.as_bytes();
    let mut cut = 0;
    for i in (1..b.len()).rev() {
        if (b[i] == b'+' || b[i] == b'-') && !(b[i - 1] == b'e' || b[i - 1] == b'E') {
            cut = i;
            break;
        }
    }
    let (re_s, im_s) = body.split_at(cut);
    let re = if re_s.is_empty() { 0.0 } else { re_s.parse::<f64>().map_err(|_| XlErr::Num)? };
    let im = match im_s {
        "" | "+" => 1.0,
        "-" => -1.0,
        t => t.parse::<f64>().map_err(|_| XlErr::Num)?,
    };
    if !re_s.is_empty() && cut == 0 {
        return Err(XlErr::Num);
    }
    Ok((C(re, im), Some(suf)))
}

pub fn cfmt(z: C, suf: char) -> String {
    let clean = |x: f64| if x.abs() < 1e-300 { 0.0 } else { x };
    let (re, im) = (clean(z.0), clean(z.1));
    if im == 0.0 {
        return num_to_text(re);
    }
    let imtxt = match im {
        x if x == 1.0 => String::new(),
        x if x == -1.0 => "-".into(),
        x => num_to_text(x),
    };
    if re == 0.0 {
        return format!("{}{}", imtxt, suf);
    }
    let sign = if im > 0.0 { "+" } else { "" };
    format!("{}{}{}{}", num_to_text(re), sign, imtxt, suf)
}

fn cval(v: &Value) -> Result<(C, Option<char>), XlErr> {
    match v {
        Value::Num(n) => Ok((C(*n, 0.0), None)),
        Value::Str(s) => cparse(s),
        Value::Empty => Ok((C(0.0, 0.0), None)),
        Value::Bool(_) => Err(XlErr::Value),
        Value::Err(e) => Err(*e),
        Value::Arr(_) => Err(XlErr::Value),
    }
}

fn carg(ev: &mut Eval, e: &Expr) -> Result<(C, char), XlErr> {
    let v = ev.eval_scalar(e);
    let (z, s) = cval(&v)?;
    Ok((z, s.unwrap_or('i')))
}

fn imnum(ev: &mut Eval, a: &[Expr], f: fn(C) -> f64) -> Res {
    lift(ev, a, move |_, v| match cval(&v[0]) {
        Ok((z, _)) => Value::num(f(z)),
        Err(e) => Value::Err(e),
    })
}

fn im1(ev: &mut Eval, a: &[Expr], f: fn(C) -> Result<C, XlErr>) -> Res {
    lift(ev, a, move |_, v| match cval(&v[0]).and_then(|(z, s)| Ok((f(z)?, s))) {
        Ok((r, s)) => {
            if !r.0.is_finite() || !r.1.is_finite() {
                return Value::Err(XlErr::Num);
            }
            Value::string(cfmt(r, s.unwrap_or('i')))
        }
        Err(e) => Value::Err(e),
    })
}

fn suffix(a: Option<char>, b: Option<char>) -> Result<char, XlErr> {
    match (a, b) {
        (Some(x), Some(y)) if x != y => Err(XlErr::Value),
        (Some(x), _) | (_, Some(x)) => Ok(x),
        _ => Ok('i'),
    }
}

fn im2(ev: &mut Eval, a: &[Expr], f: fn(C, C) -> Result<C, XlErr>) -> Res {
    lift(ev, a, move |_, v| {
        let r = (|| {
            let (x, s1) = cval(&v[0])?;
            let (y, s2) = cval(&v[1])?;
            let s = suffix(s1, s2)?;
            Ok::<_, XlErr>((f(x, y)?, s))
        })();
        match r {
            Ok((z, s)) => Value::string(cfmt(z, s)),
            Err(e) => Value::Err(e),
        }
    })
}

fn imfold(ev: &mut Eval, a: &[Expr], init: C, f: fn(C, C) -> C) -> Res {
    let mut acc = init;
    let mut suf: Option<char> = None;
    for e in a {
        for v in values_of(ev, e) {
            if matches!(v, Value::Empty) {
                continue;
            }
            let (z, s) = tri!(cval(&v));
            suf = Some(tri!(suffix(suf, s)));
            acc = f(acc, z);
        }
    }
    Res::V(Value::string(cfmt(acc, suf.unwrap_or('i'))))
}

// --------------------------------------------------------------------------- CONVERT

enum Cat {
    Mass,
    Dist,
    Time,
    Press,
    Force,
    Energy,
    Power,
    Magn,
    Temp,
    Vol,
    Area,
    Info,
    Speed,
}

/// (names, category, factor to the category's base unit, accepts prefixes, power for prefixes)
const UNITS: &[(&[&str], Cat, f64, bool, i32)] = &[
    (&["g"], Cat::Mass, 1.0, true, 1),
    (&["sg"], Cat::Mass, 14593.9029372064, false, 1),
    (&["lbm"], Cat::Mass, 453.59237, false, 1),
    (&["u"], Cat::Mass, 1.66053886e-24, true, 1),
    (&["ozm"], Cat::Mass, 28.349523125, false, 1),
    (&["grain"], Cat::Mass, 0.06479891, false, 1),
    (&["cwt", "shweight"], Cat::Mass, 45359.237, false, 1),
    (&["uk_cwt", "lcwt", "hweight"], Cat::Mass, 50802.34544, false, 1),
    (&["stone"], Cat::Mass, 6350.29318, false, 1),
    (&["ton"], Cat::Mass, 907184.74, false, 1),
    (&["uk_ton", "LTON", "brton"], Cat::Mass, 1016046.9088, false, 1),
    (&["m"], Cat::Dist, 1.0, true, 1),
    (&["mi"], Cat::Dist, 1609.344, false, 1),
    (&["Nmi"], Cat::Dist, 1852.0, false, 1),
    (&["in"], Cat::Dist, 0.0254, false, 1),
    (&["ft"], Cat::Dist, 0.3048, false, 1),
    (&["yd"], Cat::Dist, 0.9144, false, 1),
    (&["ang"], Cat::Dist, 1e-10, true, 1),
    (&["ell"], Cat::Dist, 1.143, false, 1),
    (&["ly"], Cat::Dist, 9.4607304725808e15, true, 1),
    (&["parsec", "pc"], Cat::Dist, 3.08567758128155e16, true, 1),
    (&["Picapt", "Pica"], Cat::Dist, 0.0254 / 72.0, false, 1),
    (&["pica"], Cat::Dist, 0.0254 / 6.0, false, 1),
    (&["survey_mi"], Cat::Dist, 1609.34721869444, false, 1),
    (&["yr"], Cat::Time, 31557600.0, false, 1),
    (&["day", "d"], Cat::Time, 86400.0, false, 1),
    (&["hr"], Cat::Time, 3600.0, false, 1),
    (&["mn", "min"], Cat::Time, 60.0, false, 1),
    (&["sec", "s"], Cat::Time, 1.0, true, 1),
    (&["Pa", "p"], Cat::Press, 1.0, true, 1),
    (&["atm", "at"], Cat::Press, 101325.0, true, 1),
    (&["mmHg"], Cat::Press, 133.322, true, 1),
    (&["psi"], Cat::Press, 6894.75729316836, false, 1),
    (&["Torr"], Cat::Press, 133.322368421053, false, 1),
    (&["N"], Cat::Force, 1.0, true, 1),
    (&["dyn", "dy"], Cat::Force, 1e-5, true, 1),
    (&["lbf"], Cat::Force, 4.4482216152605, false, 1),
    (&["pond"], Cat::Force, 0.00980665, true, 1),
    (&["J"], Cat::Energy, 1.0, true, 1),
    (&["e"], Cat::Energy, 1e-7, true, 1),
    (&["c"], Cat::Energy, 4.184, true, 1),
    (&["cal"], Cat::Energy, 4.1868, true, 1),
    (&["eV", "ev"], Cat::Energy, 1.602176487e-19, true, 1),
    (&["HPh", "hh"], Cat::Energy, 2684519.53769617, false, 1),
    (&["Wh", "wh"], Cat::Energy, 3600.0, true, 1),
    (&["flb"], Cat::Energy, 1.3558179483314, false, 1),
    (&["BTU", "btu"], Cat::Energy, 1055.05585262, false, 1),
    (&["HP", "h"], Cat::Power, 745.69987158227, false, 1),
    (&["PS"], Cat::Power, 735.49875, false, 1),
    (&["W", "w"], Cat::Power, 1.0, true, 1),
    (&["T"], Cat::Magn, 1.0, true, 1),
    (&["ga"], Cat::Magn, 1e-4, true, 1),
    (&["C", "cel"], Cat::Temp, 0.0, false, 1),
    (&["F", "fah"], Cat::Temp, 1.0, false, 1),
    (&["K", "kel"], Cat::Temp, 2.0, true, 1),
    (&["Rank"], Cat::Temp, 3.0, false, 1),
    (&["Reau"], Cat::Temp, 4.0, false, 1),
    (&["tsp"], Cat::Vol, 4.92892159375e-6, false, 1),
    (&["tspm"], Cat::Vol, 5e-6, false, 1),
    (&["tbs"], Cat::Vol, 1.478676478125e-5, false, 1),
    (&["oz"], Cat::Vol, 2.95735295625e-5, false, 1),
    (&["cup"], Cat::Vol, 2.365882365e-4, false, 1),
    (&["pt", "us_pt"], Cat::Vol, 4.73176473e-4, false, 1),
    (&["uk_pt"], Cat::Vol, 5.6826125e-4, false, 1),
    (&["qt"], Cat::Vol, 9.46352946e-4, false, 1),
    (&["uk_qt"], Cat::Vol, 1.1365225e-3, false, 1),
    (&["gal"], Cat::Vol, 3.785411784e-3, false, 1),
    (&["uk_gal"], Cat::Vol, 4.54609e-3, false, 1),
    (&["l", "L", "lt"], Cat::Vol, 1e-3, true, 1),
    (&["ang3", "ang^3"], Cat::Vol, 1e-30, true, 3),
    (&["barrel"], Cat::Vol, 0.158987294928, false, 1),
    (&["bushel"], Cat::Vol, 0.03523907016688, false, 1),
    (&["ft3", "ft^3"], Cat::Vol, 0.028316846592, false, 1),
    (&["in3", "in^3"], Cat::Vol, 1.6387064e-5, false, 1),
    (&["ly3", "ly^3"], Cat::Vol, 8.46786664623715e47, false, 1),
    (&["m3", "m^3"], Cat::Vol, 1.0, true, 3),
    (&["mi3", "mi^3"], Cat::Vol, 4168181825.44058, false, 1),
    (&["yd3", "yd^3"], Cat::Vol, 0.764554857984, false, 1),
    (&["Nmi3", "Nmi^3"], Cat::Vol, 6352182208.0, false, 1),
    (&["Picapt3", "Picapt^3", "Pica3", "Pica^3"], Cat::Vol, 4.39039566186557e-11, false, 1),
    (&["GRT", "regton"], Cat::Vol, 2.8316846592, false, 1),
    (&["MTON"], Cat::Vol, 1.13267386368, false, 1),
    (&["uk_acre"], Cat::Area, 4046.8564224, false, 1),
    (&["us_acre"], Cat::Area, 4046.87260987425, false, 1),
    (&["ang2", "ang^2"], Cat::Area, 1e-20, true, 2),
    (&["ar"], Cat::Area, 100.0, true, 1),
    (&["ft2", "ft^2"], Cat::Area, 0.09290304, false, 1),
    (&["ha"], Cat::Area, 10000.0, false, 1),
    (&["in2", "in^2"], Cat::Area, 6.4516e-4, false, 1),
    (&["ly2", "ly^2"], Cat::Area, 8.95054210748189e31, false, 1),
    (&["m2", "m^2"], Cat::Area, 1.0, true, 2),
    (&["Morgen"], Cat::Area, 2500.0, false, 1),
    (&["mi2", "mi^2"], Cat::Area, 2589988.110336, false, 1),
    (&["Nmi2", "Nmi^2"], Cat::Area, 3429904.0, false, 1),
    (&["Picapt2", "Picapt^2", "Pica2", "Pica^2"], Cat::Area, 1.24452160493827e-7, false, 1),
    (&["yd2", "yd^2"], Cat::Area, 0.83612736, false, 1),
    (&["bit"], Cat::Info, 1.0, true, 1),
    (&["byte"], Cat::Info, 8.0, true, 1),
    (&["admkn"], Cat::Speed, 0.514773333333333, false, 1),
    (&["kn"], Cat::Speed, 0.514444444444444, false, 1),
    (&["m/h", "m/hr"], Cat::Speed, 1.0 / 3600.0, true, 1),
    (&["m/s", "m/sec"], Cat::Speed, 1.0, true, 1),
    (&["mph"], Cat::Speed, 0.44704, false, 1),
];

const PREFIXES: &[(&str, f64)] = &[
    ("Y", 1e24), ("Z", 1e21), ("E", 1e18), ("P", 1e15), ("T", 1e12), ("G", 1e9), ("M", 1e6), ("k", 1e3), ("h", 1e2),
    ("da", 1e1), ("e", 1e1), ("d", 1e-1), ("c", 1e-2), ("m", 1e-3), ("u", 1e-6), ("n", 1e-9), ("p", 1e-12), ("f", 1e-15),
    ("a", 1e-18), ("z", 1e-21), ("y", 1e-24),
];

const BIN_PREFIXES: &[(&str, f64)] = &[
    ("ki", 1024.0), ("Mi", 1048576.0), ("Gi", 1073741824.0), ("Ti", 1099511627776.0), ("Pi", 1125899906842624.0),
    ("Ei", 1152921504606846976.0), ("Zi", 1180591620717411303424.0), ("Yi", 1208925819614629174706176.0),
];

fn find_unit(name: &str) -> Option<(usize, f64)> {
    for (i, u) in UNITS.iter().enumerate() {
        if u.0.contains(&name) {
            return Some((i, 1.0));
        }
    }
    for list in [BIN_PREFIXES, PREFIXES] {
        for (p, k) in list {
            if let Some(rest) = name.strip_prefix(p) {
                for (i, u) in UNITS.iter().enumerate() {
                    if u.3 && u.0.contains(&rest) {
                        if list.len() == BIN_PREFIXES.len() && !matches!(u.1, Cat::Info) {
                            continue;
                        }
                        return Some((i, k.powi(u.4)));
                    }
                }
            }
        }
    }
    None
}

fn same_cat(a: &Cat, b: &Cat) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

pub fn convert(x: f64, from: &str, to: &str) -> Option<f64> {
    let (fi, fk) = find_unit(from)?;
    let (ti, tk) = find_unit(to)?;
    let (fu, tu) = (&UNITS[fi], &UNITS[ti]);
    if !same_cat(&fu.1, &tu.1) {
        return None;
    }
    if let Cat::Temp = fu.1 {
        let x = x * fk;
        let k = match fu.2 as i32 {
            0 => x + 273.15,
            1 => (x - 32.0) * 5.0 / 9.0 + 273.15,
            2 => x,
            3 => x * 5.0 / 9.0,
            _ => x * 1.25 + 273.15,
        };
        let r = match tu.2 as i32 {
            0 => k - 273.15,
            1 => (k - 273.15) * 9.0 / 5.0 + 32.0,
            2 => k,
            3 => k * 9.0 / 5.0,
            _ => (k - 273.15) * 0.8,
        };
        return Some(r / tk);
    }
    Some(x * fu.2 * fk / (tu.2 * tk))
}

// --------------------------------------------------------------------------- Bessel

fn bessel(ev: &mut Eval, a: &[Expr], f: fn(f64, i32) -> f64, positive: bool) -> Res {
    lift(ev, a, move |ev, v| {
        let x = match vnum(ev, &v[0]) { Ok(x) => x, Err(e) => return Value::Err(e) };
        let n = match vnum(ev, &v[1]) { Ok(n) => n.trunc(), Err(e) => return Value::Err(e) };
        if n < 0.0 || (positive && x <= 0.0) {
            return Value::Err(XlErr::Num);
        }
        let r = f(x, n as i32);
        if r.is_finite() { Value::num(r) } else { Value::Err(XlErr::Num) }
    })
}

/// J_n(x) = 1/2π ∫ cos(nτ − x sin τ) over a full period: the trapezoid rule is
/// spectrally accurate for periodic integrands.
pub fn bessel_j(x: f64, n: i32) -> f64 {
    let m = (64.0 + 2.0 * (x.abs() + n as f64)).min(20000.0) as usize;
    let h = 2.0 * std::f64::consts::PI / m as f64;
    let mut s = 0.0;
    for k in 0..m {
        let t = k as f64 * h;
        s += (n as f64 * t - x * t.sin()).cos();
    }
    s / m as f64
}

pub fn bessel_i(x: f64, n: i32) -> f64 {
    let m = (64.0 + 2.0 * (x.abs() + n as f64)).min(20000.0) as usize;
    let h = 2.0 * std::f64::consts::PI / m as f64;
    let mut s = 0.0;
    for k in 0..m {
        let t = k as f64 * h;
        s += (x * t.cos()).exp() * (n as f64 * t).cos();
    }
    s / m as f64
}

pub fn bessel_k(x: f64, n: i32) -> f64 {
    let h: f64 = 0.02;
    let mut s = 0.5 * (-x).exp();
    let mut t = h;
    loop {
        let v = (-x * t.cosh() + n as f64 * t).exp() * 0.5 * (1.0 + (-2.0 * n as f64 * t).exp());
        s += v;
        if v < 1e-18 * s || t > 50.0 {
            break;
        }
        t += h;
    }
    s * h
}

pub fn bessel_y(x: f64, n: i32) -> f64 {
    use std::f64::consts::PI;
    let nf = n as f64;
    // first part: 1/π ∫_0^π sin(x sin τ − nτ) dτ by Simpson
    let m = (2000.0 + 200.0 * x.abs()).min(400000.0) as usize & !1;
    let h = PI / m as f64;
    let f = |t: f64| (x * t.sin() - nf * t).sin();
    let mut s = f(0.0) + f(PI);
    for k in 1..m {
        s += f(k as f64 * h) * if k % 2 == 1 { 4.0 } else { 2.0 };
    }
    let part1 = s * h / 3.0 / PI;
    // second: 1/π ∫_0^∞ (e^{nt} + (−1)^n e^{−nt}) e^{−x sinh t} dt
    let sign = if n % 2 == 0 { 1.0 } else { -1.0 };
    let g = |t: f64| ((nf * t) - x * t.sinh()).exp() + sign * ((-nf * t) - x * t.sinh()).exp();
    let h2: f64 = 0.0005;
    let mut s2 = 0.5 * g(0.0);
    let mut t = h2;
    loop {
        let v = g(t);
        s2 += v;
        if (v.abs() < 1e-18 * s2.abs().max(1e-300) && t > 1.0) || t > 60.0 {
            break;
        }
        t += h2;
    }
    part1 - s2 * h2 / PI
}

#[cfg(test)]
mod tests {
    use super::*;
    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol * b.abs().max(1.0)
    }
    #[test]
    fn bases() {
        assert_eq!(parse_base("1111111111", 2).unwrap(), -1);
        assert_eq!(parse_base("FFFFFFFF9C", 16).unwrap(), -100);
        assert_eq!(to_base(-1, 2, None).unwrap(), "1111111111");
        assert_eq!(to_base(9, 2, Some(4)).unwrap(), "1001");
        assert!(to_base(9, 2, Some(3)).is_err());
        assert_eq!(to_base(100, 16, Some(4)).unwrap(), "0064");
    }
    #[test]
    fn complex() {
        assert_eq!(cparse("3+4i").unwrap().0, C(3.0, 4.0));
        assert_eq!(cparse("-i").unwrap().0, C(0.0, -1.0));
        assert_eq!(cparse("1e2-2.5j").unwrap(), (C(100.0, -2.5), Some('j')));
        assert_eq!(cfmt(C(1.0, -1.0), 'i'), "1-i");
        assert_eq!(cfmt(C(0.0, 2.0), 'j'), "2j");
        assert_eq!(cfmt(C(5.0, 0.0), 'i'), "5");
    }
    #[test]
    fn convert_units() {
        assert!(close(convert(1.0, "lbm", "kg").unwrap(), 0.45359237, 1e-12));
        assert!(close(convert(68.0, "F", "C").unwrap(), 20.0, 1e-12));
        assert!(close(convert(1.0, "kibyte", "bit").unwrap(), 8192.0, 1e-12));
        assert!(close(convert(1.0, "km2", "m2").unwrap(), 1e6, 1e-12));
        assert!(convert(1.0, "m", "kg").is_none());
    }
    #[test]
    fn bessel_values() {
        assert!(close(bessel_j(1.9, 2), 0.329925829, 1e-6));
        assert!(close(bessel_i(1.5, 1), 0.981666428, 1e-8));
        assert!(close(bessel_k(1.5, 1), 0.277387804, 1e-7));
        assert!(close(bessel_y(2.5, 1), 0.145918138, 1e-6));
    }
}
