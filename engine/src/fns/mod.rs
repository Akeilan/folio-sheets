//! Worksheet functions: the registry and the argument helpers every function uses.

use crate::calc::{to_bool, to_num, to_text, Area, Eval, Res};
use crate::cellref::Rect;
use crate::formula::Expr;
use crate::value::{Array, Value, XlErr};
use std::rc::Rc;

pub mod date;
pub mod db;
pub mod dist;
pub mod eng;
pub mod fin;
pub mod logic;
pub mod lookup;
pub mod math;
pub mod stats;
pub mod text;

pub type Func = fn(&mut Eval, &[Expr]) -> Res;

pub struct Def {
    pub name: &'static str,
    pub min: usize,
    pub max: usize,
    pub f: Func,
    pub volatile: bool,
}

pub const MANY: usize = 255;

macro_rules! d {
    ($name:expr, $min:expr, $max:expr, $f:expr) => {
        Def { name: $name, min: $min, max: $max, f: $f, volatile: false }
    };
    ($name:expr, $min:expr, $max:expr, $f:expr, vol) => {
        Def { name: $name, min: $min, max: $max, f: $f, volatile: true }
    };
}
pub(crate) use d;

thread_local! {
    static TABLE: std::collections::HashMap<&'static str, &'static Def> = {
        let mut m = std::collections::HashMap::new();
        for list in [math::DEFS, stats::DEFS, text::DEFS, date::DEFS, lookup::DEFS, logic::DEFS, fin::DEFS, eng::DEFS, dist::DEFS, db::DEFS] {
            for d in list.iter() {
                m.insert(d.name, d);
            }
        }
        m
    };
}

pub fn lookup_def(name: &str) -> Option<&'static Def> {
    TABLE.with(|t| t.get(name).copied())
}

pub fn names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = TABLE.with(|t| t.keys().copied().collect());
    v.sort();
    v
}

pub fn is_volatile(name: &str) -> bool {
    lookup_def(name).map(|d| d.volatile).unwrap_or(false)
}

pub fn call(ev: &mut Eval, name: &str, args: &[Expr]) -> Res {
    match lookup_def(name) {
        Some(d) => {
            if args.len() < d.min || args.len() > d.max {
                return Res::err(XlErr::NA);
            }
            (d.f)(ev, args)
        }
        None => {
            // a LAMBDA stored in a name, called like a function
            if let Res::L(l) = ev.eval(&Expr::Name(None, name.to_string())) {
                return ev.call_lambda(&l, args);
            }
            Res::err(XlErr::Name)
        }
    }
}

/// Argument count problems, for checking a formula as it's entered.
pub fn check_args(e: &Expr) -> Option<String> {
    match e {
        Expr::Func(name, args) => {
            if let Some(d) = lookup_def(name) {
                if args.len() < d.min {
                    return Some(format!("You've entered too few arguments for this function ({}).", name));
                }
                if args.len() > d.max {
                    return Some(format!("You've entered too many arguments for this function ({}).", name));
                }
            }
            args.iter().find_map(check_args)
        }
        Expr::Neg(x) | Expr::Plus(x) | Expr::Percent(x) | Expr::Spill(x) | Expr::At(x) => check_args(x),
        Expr::Bin(_, a, b) | Expr::Range(a, b) | Expr::Isect(a, b) => check_args(a).or_else(|| check_args(b)),
        Expr::Union(v) => v.iter().find_map(check_args),
        Expr::Call(f, args) => check_args(f).or_else(|| args.iter().find_map(check_args)),
        _ => None,
    }
}

// --------------------------------------------------------------------------- helpers

pub fn num(ev: &mut Eval, e: &Expr) -> Result<f64, XlErr> {
    let v = ev.eval_scalar(e);
    to_num(&v, ev.d1904())
}

pub fn opt_num(ev: &mut Eval, args: &[Expr], i: usize, default: f64) -> Result<f64, XlErr> {
    match args.get(i) {
        None | Some(Expr::Missing) => Ok(default),
        Some(e) => num(ev, e),
    }
}

