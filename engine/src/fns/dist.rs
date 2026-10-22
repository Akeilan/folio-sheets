//! Special functions and probability distributions.

use super::*;
use crate::calc::{Eval, Res};
use crate::value::{Value, XlErr};

// --------------------------------------------------------------------------- special functions

const LANCZOS: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.520_368_121_885_1,
    -1_259.139_216_722_402_8,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507_343_278_686_905,
    -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6,
    1.505_632_735_149_311_6e-7,
];

pub fn ln_gamma(x: f64) -> f64 {
    if x < 0.5 {
        // reflection
        let s = (std::f64::consts::PI * x).sin().abs();
        return (std::f64::consts::PI / s).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = LANCZOS[0];
    let t = x + 7.5;
    for (i, c) in LANCZOS.iter().enumerate().skip(1) {
        a += c / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

pub fn gamma(x: f64) -> f64 {
    if x < 0.5 {
        return std::f64::consts::PI / ((std::f64::consts::PI * x).sin() * gamma(1.0 - x));
    }
    if x == x.floor() && x < 171.0 {
        return super::math::fact(x - 1.0);
    }
    ln_gamma(x).exp()
}

/// Regularized lower incomplete gamma P(a, x).
pub fn gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        // series
        let mut sum = 1.0 / a;
        let mut del = sum;
        let mut ap = a;
        for _ in 0..10000 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-16 {
                break;
            }
        }
        (sum.ln() - x + a * x.ln() - ln_gamma(a)).exp()
    } else {
        1.0 - gamma_q_cf(a, x)
    }
}

pub fn gamma_q(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 1.0;
    }
    if x < a + 1.0 {
        1.0 - gamma_p(a, x)
    } else {
        gamma_q_cf(a, x)
    }
}

fn gamma_q_cf(a: f64, x: f64) -> f64 {
    let tiny = 1e-300;
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / tiny;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..10000 {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < tiny {
            d = tiny;
        }
        c = b + an / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    (-x + a * x.ln() - ln_gamma(a)).exp() * h
}

/// Regularized incomplete beta I_x(a, b).
pub fn beta_i(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let lbt = ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln();
    let bt = lbt.exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        bt * betacf(x, a, b) / a
    } else {
        1.0 - bt * betacf(1.0 - x, b, a) / b
    }
}

fn betacf(x: f64, a: f64, b: f64) -> f64 {
    let tiny = 1e-300;
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < tiny {
        d = tiny;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..10000 {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < tiny {
            d = tiny;
        }
        c = 1.0 + aa / c;
        if c.abs() < tiny {
            c = tiny;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < 1e-16 {
            break;
        }
    }
    h
}

pub fn erf(x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let v = gamma_p(0.5, x * x);
    if x < 0.0 { -v } else { v }
}

pub fn erfc(x: f64) -> f64 {
    if x < 0.0 {
        2.0 - erfc(-x)
    } else {
        gamma_q(0.5, x * x)
    }
}

pub fn norm_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / std::f64::consts::SQRT_2)
}

pub fn norm_pdf(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

/// Inverse of the standard normal distribution (Wichura, algorithm AS 241).
pub fn norm_inv(p: f64) -> f64 {
    let q = p - 0.5;
    if q.abs() <= 0.425 {
        let r = 0.180625 - q * q;
        return q * (((((((2509.0809287301226727 * r + 33430.575583588128105) * r + 67265.770927008700853) * r
            + 45921.953931549871457) * r + 13731.693765509461125) * r + 1971.5909503065514427) * r
            + 133.14166789178437745) * r + 3.387132872796366608)
            / (((((((5226.495278852545925 * r + 28729.085735721942674) * r + 39307.89580009271061) * r
                + 21213.794301586595867) * r + 5394.1960214247511077) * r + 687.1870074920579083) * r
                + 42.313330701600911252) * r + 1.0);
    }
    let mut r = if q < 0.0 { p } else { 1.0 - p };
    r = (-r.ln()).sqrt();
    let val = if r <= 5.0 {
        let r = r - 1.6;
        (((((((7.7454501427834140764e-4 * r + 0.0227238449892691845833) * r + 0.24178072517745061177) * r
            + 1.27045825245236838258) * r + 3.64784832476320460504) * r + 5.7694972214606914055) * r
            + 4.6303378461565452959) * r + 1.42343711074968357734)
            / (((((((1.05075007164441684324e-9 * r + 5.475938084995344946e-4) * r + 0.0151986665636164571966) * r
                + 0.14810397642748007459) * r + 0.68976733498510000455) * r + 1.6763848301838038494) * r
                + 2.05319162663775882187) * r + 1.0)
    } else {
        let r = r - 5.0;
        (((((((2.01033439929228813265e-7 * r + 2.71155556874348757815e-5) * r + 0.0012426609473880784386) * r
            + 0.026532189526576123093) * r + 0.29656057182850489123) * r + 1.7848265399172913358) * r
            + 5.4637849111641143699) * r + 6.6579046435011037772)
            / (((((((2.04426310338993978564e-15 * r + 1.4215117583164458887e-7) * r + 1.8463183175100546818e-5) * r
                + 7.868691311456132591e-4) * r + 0.0148753612908506148525) * r + 0.13692988092273580531) * r
                + 0.59983220655588793769) * r + 1.0)
    };
    if q < 0.0 { -val } else { val }
}

pub fn t_cdf(t: f64, df: f64) -> f64 {
    let x = df / (df + t * t);
    let p = 0.5 * beta_i(x, df / 2.0, 0.5);
    if t > 0.0 { 1.0 - p } else { p }
}

pub fn chisq_cdf(x: f64, k: f64) -> f64 {
    gamma_p(k / 2.0, x / 2.0)
}

pub fn f_cdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    beta_i(d1 * x / (d1 * x + d2), d1 / 2.0, d2 / 2.0)
}

