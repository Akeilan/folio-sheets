//! FORECAST.ETS and its companions: additive exponential smoothing with trend and seasonality
//! (Holt-Winters "AAA"), the seasonality found from the data when not given.

use super::stats::paired;
use super::*;
use crate::calc::{Eval, Res};
use crate::value::XlErr;

pub static DEFS: &[Def] = &[
    d!("FORECAST.ETS", 3, 6, forecast_ets),
    d!("FORECAST.ETS.CONFINT", 3, 7, forecast_confint),
    d!("FORECAST.ETS.SEASONALITY", 2, 4, forecast_seasonality),
    d!("FORECAST.ETS.STAT", 3, 6, forecast_stat),
];

/// A series on a regular timeline.
pub struct Series {
    pub start: f64,
    pub step: f64,
    pub y: Vec<f64>,
}

/// The values on their timeline: sorted, the step found, duplicates aggregated, gaps filled.
pub fn regular(ts: &[f64], ys: &[f64], completion: i32, aggregation: i32) -> Result<Series, XlErr> {
    if ts.len() < 3 || ts.len() != ys.len() {
        return Err(XlErr::Num);
    }
    let mut pts: Vec<(f64, f64)> = ts.iter().copied().zip(ys.iter().copied()).collect();
    pts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    // duplicates
    let mut groups: Vec<(f64, Vec<f64>)> = Vec::new();
    for (t, y) in pts {
        match groups.last_mut() {
            Some((lt, v)) if (*lt - t).abs() < 1e-9 => v.push(y),
            _ => groups.push((t, vec![y])),
        }
    }
    let agg = |v: &[f64]| -> f64 {
        match aggregation {
            2 => v.len() as f64,
            3 => v.len() as f64,
            4 => v.iter().cloned().fold(f64::MIN, f64::max),
            5 => {
                let mut s = v.to_vec();
                s.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let n = s.len();
                if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) / 2.0 }
            }
            6 => v.iter().cloned().fold(f64::MAX, f64::min),
            7 => v.iter().sum(),
            _ => v.iter().sum::<f64>() / v.len() as f64,
        }
    };
    let g: Vec<(f64, f64)> = groups.iter().map(|(t, v)| (*t, agg(v))).collect();
    if g.len() < 3 {
        return Err(XlErr::Num);
    }
    // the step: the smallest gap (the timeline must be regular, with gaps of whole steps)
    let step = g.windows(2).map(|w| w[1].0 - w[0].0).fold(f64::INFINITY, f64::min);
    if !(step > 0.0) {
        return Err(XlErr::Num);
    }
    let start = g[0].0;
    let mut y: Vec<f64> = Vec::new();
    for (k, (t, v)) in g.iter().enumerate() {
        let idx = ((t - start) / step).round() as usize;
        if ((t - start) / step - idx as f64).abs() > 1e-6 {
            return Err(XlErr::Num);
        }
        while y.len() < idx {
            // a missing point: interpolated between its neighbours, or 0
            let prev = *y.last().unwrap();
            let gap = idx - (y.len() - 1);
            let frac = 1.0 / gap as f64;
            y.push(if completion == 0 { 0.0 } else { prev + (v - prev) * frac });
        }
        let _ = k;
        y.push(*v);
    }
    if y.len() > 50000 {
        return Err(XlErr::Num);
    }
    Ok(Series { start, step, y })
}

/// The seasonality the data shows (0 when none): the lag with the strongest autocorrelation.
pub fn detect_season(y: &[f64]) -> usize {
    let n = y.len();
    if n < 6 {
        return 0;
    }
    // detrended
    let xs: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let mx = (n - 1) as f64 / 2.0;
    let my = y.iter().sum::<f64>() / n as f64;
    let sxy: f64 = xs.iter().zip(y).map(|(x, v)| (x - mx) * (v - my)).sum();
    let sxx: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
    let b = if sxx > 0.0 { sxy / sxx } else { 0.0 };
    let d: Vec<f64> = y.iter().enumerate().map(|(i, v)| v - (my + b * (i as f64 - mx))).collect();
    let var: f64 = d.iter().map(|v| v * v).sum();
    if var <= 1e-12 {
        return 0;
    }
    let acf = |lag: usize| -> f64 { (0..n - lag).map(|i| d[i] * d[i + lag]).sum::<f64>() / var };
    let mut best = (0usize, 0.0f64);
    for lag in 2..=(n / 2).min(8760) {
        let r = acf(lag);
        // a peak: higher than its neighbours
        let left = acf(lag - 1);
        let right = if lag + 1 < n { acf(lag + 1) } else { f64::MIN };
        if r > left && r >= right && r > best.1 {
            best = (lag, r);
        }
    }
    if best.1 > 0.3 { best.0 } else { 0 }
}