pub fn text(ev: &mut Eval, e: &Expr) -> Result<String, XlErr> {
    let v = ev.eval_scalar(e);
    to_text(&v)
}

pub fn opt_text(ev: &mut Eval, args: &[Expr], i: usize, default: &str) -> Result<String, XlErr> {
    match args.get(i) {
        None | Some(Expr::Missing) => Ok(default.to_string()),
        Some(e) => text(ev, e),
    }
}

pub fn boolean(ev: &mut Eval, e: &Expr) -> Result<bool, XlErr> {
    let v = ev.eval_scalar(e);
    to_bool(&v)
}

pub fn opt_bool(ev: &mut Eval, args: &[Expr], i: usize, default: bool) -> Result<bool, XlErr> {
    match args.get(i) {
        None | Some(Expr::Missing) => Ok(default),
        Some(e) => boolean(ev, e),
    }
}

pub fn is_missing(args: &[Expr], i: usize) -> bool {
    matches!(args.get(i), None | Some(Expr::Missing))
}

macro_rules! tri {
    ($e:expr) => {
        match $e {
            Ok(v) => v,
            Err(e) => return Res::err(e),
        }
    };
}
pub(crate) use tri;

/// Apply a scalar function element by element when any argument is an array
/// (dynamic arrays). `f` gets one value per argument.
pub fn lift(ev: &mut Eval, args: &[Expr], f: impl Fn(&mut Eval, &[Value]) -> Value) -> Res {
    let vals: Vec<Value> = args.iter().map(|a| if matches!(a, Expr::Missing) { Value::Empty } else { ev.eval_value(a) }).collect();
    Res::V(lift_values(ev, &vals, f))
}

pub fn lift_values(ev: &mut Eval, vals: &[Value], f: impl Fn(&mut Eval, &[Value]) -> Value) -> Value {
    let arrays: Vec<(usize, usize)> = vals.iter().filter_map(|v| if let Value::Arr(a) = v { Some((a.rows, a.cols)) } else { None }).collect();
    if arrays.is_empty() {
        return f(ev, vals);
    }
    let rows = arrays.iter().map(|x| x.0).max().unwrap();
    let cols = arrays.iter().map(|x| x.1).max().unwrap();
    let mut out = Vec::with_capacity(rows * cols);
    let mut buf = vals.to_vec();
    for r in 0..rows {
        for c in 0..cols {
            for (i, v) in vals.iter().enumerate() {
                buf[i] = match v {
                    Value::Arr(a) => a.get_b(r, c),
                    x => x.clone(),
                };
            }
            out.push(f(ev, &buf));
        }
    }
    Value::Arr(Rc::new(Array::new(rows, cols, out)))
}

/// Number from a lifted argument value.
pub fn vnum(ev: &Eval, v: &Value) -> Result<f64, XlErr> {
    to_num(v, ev.ctx.date1904())
}

pub fn vtext(v: &Value) -> Result<String, XlErr> {
    to_text(v)
}

/// One-number functions: lifted over arrays, errors propagate.
pub fn math1(ev: &mut Eval, args: &[Expr], f: fn(f64) -> Value) -> Res {
    lift(ev, args, |ev, v| match vnum(ev, &v[0]) {
        Ok(x) => f(x),
        Err(e) => Value::Err(e),
    })
}

pub fn math2(ev: &mut Eval, args: &[Expr], f: fn(f64, f64) -> Value) -> Res {
    lift(ev, args, |ev, v| {
        let x = match vnum(ev, &v[0]) {
            Ok(x) => x,
            Err(e) => return Value::Err(e),
        };
        let y = match vnum(ev, &v[1]) {
            Ok(y) => y,
            Err(e) => return Value::Err(e),
        };
        f(x, y)
    })
}