/// Solve cdf(x) = p for an increasing cdf on (lo, hi) by bisection then refinement.
pub fn invert(p: f64, mut lo: f64, mut hi: f64, cdf: impl Fn(f64) -> f64) -> f64 {
    // widen the upper bound if needed
    let mut k = 0;
    while cdf(hi) < p && k < 2000 {
        lo = hi;
        hi *= 2.0;
        k += 1;
    }
    for _ in 0..300 {
        let mid = 0.5 * (lo + hi);
        if cdf(mid) < p {
            lo = mid;
        } else {
            hi = mid;
        }
        if (hi - lo).abs() <= 1e-15 * hi.abs().max(1e-300) {
            break;
        }
    }
    0.5 * (lo + hi)
}

fn binom_pmf(k: f64, n: f64, p: f64) -> f64 {
    if p == 0.0 {
        return if k == 0.0 { 1.0 } else { 0.0 };
    }
    if p == 1.0 {
        return if k == n { 1.0 } else { 0.0 };
    }
    (ln_gamma(n + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n - k + 1.0) + k * p.ln() + (n - k) * (1.0 - p).ln()).exp()
}

fn binom_cdf(k: f64, n: f64, p: f64) -> f64 {
    if k >= n {
        return 1.0;
    }
    if p == 0.0 {
        return 1.0;
    }
    if p == 1.0 {
        return 0.0;
    }
    // I_{1-p}(n-k, k+1)
    beta_i(1.0 - p, n - k, k + 1.0)
}

fn poisson_cdf(k: f64, l: f64) -> f64 {
    gamma_q(k + 1.0, l)
}

fn hyp_pmf(k: f64, n: f64, kk: f64, nn: f64) -> f64 {
    let lc = |a: f64, b: f64| ln_gamma(a + 1.0) - ln_gamma(b + 1.0) - ln_gamma(a - b + 1.0);
    (lc(kk, k) + lc(nn - kk, n - k) - lc(nn, n)).exp()
}

// --------------------------------------------------------------------------- functions

fn nums(ev: &mut Eval, v: &[Value]) -> Result<Vec<f64>, XlErr> {
    v.iter().map(|x| vnum(ev, x)).collect()
}

/// A distribution function of numeric arguments (the last may be a logical "cumulative").
fn distf(ev: &mut Eval, a: &[Expr], f: fn(&[f64]) -> Result<f64, XlErr>) -> Res {
    lift(ev, a, move |ev, v| match nums(ev, v) {
        Ok(x) => match f(&x) {
            Ok(r) => Value::num(r),
            Err(e) => Value::Err(e),
        },
        Err(e) => Value::Err(e),
    })
}

fn num_e() -> XlErr {
    XlErr::Num
}