/// The fitted model.
pub struct Fit {
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
    pub level: f64,
    pub trend: f64,
    pub season: Vec<f64>,
    pub m: usize,
    pub sse: f64,
    pub errors: Vec<f64>,
    pub fitted: Vec<f64>,
    pub n: usize,
}

fn run(y: &[f64], m: usize, a: f64, b: f64, g: f64) -> Fit {
    let n = y.len();
    let (mut l, mut t, mut s): (f64, f64, Vec<f64>);
    if m >= 2 && n >= 2 * m {
        let first: f64 = y[..m].iter().sum::<f64>() / m as f64;
        let second: f64 = y[m..2 * m].iter().sum::<f64>() / m as f64;
        l = first;
        t = (second - first) / m as f64;
        s = (0..m).map(|i| y[i] - first).collect();
    } else {
        l = y[0];
        t = if n > 1 { y[1] - y[0] } else { 0.0 };
        s = vec![0.0; m.max(1)];
    }
    let mm = s.len();
    let mut sse = 0.0;
    let mut errors = Vec::with_capacity(n);
    let mut fitted = Vec::with_capacity(n);
    for (i, v) in y.iter().enumerate() {
        let si = if m >= 2 { s[i % mm] } else { 0.0 };
        let f = l + t + si;
        fitted.push(f);
        let e = v - f;
        if i > 0 {
            sse += e * e;
        }
        errors.push(e);
        let lp = l;
        l = a * (v - si) + (1.0 - a) * (l + t);
        t = b * (l - lp) + (1.0 - b) * t;
        if m >= 2 {
            s[i % mm] = g * (v - l) + (1.0 - g) * si;
        }
    }
    Fit { alpha: a, beta: b, gamma: g, level: l, trend: t, season: s, m, sse, errors, fitted, n }
}

/// Parameters by Nelder-Mead on the one-step errors, kept inside (0, 1).
pub fn fit(y: &[f64], m: usize) -> Fit {
    let dims = if m >= 2 { 3 } else { 2 };
    let clamp = |p: &[f64]| -> Vec<f64> { p.iter().map(|v| v.clamp(0.001, 0.999)).collect() };
    let cost = |p: &[f64]| -> f64 {
        let p = clamp(p);
        run(y, m, p[0], p[1], if dims == 3 { p[2] } else { 0.0 }).sse
    };
    let mut simplex: Vec<Vec<f64>> = vec![vec![0.5, 0.1, 0.1][..dims].to_vec()];
    for i in 0..dims {
        let mut p = simplex[0].clone();
        p[i] = (p[i] + 0.25).min(0.95);
        simplex.push(p);
    }
    let mut vals: Vec<f64> = simplex.iter().map(|p| cost(p)).collect();
    for _ in 0..400 {
        let mut idx: Vec<usize> = (0..=dims).collect();
        idx.sort_by(|a, b| vals[*a].partial_cmp(&vals[*b]).unwrap_or(std::cmp::Ordering::Equal));
        simplex = idx.iter().map(|i| simplex[*i].clone()).collect();
        vals = idx.iter().map(|i| vals[*i]).collect();
        if (vals[dims] - vals[0]).abs() <= 1e-10 * (1.0 + vals[0].abs()) {
            break;
        }
        let c: Vec<f64> = (0..dims).map(|j| simplex[..dims].iter().map(|p| p[j]).sum::<f64>() / dims as f64).collect();
        let w = &simplex[dims];
        let refl: Vec<f64> = (0..dims).map(|j| c[j] + (c[j] - w[j])).collect();
        let fr = cost(&refl);
        if fr < vals[0] {
            let exp: Vec<f64> = (0..dims).map(|j| c[j] + 2.0 * (c[j] - w[j])).collect();
            let fe = cost(&exp);
            if fe < fr {
                simplex[dims] = exp;
                vals[dims] = fe;
            } else {
                simplex[dims] = refl;
                vals[dims] = fr;
            }
        } else if fr < vals[dims - 1] {
            simplex[dims] = refl;
            vals[dims] = fr;
        } else {
            let con: Vec<f64> = (0..dims).map(|j| c[j] + 0.5 * (w[j] - c[j])).collect();
            let fc = cost(&con);
            if fc < vals[dims] {
                simplex[dims] = con;
                vals[dims] = fc;
            } else {
                let best = simplex[0].clone();
                for k in 1..=dims {
                    simplex[k] = (0..dims).map(|j| best[j] + 0.5 * (simplex[k][j] - best[j])).collect();
                    vals[k] = cost(&simplex[k]);
                }
            }
        }
    }
    let p = clamp(&simplex[0]);
    run(y, m, p[0], p[1], if dims == 3 { p[2] } else { 0.0 })
}