/// How text and logical values in references count for an aggregate.
#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    /// SUM, AVERAGE...: numbers only from references; typed text/logicals convert
    Plain,
    /// AVERAGEA, MAXA...: text counts as 0, logicals as 1/0
    A,
}

/// The numbers an aggregate function works on.
pub fn collect(ev: &mut Eval, args: &[Expr], mode: Mode) -> Result<Vec<f64>, XlErr> {
    let mut out = Vec::new();
    for a in args {
        if matches!(a, Expr::Missing) {
            if mode == Mode::Plain {
                out.push(0.0);
            }
            continue;
        }
        let r = ev.eval(a);
        collect_res(ev, r, mode, &mut out, false)?;
    }
    Ok(out)
}

pub fn collect_res(ev: &mut Eval, r: Res, mode: Mode, out: &mut Vec<f64>, skip_hidden: bool) -> Result<(), XlErr> {
    match r {
        Res::R(areas) => {
            for a in &areas {
                let rc = ev.bound(a);
                for (rr, cc, v) in ev.ctx.cells_in(a.sheet, rc) {
                    if skip_hidden && ev.ctx.row_hidden(a.sheet, rr) {
                        continue;
                    }
                    let _ = cc;
                    match v {
                        Value::Num(n) => out.push(n),
                        Value::Err(e) => return Err(e),
                        Value::Bool(b) if mode == Mode::A => out.push(if b { 1.0 } else { 0.0 }),
                        Value::Str(_) if mode == Mode::A => out.push(0.0),
                        _ => {}
                    }
                }
            }
        }
        Res::V(Value::Arr(arr)) => {
            for v in arr.data.iter() {
                match v {
                    Value::Num(n) => out.push(*n),
                    Value::Err(e) => return Err(*e),
                    Value::Bool(b) if mode == Mode::A => out.push(if *b { 1.0 } else { 0.0 }),
                    Value::Str(_) if mode == Mode::A => out.push(0.0),
                    _ => {}
                }
            }
        }
        Res::V(v) => match v {
            Value::Num(n) => out.push(n),
            Value::Bool(b) => out.push(if b { 1.0 } else { 0.0 }),
            Value::Empty => {}
            Value::Str(s) => {
                let n = crate::calc::text_to_number(&s, ev.d1904()).ok_or(XlErr::Value)?;
                out.push(n);
            }
            Value::Err(e) => return Err(e),
            Value::Arr(_) => {}
        },
        Res::L(_) => return Err(XlErr::Value),
    }
    Ok(())
}

/// Every value of an argument (references and arrays flattened), with blanks.
pub fn values_of(ev: &mut Eval, e: &Expr) -> Vec<Value> {
    let r = ev.eval(e);
    res_values(ev, r)
}

pub fn res_values(ev: &mut Eval, r: Res) -> Vec<Value> {
    match r {
        Res::R(areas) => {
            let mut out = Vec::new();
            for a in &areas {
                let rc = ev.bound(a);
                let arr = ev.area_array(a.sheet, rc);
                out.extend(arr.data);
            }
            out
        }
        Res::V(Value::Arr(a)) => a.data.clone(),
        Res::V(v) => vec![v],
        Res::L(_) => vec![Value::Err(XlErr::Calc)],
    }
}

/// An argument as a 2-D array (a range bounded to the used area, a value as 1x1).
pub fn array_of(ev: &mut Eval, e: &Expr) -> Result<Rc<Array>, XlErr> {
    let r = ev.eval(e);
    res_array(ev, r)
}

pub fn res_array(ev: &mut Eval, r: Res) -> Result<Rc<Array>, XlErr> {
    match r {
        Res::R(areas) => {
            if areas.len() != 1 {
                return Err(XlErr::Value);
            }
            let rc = ev.bound(&areas[0]);
            Ok(Rc::new(ev.area_array(areas[0].sheet, rc)))
        }
        Res::V(Value::Arr(a)) => Ok(a),
        Res::V(Value::Err(e)) => Err(e),
        Res::V(v) => Ok(Rc::new(Array::new(1, 1, vec![v]))),
        Res::L(_) => Err(XlErr::Calc),
    }
}