pub static DEFS: &[Def] = &[
    d!("NORM.S.DIST", 2, 2, |ev, a| distf(ev, a, |x| Ok(if x[1] != 0.0 { norm_cdf(x[0]) } else { norm_pdf(x[0]) }))),
    d!("NORMSDIST", 1, 1, |ev, a| distf(ev, a, |x| Ok(norm_cdf(x[0])))),
    d!("NORM.S.INV", 1, 1, |ev, a| distf(ev, a, |x| if x[0] <= 0.0 || x[0] >= 1.0 { Err(num_e()) } else { Ok(norm_inv(x[0])) })),
    d!("NORMSINV", 1, 1, |ev, a| distf(ev, a, |x| if x[0] <= 0.0 || x[0] >= 1.0 { Err(num_e()) } else { Ok(norm_inv(x[0])) })),
    d!("NORM.DIST", 4, 4, |ev, a| distf(ev, a, normdist)),
    d!("NORMDIST", 4, 4, |ev, a| distf(ev, a, normdist)),
    d!("NORM.INV", 3, 3, |ev, a| distf(ev, a, norminv)),
    d!("NORMINV", 3, 3, |ev, a| distf(ev, a, norminv)),
    d!("PHI", 1, 1, |ev, a| distf(ev, a, |x| Ok(norm_pdf(x[0])))),
    d!("GAUSS", 1, 1, |ev, a| distf(ev, a, |x| Ok(norm_cdf(x[0]) - 0.5))),
    d!("ERF", 1, 2, |ev, a| distf(ev, a, |x| Ok(if x.len() > 1 { erf(x[1]) - erf(x[0]) } else { erf(x[0]) }))),
    d!("ERF.PRECISE", 1, 1, |ev, a| distf(ev, a, |x| Ok(erf(x[0])))),
    d!("ERFC", 1, 1, |ev, a| distf(ev, a, |x| Ok(erfc(x[0])))),
    d!("ERFC.PRECISE", 1, 1, |ev, a| distf(ev, a, |x| Ok(erfc(x[0])))),
    d!("LOGNORM.DIST", 4, 4, |ev, a| distf(ev, a, |x| {
        let (v, m, s) = (x[0], x[1], x[2]);
        if v <= 0.0 || s <= 0.0 { return Err(num_e()); }
        let z = (v.ln() - m) / s;
        Ok(if x[3] != 0.0 { norm_cdf(z) } else { norm_pdf(z) / (v * s) })
    })),
    d!("LOGNORMDIST", 3, 3, |ev, a| distf(ev, a, |x| {
        if x[0] <= 0.0 || x[2] <= 0.0 { return Err(num_e()); }
        Ok(norm_cdf((x[0].ln() - x[1]) / x[2]))
    })),
    d!("LOGNORM.INV", 3, 3, |ev, a| distf(ev, a, loginv)),
    d!("LOGINV", 3, 3, |ev, a| distf(ev, a, loginv)),
    d!("EXPON.DIST", 3, 3, |ev, a| distf(ev, a, expon)),
    d!("EXPONDIST", 3, 3, |ev, a| distf(ev, a, expon)),
    d!("WEIBULL", 4, 4, |ev, a| distf(ev, a, weibull)),
    d!("WEIBULL.DIST", 4, 4, |ev, a| distf(ev, a, weibull)),
    d!("GAMMA.DIST", 4, 4, |ev, a| distf(ev, a, gammadist)),
    d!("GAMMADIST", 4, 4, |ev, a| distf(ev, a, gammadist)),
    d!("GAMMA.INV", 3, 3, |ev, a| distf(ev, a, gammainv)),
    d!("GAMMAINV", 3, 3, |ev, a| distf(ev, a, gammainv)),
    d!("BETA.DIST", 4, 6, |ev, a| distf(ev, a, |x| {
        let (v, al, be) = (x[0], x[1], x[2]);
        let cum = x[3] != 0.0;
        let (lo, hi) = (*x.get(4).unwrap_or(&0.0), *x.get(5).unwrap_or(&1.0));
        if al <= 0.0 || be <= 0.0 || v < lo || v > hi || lo >= hi { return Err(num_e()); }
        let y = (v - lo) / (hi - lo);
        if cum { Ok(beta_i(y, al, be)) } else {
            Ok(((al - 1.0) * y.ln() + (be - 1.0) * (1.0 - y).ln() - (ln_gamma(al) + ln_gamma(be) - ln_gamma(al + be))).exp() / (hi - lo))
        }
    })),
    d!("BETADIST", 3, 5, |ev, a| distf(ev, a, |x| {
        let (v, al, be) = (x[0], x[1], x[2]);
        let (lo, hi) = (*x.get(3).unwrap_or(&0.0), *x.get(4).unwrap_or(&1.0));
        if al <= 0.0 || be <= 0.0 || v < lo || v > hi || lo >= hi { return Err(num_e()); }
        Ok(beta_i((v - lo) / (hi - lo), al, be))
    })),
    d!("BETA.INV", 3, 5, |ev, a| distf(ev, a, betainv)),
    d!("BETAINV", 3, 5, |ev, a| distf(ev, a, betainv)),
    d!("BINOM.DIST", 4, 4, |ev, a| distf(ev, a, binomdist)),
    d!("BINOMDIST", 4, 4, |ev, a| distf(ev, a, binomdist)),
    d!("BINOM.DIST.RANGE", 3, 4, |ev, a| distf(ev, a, |x| {
        let (n, p, s) = (x[0].trunc(), x[1], x[2].trunc());
        let s2 = x.get(3).map(|v| v.trunc()).unwrap_or(s);
        if p < 0.0 || p > 1.0 || s < 0.0 || s > n || s2 < s || s2 > n { return Err(num_e()); }
        let mut t = 0.0; let mut k = s;
        while k <= s2 { t += binom_pmf(k, n, p); k += 1.0; }
        Ok(t)
    })),
    d!("BINOM.INV", 3, 3, |ev, a| distf(ev, a, critbinom)),
    d!("CRITBINOM", 3, 3, |ev, a| distf(ev, a, critbinom)),
    d!("NEGBINOM.DIST", 4, 4, |ev, a| distf(ev, a, |x| {
        let (f, s, p) = (x[0].trunc(), x[1].trunc(), x[2]);
        if p < 0.0 || p > 1.0 || f < 0.0 || s < 1.0 { return Err(num_e()); }
        if x[3] != 0.0 { Ok(beta_i(p, s, f + 1.0)) } else {
            Ok((ln_gamma(f + s) - ln_gamma(f + 1.0) - ln_gamma(s) + s * p.ln() + f * (1.0 - p).ln()).exp())
        }
    })),
    d!("NEGBINOMDIST", 3, 3, |ev, a| distf(ev, a, |x| {
        let (f, s, p) = (x[0].trunc(), x[1].trunc(), x[2]);
        if p < 0.0 || p > 1.0 || f < 0.0 || s < 1.0 { return Err(num_e()); }
        Ok((ln_gamma(f + s) - ln_gamma(f + 1.0) - ln_gamma(s) + s * p.ln() + f * (1.0 - p).ln()).exp())
    })),
    d!("POISSON", 3, 3, |ev, a| distf(ev, a, poisson)),
    d!("POISSON.DIST", 3, 3, |ev, a| distf(ev, a, poisson)),
    d!("HYPGEOM.DIST", 5, 5, |ev, a| distf(ev, a, |x| {
        let (k, n, kk, nn) = (x[0].trunc(), x[1].trunc(), x[2].trunc(), x[3].trunc());
        if k < 0.0 || n <= 0.0 || kk <= 0.0 || nn <= 0.0 || n > nn || kk > nn || k > n.min(kk) || k < (n - nn + kk).max(0.0) { return Err(num_e()); }
        if x[4] != 0.0 {
            let mut s = 0.0; let mut i = (n - nn + kk).max(0.0);
            while i <= k { s += hyp_pmf(i, n, kk, nn); i += 1.0; }
            Ok(s)
        } else { Ok(hyp_pmf(k, n, kk, nn)) }
    })),
    d!("HYPGEOMDIST", 4, 4, |ev, a| distf(ev, a, |x| {
        let (k, n, kk, nn) = (x[0].trunc(), x[1].trunc(), x[2].trunc(), x[3].trunc());
        if k < 0.0 || n <= 0.0 || kk <= 0.0 || nn <= 0.0 || n > nn || kk > nn || k > n.min(kk) { return Err(num_e()); }
        Ok(hyp_pmf(k, n, kk, nn))
    })),
    d!("CHISQ.DIST", 3, 3, |ev, a| distf(ev, a, |x| {
        let (v, k) = (x[0], x[1].trunc());
        if v < 0.0 || k < 1.0 { return Err(num_e()); }
        if x[2] != 0.0 { Ok(chisq_cdf(v, k)) } else {
            if v == 0.0 { return Ok(if k == 2.0 { 0.5 } else { 0.0 }); }
            Ok(((k / 2.0 - 1.0) * v.ln() - v / 2.0 - (k / 2.0) * 2f64.ln() - ln_gamma(k / 2.0)).exp())
        }
    })),
    d!("CHISQ.DIST.RT", 2, 2, |ev, a| distf(ev, a, chidist)),
    d!("CHIDIST", 2, 2, |ev, a| distf(ev, a, chidist)),
    d!("CHISQ.INV", 2, 2, |ev, a| distf(ev, a, |x| {
        let (p, k) = (x[0], x[1].trunc());
        if !(0.0..1.0).contains(&p) || k < 1.0 { return Err(num_e()); }
        Ok(invert(p, 0.0, k.max(1.0) * 2.0 + 10.0, |v| chisq_cdf(v, k)))
    })),
    d!("CHISQ.INV.RT", 2, 2, |ev, a| distf(ev, a, chiinv)),
    d!("CHIINV", 2, 2, |ev, a| distf(ev, a, chiinv)),
    d!("T.DIST", 3, 3, |ev, a| distf(ev, a, |x| {
        let (t, df) = (x[0], x[1].trunc());
        if df < 1.0 { return Err(num_e()); }
        if x[2] != 0.0 { Ok(t_cdf(t, df)) } else {
            Ok((ln_gamma((df + 1.0) / 2.0) - ln_gamma(df / 2.0)).exp() / (df * std::f64::consts::PI).sqrt() * (1.0 + t * t / df).powf(-(df + 1.0) / 2.0))
        }
    })),
    d!("T.DIST.2T", 2, 2, |ev, a| distf(ev, a, |x| {
        let (t, df) = (x[0], x[1].trunc());
        if t < 0.0 || df < 1.0 { return Err(num_e()); }
        Ok(2.0 * (1.0 - t_cdf(t, df)))
    })),
    d!("T.DIST.RT", 2, 2, |ev, a| distf(ev, a, |x| {
        let (t, df) = (x[0], x[1].trunc());
        if df < 1.0 { return Err(num_e()); }
        Ok(1.0 - t_cdf(t, df))
    })),
    d!("TDIST", 3, 3, |ev, a| distf(ev, a, |x| {
        let (t, df, tails) = (x[0], x[1].trunc(), x[2].trunc());
        if t < 0.0 || df < 1.0 || !(tails == 1.0 || tails == 2.0) { return Err(num_e()); }
        Ok(tails * (1.0 - t_cdf(t, df)))
    })),
    d!("T.INV", 2, 2, |ev, a| distf(ev, a, |x| {
        let (p, df) = (x[0], x[1].trunc());
        if p <= 0.0 || p >= 1.0 || df < 1.0 { return Err(num_e()); }
        Ok(tinv_left(p, df))
    })),
    d!("T.INV.2T", 2, 2, |ev, a| distf(ev, a, tinv2)),
    d!("TINV", 2, 2, |ev, a| distf(ev, a, tinv2)),
    d!("F.DIST", 4, 4, |ev, a| distf(ev, a, |x| {
        let (v, d1, d2) = (x[0], x[1].trunc(), x[2].trunc());
        if v < 0.0 || d1 < 1.0 || d2 < 1.0 { return Err(num_e()); }
        if x[3] != 0.0 { Ok(f_cdf(v, d1, d2)) } else {
            if v == 0.0 { return Ok(0.0); }
            let lb = ln_gamma(d1 / 2.0) + ln_gamma(d2 / 2.0) - ln_gamma((d1 + d2) / 2.0);
            Ok(((d1 / 2.0) * (d1 / d2).ln() + (d1 / 2.0 - 1.0) * v.ln() - ((d1 + d2) / 2.0) * (1.0 + d1 * v / d2).ln() - lb).exp())
        }
    })),
    d!("F.DIST.RT", 3, 3, |ev, a| distf(ev, a, fdist)),
    d!("FDIST", 3, 3, |ev, a| distf(ev, a, fdist)),
    d!("F.INV", 3, 3, |ev, a| distf(ev, a, |x| {
        let (p, d1, d2) = (x[0], x[1].trunc(), x[2].trunc());
        if !(0.0..1.0).contains(&p) || d1 < 1.0 || d2 < 1.0 { return Err(num_e()); }
        Ok(invert(p, 0.0, 10.0, |v| f_cdf(v, d1, d2)))
    })),
    d!("F.INV.RT", 3, 3, |ev, a| distf(ev, a, finv)),
    d!("FINV", 3, 3, |ev, a| distf(ev, a, finv)),
    d!("CONFIDENCE", 3, 3, |ev, a| distf(ev, a, confnorm)),
    d!("CONFIDENCE.NORM", 3, 3, |ev, a| distf(ev, a, confnorm)),
    d!("CONFIDENCE.T", 3, 3, |ev, a| distf(ev, a, |x| {
        let (al, sd, n) = (x[0], x[1], x[2].trunc());
        if al <= 0.0 || al >= 1.0 || sd <= 0.0 || n < 1.0 { return Err(num_e()); }
        if n == 1.0 { return Err(XlErr::Div0); }
        Ok(tinv2(&[al, n - 1.0])? * sd / n.sqrt())
    })),
    d!("Z.TEST", 2, 3, ztest),
    d!("ZTEST", 2, 3, ztest),
    d!("T.TEST", 4, 4, ttest),
    d!("TTEST", 4, 4, ttest),
    d!("CHISQ.TEST", 2, 2, chitest),
    d!("CHITEST", 2, 2, chitest),
    d!("F.TEST", 2, 2, ftest),
    d!("FTEST", 2, 2, ftest),
    d!("BETA", 2, 2, |ev, a| distf(ev, a, |x| Ok((ln_gamma(x[0]) + ln_gamma(x[1]) - ln_gamma(x[0] + x[1])).exp()))),
];