impl Fit {
    /// The forecast h steps after the last point (h may be fractional).
    pub fn at(&self, h: f64) -> f64 {
        let one = |k: i64| -> f64 {
            if k <= 0 {
                // within the data: the fitted value
                let i = (self.n as i64 - 1 + k).max(0) as usize;
                return self.fitted.get(i).copied().unwrap_or(self.level);
            }
            let s = if self.m >= 2 { self.season[((self.n as i64 + k - 1) as usize) % self.season.len()] } else { 0.0 };
            self.level + k as f64 * self.trend + s
        };
        let lo = h.floor();
        let fr = h - lo;
        if fr < 1e-12 {
            one(lo as i64)
        } else {
            one(lo as i64) * (1.0 - fr) + one(lo as i64 + 1) * fr
        }
    }

    pub fn sigma(&self) -> f64 {
        let k = if self.m >= 2 { 3 } else { 2 };
        let df = (self.n as f64 - 1.0 - k as f64).max(1.0);
        (self.sse / df).sqrt()
    }

    /// Half the width of the confidence interval h steps ahead.
    pub fn confint(&self, h: f64, conf: f64) -> f64 {
        let z = super::dist::norm_inv((1.0 + conf) / 2.0);
        let hs = h.ceil().max(1.0) as usize;
        let mut v = 1.0;
        for j in 1..hs {
            let c = self.alpha * (1.0 + j as f64 * self.beta) + if self.m >= 2 && j % self.m == 0 { self.gamma * (1.0 - self.alpha) } else { 0.0 };
            v += c * c;
        }
        z * self.sigma() * v.sqrt()
    }
}

fn opt_num(ev: &mut Eval, a: &[Expr], i: usize, d: f64) -> Result<f64, XlErr> {
    if a.len() > i && !matches!(a[i], Expr::Missing) {
        num(ev, &a[i])
    } else {
        Ok(d)
    }
}

struct Setup {
    s: Series,
    fit: Fit,
    /// the timeline counted in months (or years) when its dates step by them
    calendar: Option<u32>,
}

/// Dates a whole number of months apart (months, quarters, years) on the same day of the
/// month are steps of one, as the desktop spreadsheet counts them.
fn calendar_steps(ts: &[f64]) -> Option<u32> {
    let mut s = ts.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    s.dedup();
    if s.len() < 2 || s.iter().any(|t| t.fract() != 0.0 || *t < 1.0) {
        return None;
    }
    let ymd: Vec<(i64, u32, u32)> = s.iter().filter_map(|t| crate::dates::serial_to_ymd(*t, false)).collect();
    if ymd.len() != s.len() || ymd.iter().any(|x| x.2 != ymd[0].2) {
        return None;
    }
    let mi: Vec<i64> = ymd.iter().map(|(y, m, _)| y * 12 + *m as i64).collect();
    let step = mi.windows(2).map(|w| w[1] - w[0]).min()?;
    if step <= 0 || mi.windows(2).any(|w| (w[1] - w[0]) % step != 0) {
        return None;
    }
    // plain day counts stay days when they already step evenly (weekly data, say)
    let days: Vec<f64> = s.windows(2).map(|w| w[1] - w[0]).collect();
    if days.iter().all(|d| (d - days[0]).abs() < 1e-9) {
        return None;
    }
    Some(step as u32)
}

fn months(t: f64, unit: u32) -> f64 {
    match crate::dates::serial_to_ymd(t.floor(), false) {
        Some((y, m, d)) => {
            let mi = (y * 12 + m as i64 - 1) as f64 + (d as f64 - 1.0) / 31.0;
            mi / unit as f64
        }
        None => t,
    }
}

fn setup(ev: &mut Eval, values: &Expr, timeline: &Expr, season_arg: f64, completion: f64, aggregation: f64) -> Result<Setup, XlErr> {
    let (ts, ys) = paired(ev, timeline, values)?;
    if !(0.0..=1.0).contains(&completion) || !(1.0..=7.0).contains(&aggregation) {
        return Err(XlErr::Num);
    }
    let calendar = calendar_steps(&ts);
    let ts: Vec<f64> = match calendar {
        Some(u) => ts.iter().map(|t| months(*t, u)).collect(),
        None => ts,
    };
    let s = regular(&ts, &ys, completion as i32, aggregation as i32)?;
    let m = if season_arg == 1.0 {
        detect_season(&s.y)
    } else if season_arg == 0.0 {
        0
    } else if season_arg >= 2.0 && season_arg <= 8760.0 && season_arg.fract() == 0.0 {
        season_arg as usize
    } else {
        return Err(XlErr::Num);
    };
    let fit = fit(&s.y, m);
    Ok(Setup { s, fit, calendar })
}