/// An argument that must be a reference.
pub fn areas_of(ev: &mut Eval, e: &Expr) -> Result<Vec<Area>, XlErr> {
    match ev.eval(e) {
        Res::R(a) => Ok(a),
        Res::V(Value::Err(e)) => Err(e),
        _ => Err(XlErr::Value),
    }
}

pub fn arr_val(v: Array) -> Res {
    if v.rows == 1 && v.cols == 1 {
        return Res::V(v.data.into_iter().next().unwrap());
    }
    Res::V(Value::Arr(Rc::new(v)))
}

pub fn rect_of(a: &Area) -> Rect {
    a.rect
}

// --------------------------------------------------------------------------- criteria

/// A COUNTIF-style condition: "<5", ">=x", "=", "a*b?", 7, TRUE.
pub struct Criteria {
    op: u8, // b'=' b'<' b'>' b'l' (<=) b'g' (>=) b'n' (<>)
    num: Option<f64>,
    text: Option<String>,
    boolv: Option<bool>,
    err: Option<XlErr>,
    wild: bool,
    blank: bool,
}

impl Criteria {
    pub fn new(v: &Value, d1904: bool) -> Criteria {
        let mut c = Criteria { op: b'=', num: None, text: None, boolv: None, err: None, wild: false, blank: false };
        match v {
            Value::Num(n) => c.num = Some(*n),
            Value::Bool(b) => c.boolv = Some(*b),
            Value::Err(e) => c.err = Some(*e),
            Value::Empty => {
                c.num = Some(0.0);
            }
            Value::Str(s) => {
                let s: &str = s;
                let (op, rest) = if let Some(r) = s.strip_prefix("<=") {
                    (b'l', r)
                } else if let Some(r) = s.strip_prefix(">=") {
                    (b'g', r)
                } else if let Some(r) = s.strip_prefix("<>") {
                    (b'n', r)
                } else if let Some(r) = s.strip_prefix('<') {
                    (b'<', r)
                } else if let Some(r) = s.strip_prefix('>') {
                    (b'>', r)
                } else if let Some(r) = s.strip_prefix('=') {
                    (b'=', r)
                } else {
                    (b'=', s)
                };
                c.op = op;
                if rest.is_empty() {
                    c.blank = true;
                    c.text = Some(String::new());
                } else if let Some(n) = crate::calc::text_to_number(rest, d1904) {
                    c.num = Some(n);
                    c.text = Some(rest.to_lowercase());
                } else if rest.eq_ignore_ascii_case("TRUE") {
                    c.boolv = Some(true);
                } else if rest.eq_ignore_ascii_case("FALSE") {
                    c.boolv = Some(false);
                } else if let Some(e) = XlErr::parse(rest) {
                    c.err = Some(e);
                } else {
                    c.wild = (op == b'=' || op == b'n') && (rest.contains('*') || rest.contains('?'));
                    c.text = Some(rest.to_lowercase());
                }
            }
            Value::Arr(a) => return Criteria::new(a.data.first().unwrap_or(&Value::Empty), d1904),
        }
        c
    }