fn normdist(x: &[f64]) -> Result<f64, XlErr> {
    let (v, m, s) = (x[0], x[1], x[2]);
    if s <= 0.0 {
        return Err(num_e());
    }
    let z = (v - m) / s;
    Ok(if x[3] != 0.0 { norm_cdf(z) } else { norm_pdf(z) / s })
}

fn norminv(x: &[f64]) -> Result<f64, XlErr> {
    let (p, m, s) = (x[0], x[1], x[2]);
    if p <= 0.0 || p >= 1.0 || s <= 0.0 {
        return Err(num_e());
    }
    Ok(m + s * norm_inv(p))
}

fn loginv(x: &[f64]) -> Result<f64, XlErr> {
    let (p, m, s) = (x[0], x[1], x[2]);
    if p <= 0.0 || p >= 1.0 || s <= 0.0 {
        return Err(num_e());
    }
    Ok((m + s * norm_inv(p)).exp())
}

fn expon(x: &[f64]) -> Result<f64, XlErr> {
    let (v, l) = (x[0], x[1]);
    if v < 0.0 || l <= 0.0 {
        return Err(num_e());
    }
    Ok(if x[2] != 0.0 { 1.0 - (-l * v).exp() } else { l * (-l * v).exp() })
}

fn weibull(x: &[f64]) -> Result<f64, XlErr> {
    let (v, al, be) = (x[0], x[1], x[2]);
    if v < 0.0 || al <= 0.0 || be <= 0.0 {
        return Err(num_e());
    }
    Ok(if x[3] != 0.0 { 1.0 - (-(v / be).powf(al)).exp() } else { al / be.powf(al) * v.powf(al - 1.0) * (-(v / be).powf(al)).exp() })
}