fn steps_ahead(st: &Setup, target: f64) -> Result<f64, XlErr> {
    let target = match st.calendar {
        Some(u) => months(target, u),
        None => target,
    };
    let last = st.s.start + st.s.step * (st.s.y.len() - 1) as f64;
    let h = (target - last) / st.s.step;
    if target < st.s.start {
        return Err(XlErr::Num);
    }
    Ok(h)
}

fn forecast_ets(ev: &mut Eval, a: &[Expr]) -> Res {
    let target = tri!(num(ev, &a[0]));
    let season = tri!(opt_num(ev, a, 3, 1.0));
    let comp = tri!(opt_num(ev, a, 4, 1.0));
    let agg = tri!(opt_num(ev, a, 5, 1.0));
    let st = tri!(setup(ev, &a[1], &a[2], season, comp, agg));
    let h = tri!(steps_ahead(&st, target));
    Res::num(st.fit.at(h))
}

fn forecast_confint(ev: &mut Eval, a: &[Expr]) -> Res {
    let target = tri!(num(ev, &a[0]));
    let conf = tri!(opt_num(ev, a, 3, 0.95));
    if !(conf > 0.0 && conf < 1.0) {
        return Res::err(XlErr::Num);
    }
    let season = tri!(opt_num(ev, a, 4, 1.0));
    let comp = tri!(opt_num(ev, a, 5, 1.0));
    let agg = tri!(opt_num(ev, a, 6, 1.0));
    let st = tri!(setup(ev, &a[1], &a[2], season, comp, agg));
    let h = tri!(steps_ahead(&st, target));
    Res::num(st.fit.confint(h.max(1.0), conf))
}

fn forecast_seasonality(ev: &mut Eval, a: &[Expr]) -> Res {
    let comp = tri!(opt_num(ev, a, 2, 1.0));
    let agg = tri!(opt_num(ev, a, 3, 1.0));
    let st = tri!(setup(ev, &a[0], &a[1], 1.0, comp, agg));
    Res::num(st.fit.m as f64)
}

fn forecast_stat(ev: &mut Eval, a: &[Expr]) -> Res {
    let kind = tri!(num(ev, &a[2])) as i32;
    let season = tri!(opt_num(ev, a, 3, 1.0));
    let comp = tri!(opt_num(ev, a, 4, 1.0));
    let agg = tri!(opt_num(ev, a, 5, 1.0));
    let st = tri!(setup(ev, &a[0], &a[1], season, comp, agg));
    let f = &st.fit;
    let y = &st.s.y;
    let n = y.len();
    let errs: Vec<f64> = f.errors[1..].to_vec();
    let v = match kind {
        1 => f.alpha,
        2 => f.beta,
        3 => f.gamma,
        4 => {
            // mean absolute scaled error (against the naive one-step forecast)
            let naive: f64 = (1..n).map(|i| (y[i] - y[i - 1]).abs()).sum::<f64>() / (n - 1) as f64;
            let mae: f64 = errs.iter().map(|e| e.abs()).sum::<f64>() / errs.len() as f64;
            if naive == 0.0 { return Res::err(XlErr::Div0); }
            mae / naive
        }
        5 => {
            (1..n).map(|i| {
                let d = y[i].abs() + f.fitted[i].abs();
                if d == 0.0 { 0.0 } else { 2.0 * (y[i] - f.fitted[i]).abs() / d }
            }).sum::<f64>() / (n - 1) as f64
        }
        6 => errs.iter().map(|e| e.abs()).sum::<f64>() / errs.len() as f64,
        7 => (errs.iter().map(|e| e * e).sum::<f64>() / errs.len() as f64).sqrt(),
        8 => if st.calendar.is_some() { st.s.step } else { st.s.step },
        _ => return Res::err(XlErr::Num),
    };
    Res::num(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasonal_series() {
        // a trend with a season of 4
        let pat = [10.0, 14.0, 8.0, 12.0];
        let y: Vec<f64> = (0..24).map(|i| 100.0 + 2.0 * i as f64 + pat[i % 4]).collect();
        let ts: Vec<f64> = (0..24).map(|i| 45000.0 + 30.0 * i as f64).collect();
        let s = regular(&ts, &y, 1, 1).unwrap();
        assert_eq!(s.step, 30.0);
        assert_eq!(detect_season(&s.y), 4);
        let f = fit(&s.y, 4);
        // next point: 100 + 2*24 + pat[0]
        let want = 100.0 + 48.0 + pat[0];
        assert!((f.at(1.0) - want).abs() < 1.0, "{} vs {}", f.at(1.0), want);
        assert!(f.confint(1.0, 0.95) >= 0.0);
        // a gap is filled in
        let ts2 = [1.0, 2.0, 4.0, 5.0];
        let s2 = regular(&ts2, &[1.0, 2.0, 4.0, 5.0], 1, 1).unwrap();
        assert_eq!(s2.y, vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    }
}