    pub fn matches(&self, v: &Value) -> bool {
        use std::cmp::Ordering;
        let cmp_ok = |o: Ordering| -> bool {
            match self.op {
                b'=' => o == Ordering::Equal,
                b'n' => o != Ordering::Equal,
                b'<' => o == Ordering::Less,
                b'>' => o == Ordering::Greater,
                b'l' => o != Ordering::Greater,
                b'g' => o != Ordering::Less,
                _ => false,
            }
        };
        if self.blank {
            // "=" matches blank cells; "<>" matches non-blank
            let is_blank = matches!(v, Value::Empty) || matches!(v, Value::Str(s) if s.is_empty());
            return match self.op {
                b'=' => is_blank,
                b'n' => !matches!(v, Value::Empty),
                _ => false,
            };
        }
        if let Some(e) = self.err {
            return match v {
                Value::Err(x) => cmp_ok(if *x == e { Ordering::Equal } else { Ordering::Less }),
                _ => self.op == b'n',
            };
        }
        if let Some(b) = self.boolv {
            return match v {
                Value::Bool(x) => cmp_ok(x.cmp(&b)),
                _ => self.op == b'n',
            };
        }
        if let Some(n) = self.num {
            return match v {
                Value::Num(x) => cmp_ok(if crate::calc::num_eq(*x, n) { Ordering::Equal } else { x.partial_cmp(&n).unwrap_or(Ordering::Equal) }),
                Value::Str(s) if self.op == b'=' || self.op == b'n' => {
                    // "5" in a cell is text: equal only for the text form
                    let eq = self.text.as_deref().map(|t| s.to_lowercase() == t).unwrap_or(false);
                    if self.op == b'=' { eq } else { !eq }
                }
                Value::Empty => self.op == b'n' && self.text.is_some(),
                _ => self.op == b'n',
            };
        }
        if let Some(t) = &self.text {
            return match v {
                Value::Str(s) => {
                    if self.wild {
                        let m = wildcard_match(t, &s.to_lowercase());
                        if self.op == b'=' { m } else { !m }
                    } else {
                        cmp_ok(s.to_lowercase().cmp(t))
                    }
                }
                Value::Empty => {
                    if self.op == b'n' {
                        true
                    } else {
                        false
                    }
                }
                _ => self.op == b'n',
            };
        }
        false
    }
}

/// * and ? with ~ as escape; both already lower-cased.
pub fn wildcard_match(pat: &str, s: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = s.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while ti < t.len() {
        if pi < p.len() && p[pi] == '~' && pi + 1 < p.len() {
            if p[pi + 1] == t[ti] {
                pi += 2;
                ti += 1;
                continue;
            }
        } else if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) && p[pi] != '*' {
            pi += 1;
            ti += 1;
            continue;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            mark = ti;
            pi += 1;
            continue;
        }
        if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            ti = mark;
            continue;
        }
        return false;
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Range + criteria pairs as COUNTIFS/SUMIFS take them: which positions match all.
pub fn ifs_mask(ev: &mut Eval, pairs: &[Expr]) -> Result<(Vec<bool>, usize, usize), XlErr> {
    let mut mask: Option<Vec<bool>> = None;
    let mut dims = (0, 0);
    for ch in pairs.chunks(2) {
        if ch.len() != 2 {
            return Err(XlErr::Value);
        }
        let arr = array_of_full(ev, &ch[0])?;
        let crit = ev.eval_scalar(&ch[1]);
        let c = Criteria::new(&crit, ev.d1904());
        if let Some(m) = &mask {
            if arr.rows != dims.0 || arr.cols != dims.1 {
                let _ = m;
                return Err(XlErr::Value);
            }
        } else {
            dims = (arr.rows, arr.cols);
            mask = Some(vec![true; arr.rows * arr.cols]);
        }
        let m = mask.as_mut().unwrap();
        for (i, v) in arr.data.iter().enumerate() {
            if m[i] && !c.matches(v) {
                m[i] = false;
            }
        }
    }
    Ok((mask.unwrap_or_default(), dims.0, dims.1))
}

/// Like array_of but whole columns keep their real height when another argument needs
/// the same shape (bounded to the used rows of the sheet).
pub fn array_of_full(ev: &mut Eval, e: &Expr) -> Result<Rc<Array>, XlErr> {
    array_of(ev, e)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wild() {
        assert!(wildcard_match("a*c", "abbbc"));
        assert!(wildcard_match("a?c", "abc"));
        assert!(!wildcard_match("a?c", "abbc"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("~*", "*"));
    }
}