fn gammadist(x: &[f64]) -> Result<f64, XlErr> {
    let (v, al, be) = (x[0], x[1], x[2]);
    if v < 0.0 || al <= 0.0 || be <= 0.0 {
        return Err(num_e());
    }
    if x[3] != 0.0 {
        Ok(gamma_p(al, v / be))
    } else {
        if v == 0.0 {
            return Ok(if al == 1.0 { 1.0 / be } else { 0.0 });
        }
        Ok(((al - 1.0) * v.ln() - v / be - al * be.ln() - ln_gamma(al)).exp())
    }
}

fn gammainv(x: &[f64]) -> Result<f64, XlErr> {
    let (p, al, be) = (x[0], x[1], x[2]);
    if !(0.0..1.0).contains(&p) || al <= 0.0 || be <= 0.0 {
        return Err(num_e());
    }
    if p == 0.0 {
        return Ok(0.0);
    }
    Ok(invert(p, 0.0, (al * be).max(1.0) * 10.0, |v| gamma_p(al, v / be)))
}

fn betainv(x: &[f64]) -> Result<f64, XlErr> {
    let (p, al, be) = (x[0], x[1], x[2]);
    let (lo, hi) = (*x.get(3).unwrap_or(&0.0), *x.get(4).unwrap_or(&1.0));
    if p <= 0.0 || p > 1.0 || al <= 0.0 || be <= 0.0 || lo >= hi {
        return Err(num_e());
    }
    let mut a = 0.0;
    let mut b = 1.0;
    for _ in 0..200 {
        let m = 0.5 * (a + b);
        if beta_i(m, al, be) < p {
            a = m;
        } else {
            b = m;
        }
    }
    Ok(lo + 0.5 * (a + b) * (hi - lo))
}

fn binomdist(x: &[f64]) -> Result<f64, XlErr> {
    let (k, n, p) = (x[0].trunc(), x[1].trunc(), x[2]);
    if k < 0.0 || k > n || !(0.0..=1.0).contains(&p) {
        return Err(num_e());
    }
    Ok(if x[3] != 0.0 { binom_cdf(k, n, p) } else { binom_pmf(k, n, p) })
}

fn critbinom(x: &[f64]) -> Result<f64, XlErr> {
    let (n, p, al) = (x[0].trunc(), x[1], x[2]);
    if n < 0.0 || !(0.0..=1.0).contains(&p) || !(0.0..=1.0).contains(&al) {
        return Err(num_e());
    }
    let mut k = 0.0;
    let mut c = 0.0;
    while k <= n {
        c += binom_pmf(k, n, p);
        if c >= al - 1e-12 {
            return Ok(k);
        }
        k += 1.0;
    }
    Ok(n)
}

fn poisson(x: &[f64]) -> Result<f64, XlErr> {
    let (k, l) = (x[0].trunc(), x[1]);
    if k < 0.0 || l < 0.0 {
        return Err(num_e());
    }
    Ok(if x[2] != 0.0 {
        poisson_cdf(k, l)
    } else {
        (k * l.ln() - l - ln_gamma(k + 1.0)).exp()
    })
}

fn chidist(x: &[f64]) -> Result<f64, XlErr> {
    let (v, k) = (x[0], x[1].trunc());
    if v < 0.0 || k < 1.0 {
        return Err(num_e());
    }
    Ok(gamma_q(k / 2.0, v / 2.0))
}

fn chiinv(x: &[f64]) -> Result<f64, XlErr> {
    let (p, k) = (x[0], x[1].trunc());
    if p <= 0.0 || p > 1.0 || k < 1.0 {
        return Err(num_e());
    }
    Ok(invert(1.0 - p, 0.0, k * 2.0 + 10.0, |v| chisq_cdf(v, k)))
}

fn tinv_left(p: f64, df: f64) -> f64 {
    if p == 0.5 {
        return 0.0;
    }
    if p < 0.5 {
        return -tinv_left(1.0 - p, df);
    }
    invert(p, 0.0, 10.0, |t| t_cdf(t, df))
}

fn tinv2(x: &[f64]) -> Result<f64, XlErr> {
    let (p, df) = (x[0], x[1].trunc());
    if p <= 0.0 || p > 1.0 || df < 1.0 {
        return Err(num_e());
    }
    Ok(tinv_left(1.0 - p / 2.0, df))
}

fn fdist(x: &[f64]) -> Result<f64, XlErr> {
    let (v, d1, d2) = (x[0], x[1].trunc(), x[2].trunc());
    if v < 0.0 || d1 < 1.0 || d2 < 1.0 {
        return Err(num_e());
    }
    Ok(1.0 - f_cdf(v, d1, d2))
}

fn finv(x: &[f64]) -> Result<f64, XlErr> {
    let (p, d1, d2) = (x[0], x[1].trunc(), x[2].trunc());
    if p <= 0.0 || p > 1.0 || d1 < 1.0 || d2 < 1.0 {
        return Err(num_e());
    }
    Ok(invert(1.0 - p, 0.0, 10.0, |v| f_cdf(v, d1, d2)))
}

fn confnorm(x: &[f64]) -> Result<f64, XlErr> {
    let (al, sd, n) = (x[0], x[1], x[2].trunc());
    if al <= 0.0 || al >= 1.0 || sd <= 0.0 || n < 1.0 {
        return Err(num_e());
    }
    Ok(norm_inv(1.0 - al / 2.0) * sd / n.sqrt())
}

fn ztest(ev: &mut Eval, a: &[Expr]) -> Res {
    let v = tri!(collect(ev, &a[..1], Mode::Plain));
    let x = tri!(num(ev, &a[1]));
    let n = v.len() as f64;
    if n < 1.0 {
        return Res::err(XlErr::NA);
    }
    let m = super::math::ksum(&v) / n;
    let s = if a.len() > 2 { tri!(num(ev, &a[2])) } else { tri!(super::stats::stdev(&v, true)) };
    if s == 0.0 {
        return Res::err(XlErr::Div0);
    }
    Res::num(1.0 - norm_cdf((m - x) / (s / n.sqrt())))
}

fn ttest(ev: &mut Eval, a: &[Expr]) -> Res {
    let tails = tri!(num(ev, &a[2])).trunc();
    let typ = tri!(num(ev, &a[3])).trunc();
    if !(tails == 1.0 || tails == 2.0) || !(1.0..=3.0).contains(&typ) {
        return Res::err(XlErr::Num);
    }
    let (t, df) = if typ == 1.0 {
        let (x, y) = tri!(super::stats::paired(ev, &a[0], &a[1]));
        let d: Vec<f64> = x.iter().zip(&y).map(|(p, q)| p - q).collect();
        let n = d.len() as f64;
        if n < 2.0 {
            return Res::err(XlErr::Div0);
        }
        let m = super::math::ksum(&d) / n;
        let s = tri!(super::stats::stdev(&d, true));
        if s == 0.0 {
            return Res::err(XlErr::Div0);
        }
        (m / (s / n.sqrt()), n - 1.0)
    } else {
        let x = tri!(collect(ev, &a[..1], Mode::Plain));
        let y = tri!(collect(ev, &a[1..2], Mode::Plain));
        let (n1, n2) = (x.len() as f64, y.len() as f64);
        if n1 < 2.0 || n2 < 2.0 {
            return Res::err(XlErr::Div0);
        }
        let (m1, m2) = (super::math::ksum(&x) / n1, super::math::ksum(&y) / n2);
        let (v1, v2) = (tri!(super::stats::var(&x, true)), tri!(super::stats::var(&y, true)));
        if typ == 2.0 {
            let sp = ((n1 - 1.0) * v1 + (n2 - 1.0) * v2) / (n1 + n2 - 2.0);
            ((m1 - m2) / (sp * (1.0 / n1 + 1.0 / n2)).sqrt(), n1 + n2 - 2.0)
        } else {
            let se = v1 / n1 + v2 / n2;
            let df = se * se / ((v1 / n1).powi(2) / (n1 - 1.0) + (v2 / n2).powi(2) / (n2 - 1.0));
            ((m1 - m2) / se.sqrt(), df)
        }
    };
    let p = 1.0 - t_cdf(t.abs(), df);
    Res::num(p * tails)
}

fn chitest(ev: &mut Eval, a: &[Expr]) -> Res {
    let obs = tri!(array_of(ev, &a[0]));
    let exp = tri!(array_of(ev, &a[1]));
    if obs.rows != exp.rows || obs.cols != exp.cols {
        return Res::err(XlErr::NA);
    }
    let mut chi = 0.0;
    for (o, e) in obs.data.iter().zip(exp.data.iter()) {
        if let (Value::Num(o), Value::Num(e)) = (o, e) {
            if *e == 0.0 {
                return Res::err(XlErr::Div0);
            }
            chi += (o - e).powi(2) / e;
        }
    }
    let df = if obs.rows > 1 && obs.cols > 1 { ((obs.rows - 1) * (obs.cols - 1)) as f64 } else { (obs.rows * obs.cols - 1) as f64 };
    if df < 1.0 {
        return Res::err(XlErr::NA);
    }
    Res::num(gamma_q(df / 2.0, chi / 2.0))
}

fn ftest(ev: &mut Eval, a: &[Expr]) -> Res {
    let x = tri!(collect(ev, &a[..1], Mode::Plain));
    let y = tri!(collect(ev, &a[1..2], Mode::Plain));
    let (v1, v2) = (tri!(super::stats::var(&x, true)), tri!(super::stats::var(&y, true)));
    if v1 == 0.0 || v2 == 0.0 {
        return Res::err(XlErr::Div0);
    }
    let (d1, d2) = (x.len() as f64 - 1.0, y.len() as f64 - 1.0);
    let f = v1 / v2;
    let p = f_cdf(f, d1, d2);
    Res::num(2.0 * p.min(1.0 - p))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn special() {
        assert!((norm_cdf(1.96) - 0.9750021048517795).abs() < 1e-14);
        assert!((norm_inv(0.975) - 1.959963984540054).abs() < 1e-12);
        assert!((gamma(5.0) - 24.0).abs() < 1e-12);
        assert!((erf(0.5) - 0.5204998778130465).abs() < 1e-14);
        assert!((t_cdf(2.0, 10.0) - 0.9633059826146299).abs() < 1e-12);
        assert!((chisq_cdf(3.84, 1.0) - 0.9499565).abs() < 1e-6);
        assert!((beta_i(0.5, 2.0, 3.0) - 0.6875).abs() < 1e-12);
    }
}
