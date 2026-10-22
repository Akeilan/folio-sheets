//! Formula evaluation: operators, coercion, references and the function dispatcher.

use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::formula::{BinOp, Expr, RefKind, RefTok};
use crate::value::{Array, Value, XlErr};
use std::rc::Rc;

/// What the evaluator needs from the workbook.
pub trait Ctx {
    fn value(&mut self, sheet: usize, r: u32, c: u32) -> Value;
    fn sheet_index(&self, name: &str) -> Option<usize>;
    fn sheet_name(&self, i: usize) -> String;
    fn sheet_count(&self) -> usize;
    /// non-empty cells inside a rectangle, row-major
    fn cells_in(&mut self, sheet: usize, rc: Rect) -> Vec<(u32, u32, Value)>;
    /// last row/column holding anything (to bound whole-column references)
    fn extent(&self, sheet: usize) -> (u32, u32);
    fn name(&self, name: &str, scope: usize) -> Option<(Rc<Expr>, Option<usize>)>;
    fn date1904(&self) -> bool;
    fn formula_text(&self, sheet: usize, r: u32, c: u32) -> Option<String>;
    fn number_format(&self, sheet: usize, r: u32, c: u32) -> String;
    fn row_hidden(&self, sheet: usize, r: u32) -> bool;
    fn cell_info(&self, sheet: usize, r: u32, c: u32, what: &str) -> Value;
    fn now(&self) -> f64;
    fn rand(&mut self) -> f64;
    /// a cell that is itself a SUBTOTAL/AGGREGATE (skipped by those functions)
    fn is_subtotal(&self, sheet: usize, r: u32, c: u32) -> bool;
}

#[derive(Clone, Debug, PartialEq)]
pub struct Area {
    pub sheet: usize,
    pub rect: Rect,
}

#[derive(Clone, Debug)]
pub struct Lambda {
    pub params: Vec<String>,
    pub body: Expr,
    pub captured: Vec<(String, Res)>,
}

#[derive(Clone, Debug)]
pub enum Res {
    V(Value),
    /// one or more areas (a union has several)
    R(Vec<Area>),
    L(Rc<Lambda>),
}

impl Res {
    pub fn err(e: XlErr) -> Res {
        Res::V(Value::Err(e))
    }
    pub fn num(v: f64) -> Res {
        Res::V(Value::num(v))
    }
}

pub struct Eval<'a> {
    pub ctx: &'a mut dyn Ctx,
    pub sheet: usize,
    pub row: u32,
    pub col: u32,
    /// legacy formula: ranges in single-value places use implicit intersection
    pub legacy: bool,
    pub locals: Vec<(String, Res)>,
    pub depth: u32,
}

pub fn err(e: XlErr) -> Value {
    Value::Err(e)
}

// --------------------------------------------------------------------------- coercion

/// Text to a number the way formulas convert it ("1,234", "50%", "1/2/2026", "$3").
pub fn text_to_number(s: &str, date1904: bool) -> Option<f64> {
    crate::input::parse_number_text(s, date1904).map(|(v, _)| v)
}

pub fn to_num(v: &Value, date1904: bool) -> Result<f64, XlErr> {
    match v {
        Value::Num(n) => Ok(*n),
        Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        Value::Empty => Ok(0.0),
        Value::Str(s) => text_to_number(s, date1904).ok_or(XlErr::Value),
        Value::Err(e) => Err(*e),
        Value::Arr(a) => to_num(a.data.first().unwrap_or(&Value::Empty), date1904),
    }
}

pub fn to_text(v: &Value) -> Result<String, XlErr> {
    match v {
        Value::Err(e) => Err(*e),
        Value::Arr(a) => to_text(a.data.first().unwrap_or(&Value::Empty)),
        v => Ok(v.to_string()),
    }
}

pub fn to_bool(v: &Value) -> Result<bool, XlErr> {
    match v {
        Value::Bool(b) => Ok(*b),
        Value::Num(n) => Ok(*n != 0.0),
        Value::Empty => Ok(false),
        Value::Str(s) => {
            if s.eq_ignore_ascii_case("TRUE") {
                Ok(true)
            } else if s.eq_ignore_ascii_case("FALSE") {
                Ok(false)
            } else {
                Err(XlErr::Value)
            }
        }
        Value::Err(e) => Err(*e),
        Value::Arr(a) => to_bool(a.data.first().unwrap_or(&Value::Empty)),
    }
}

/// Compare two values the way = < > do (text without case; numbers < text < logicals).
pub fn compare(a: &Value, b: &Value) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    let norm = |v: &Value| -> Value {
        match v {
            Value::Empty => Value::Empty,
            x => x.clone(),
        }
    };
    let (a, b) = (norm(a), norm(b));
    match (&a, &b) {
        (Value::Empty, Value::Empty) => Equal,
        (Value::Empty, Value::Num(n)) => 0.0f64.partial_cmp(n).unwrap_or(Equal),
        (Value::Num(n), Value::Empty) => n.partial_cmp(&0.0).unwrap_or(Equal),
        (Value::Empty, Value::Str(s)) => if s.is_empty() { Equal } else { Less },
        (Value::Str(s), Value::Empty) => if s.is_empty() { Equal } else { Greater },
        (Value::Empty, Value::Bool(b)) => if *b { Less } else { Equal },
        (Value::Bool(b), Value::Empty) => if *b { Greater } else { Equal },
        (Value::Num(x), Value::Num(y)) => {
            // equal within 15 significant digits
            if x == y || num_eq(*x, *y) {
                Equal
            } else {
                x.partial_cmp(y).unwrap_or(Equal)
            }
        }
        (Value::Str(x), Value::Str(y)) => cmp_text(x, y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => a.type_rank().cmp(&b.type_rank()),
    }
}

pub fn num_eq(x: f64, y: f64) -> bool {
    if x == y {
        return true;
    }
    let m = x.abs().max(y.abs());
    (x - y).abs() <= m * 1e-15
}

pub fn cmp_text(a: &str, b: &str) -> std::cmp::Ordering {
    let la = a.to_lowercase();
    let lb = b.to_lowercase();
    la.cmp(&lb)
}

// --------------------------------------------------------------------------- the evaluator

impl<'a> Eval<'a> {
    pub fn new(ctx: &'a mut dyn Ctx, sheet: usize, row: u32, col: u32, legacy: bool) -> Self {
        Eval { ctx, sheet, row, col, legacy, locals: Vec::new(), depth: 0 }
    }

    pub fn d1904(&self) -> bool {
        self.ctx.date1904()
    }

    /// Evaluate a whole formula to a cell result (a value or, for dynamic formulas, an array).
    pub fn eval_top(&mut self, e: &Expr) -> Value {
        let r = self.eval(e);
        match r {
            Res::V(v) => v,
            Res::R(areas) => {
                if self.legacy {
                    self.intersect_value(&areas)
                } else {
                    self.range_value(&areas)
                }
            }
            Res::L(_) => Value::Err(XlErr::Calc),
        }
    }

    pub fn eval(&mut self, e: &Expr) -> Res {
        self.depth += 1;
        if self.depth > 400 {
            self.depth -= 1;
            return Res::err(XlErr::Num);
        }
        let r = self.eval_inner(e);
        self.depth -= 1;
        r
    }

    fn eval_inner(&mut self, e: &Expr) -> Res {
        match e {
            Expr::Num(n) => Res::num(*n),
            Expr::Str(s) => Res::V(Value::Str(s.clone())),
            Expr::Bool(b) => Res::V(Value::Bool(*b)),
            Expr::Err(x) => Res::err(*x),
            Expr::Missing => Res::V(Value::Empty),
            Expr::Array(rows) => {
                let r = rows.len();
                let c = rows.first().map(|x| x.len()).unwrap_or(0);
                let mut data = Vec::with_capacity(r * c);
                for row in rows {
                    for item in row {
                        data.push(match self.eval(item) {
                            Res::V(v) => v,
                            _ => Value::Err(XlErr::Value),
                        });
                    }
                }
                Res::V(Value::Arr(Rc::new(Array::new(r, c, data))))
            }
            Expr::Ref(r) => self.eval_ref(r),
            Expr::Name(sheet, name) => self.eval_name(sheet.as_deref(), name),
            Expr::Func(name, args) => crate::fns::call(self, name, args),
            Expr::Call(f, args) => {
                let fv = self.eval(f);
                match fv {
                    Res::L(l) => self.call_lambda(&l, args),
                    Res::V(Value::Err(e)) => Res::err(e),
                    _ => Res::err(XlErr::Value),
                }
            }
            Expr::Neg(x) => {
                let v = self.eval_value(x);
                self.map1(v, |n| Value::num(-n))
            }
            Expr::Plus(x) => self.eval(x),
            Expr::Percent(x) => {
                let v = self.eval_value(x);
                self.map1(v, |n| Value::num(n / 100.0))
            }
            Expr::Bin(op, a, b) => {
                let va = self.eval_value(a);
                let vb = self.eval_value(b);
                Res::V(self.binary(*op, va, vb))
            }
            Expr::Range(a, b) => {
                let ra = self.eval(a);
                let rb = self.eval(b);
                match (ra, rb) {
                    (Res::R(x), Res::R(y)) => {
                        if let (Some(p), Some(q)) = (x.first(), y.first()) {
                            if p.sheet != q.sheet {
                                return Res::err(XlErr::Value);
                            }
                            let mut rect = p.rect.union(&q.rect);
                            for a in x.iter().chain(y.iter()) {
                                rect = rect.union(&a.rect);
                            }
                            return Res::R(vec![Area { sheet: p.sheet, rect }]);
                        }
                        Res::err(XlErr::Ref)
                    }
                    (Res::V(Value::Err(e)), _) | (_, Res::V(Value::Err(e))) => Res::err(e),
                    _ => Res::err(XlErr::Value),
                }
            }
            Expr::Isect(a, b) => {
                let ra = self.eval(a);
                let rb = self.eval(b);
                match (ra, rb) {
                    (Res::R(x), Res::R(y)) => {
                        let mut out = Vec::new();
                        for p in &x {
                            for q in &y {
                                if p.sheet == q.sheet {
                                    if let Some(i) = p.rect.intersect(&q.rect) {
                                        out.push(Area { sheet: p.sheet, rect: i });
                                    }
                                }
                            }
                        }
                        if out.is_empty() {
                            Res::err(XlErr::Null)
                        } else {
                            Res::R(out)
                        }
                    }
                    (Res::V(Value::Err(e)), _) | (_, Res::V(Value::Err(e))) => Res::err(e),
                    _ => Res::err(XlErr::Value),
                }
            }
            Expr::Union(items) => {
                let mut out = Vec::new();
                for it in items {
                    match self.eval(it) {
                        Res::R(a) => out.extend(a),
                        Res::V(Value::Err(e)) => return Res::err(e),
                        _ => return Res::err(XlErr::Value),
                    }
                }
                Res::R(out)
            }
            Expr::Spill(x) => {
                // A1# : the spilled range of the formula in A1
                match self.eval(x) {
                    Res::R(areas) if areas.len() == 1 => {
                        let a = &areas[0];
                        match self.spill_range(a.sheet, a.rect.r1, a.rect.c1) {
                            Some(rc) => Res::R(vec![Area { sheet: a.sheet, rect: rc }]),
                            None => Res::err(XlErr::Ref),
                        }
                    }
                    Res::V(Value::Err(e)) => Res::err(e),
                    _ => Res::err(XlErr::Ref),
                }
            }
            Expr::At(x) => {
                let r = self.eval(x);
                match r {
                    Res::R(areas) => Res::V(self.intersect_value(&areas)),
                    Res::V(Value::Arr(a)) => Res::V(a.data.first().cloned().unwrap_or(Value::Empty)),
                    other => other,
                }
            }
            Expr::Struct(_) => Res::err(XlErr::Ref),
            Expr::Ext(_) => Res::err(XlErr::Ref),
        }
    }

    fn spill_range(&mut self, sheet: usize, r: u32, c: u32) -> Option<Rect> {
        // the workbook reports a spill through cell_info("spill")
        match self.ctx.cell_info(sheet, r, c, "spill") {
            Value::Arr(a) if a.data.len() == 4 => {
                let g = |i: usize| match a.data[i] {
                    Value::Num(n) => n as u32,
                    _ => 0,
                };
                Some(Rect::new(g(0), g(1), g(2), g(3)))
            }
            _ => None,
        }
    }

    pub fn eval_ref(&mut self, r: &RefTok) -> Res {
        if r.kind == RefKind::Err {
            return Res::err(XlErr::Ref);
        }
        let sheet = match &r.sheet {
            Some(name) => match self.ctx.sheet_index(name) {
                Some(i) => i,
                None => return Res::err(XlErr::Ref),
            },
            None => self.sheet,
        };
        let rect = Rect::new(r.r1, r.c1, r.r2, r.c2);
        if let Some(s2) = &r.sheet2 {
            // 3D reference: every sheet from the first to the last
            let j = match self.ctx.sheet_index(s2) {
                Some(j) => j,
                None => return Res::err(XlErr::Ref),
            };
            let (a, b) = (sheet.min(j), sheet.max(j));
            return Res::R((a..=b).map(|s| Area { sheet: s, rect }).collect());
        }
        Res::R(vec![Area { sheet, rect }])
    }

    fn eval_name(&mut self, sheet: Option<&str>, name: &str) -> Res {
        if sheet.is_none() {
            for (k, v) in self.locals.iter().rev() {
                if k.eq_ignore_ascii_case(name) {
                    return v.clone();
                }
            }
        }
        let scope = match sheet {
            Some(s) => match self.ctx.sheet_index(s) {
                Some(i) => i,
                None => return Res::err(XlErr::Ref),
            },
            None => self.sheet,
        };
        match self.ctx.name(name, scope) {
            Some((expr, name_sheet)) => {
                let saved = self.sheet;
                if let Some(s) = name_sheet {
                    // references without a sheet inside a name belong to its sheet
                    self.sheet = s;
                }
                let r = self.eval(&expr);
                self.sheet = saved;
                r
            }
            None => Res::err(XlErr::Name),
        }
    }

    pub fn call_lambda(&mut self, l: &Lambda, args: &[Expr]) -> Res {
        if args.len() != l.params.len() {
            return Res::err(XlErr::Value);
        }
        let mut vals = Vec::new();
        for a in args {
            vals.push(self.eval(a));
        }
        self.call_lambda_values(l, vals)
    }

    pub fn call_lambda_values(&mut self, l: &Lambda, vals: Vec<Res>) -> Res {
        if vals.len() > l.params.len() {
            return Res::err(XlErr::Value);
        }
        let n = self.locals.len();
        for (k, v) in &l.captured {
            self.locals.push((k.clone(), v.clone()));
        }
        for (i, p) in l.params.iter().enumerate() {
            let v = vals.get(i).cloned().unwrap_or(Res::V(Value::Empty));
            self.locals.push((p.clone(), v));
        }
        let r = self.eval(&l.body);
        self.locals.truncate(n);
        r
    }

    /// Evaluate to a value (ranges become arrays, or the intersecting cell in legacy formulas).
    pub fn eval_value(&mut self, e: &Expr) -> Value {
        let r = self.eval(e);
        self.res_value(r)
    }

    pub fn res_value(&mut self, r: Res) -> Value {
        match r {
            Res::V(v) => v,
            Res::R(areas) => {
                if self.legacy {
                    self.intersect_value(&areas)
                } else {
                    self.range_value(&areas)
                }
            }
            Res::L(_) => Value::Err(XlErr::Calc),
        }
    }

    /// One value from an argument: a single cell, or implicit intersection.
    pub fn eval_scalar(&mut self, e: &Expr) -> Value {
        let r = self.eval(e);
        match r {
            Res::V(Value::Arr(a)) => a.data.first().cloned().unwrap_or(Value::Empty),
            Res::V(v) => v,
            Res::R(areas) => self.intersect_value(&areas),
            Res::L(_) => Value::Err(XlErr::Calc),
        }
    }

    /// The cell of a range in the formula's own row or column.
    pub fn intersect_value(&mut self, areas: &[Area]) -> Value {
        if areas.len() != 1 {
            return Value::Err(XlErr::Value);
        }
        let a = &areas[0];
        let rc = a.rect;
        if rc.r1 == rc.r2 && rc.c1 == rc.c2 {
            return self.ctx.value(a.sheet, rc.r1, rc.c1);
        }
        if a.sheet != self.sheet && !(rc.r1 == rc.r2 || rc.c1 == rc.c2) {
            return Value::Err(XlErr::Value);
        }
        if rc.c1 == rc.c2 {
            if self.row >= rc.r1 && self.row <= rc.r2 {
                return self.ctx.value(a.sheet, self.row, rc.c1);
            }
            return Value::Err(XlErr::Value);
        }
        if rc.r1 == rc.r2 {
            if self.col >= rc.c1 && self.col <= rc.c2 {
                return self.ctx.value(a.sheet, rc.r1, self.col);
            }
            return Value::Err(XlErr::Value);
        }
        if rc.contains(self.row, self.col) {
            return self.ctx.value(a.sheet, self.row, self.col);
        }
        Value::Err(XlErr::Value)
    }

    /// A range as an array (one cell stays a single value). Whole columns and rows are cut
    /// to the part of the sheet in use.
    pub fn range_value(&mut self, areas: &[Area]) -> Value {
        if areas.len() != 1 {
            return Value::Err(XlErr::Value);
        }
        let a = &areas[0];
        let rc = self.bound(a);
        if rc.r1 == rc.r2 && rc.c1 == rc.c2 && a.rect.count() == 1 {
            return self.ctx.value(a.sheet, rc.r1, rc.c1);
        }
        Value::Arr(Rc::new(self.area_array(a.sheet, rc)))
    }

    /// Cut a whole-column/row area to the used part of the sheet (at least one cell).
    pub fn bound(&self, a: &Area) -> Rect {
        let (er, ec) = self.ctx.extent(a.sheet);
        let mut rc = a.rect;
        if rc.r2 > er && rc.rows() > 65536 {
            rc.r2 = er.max(rc.r1);
        }
        if rc.c2 > ec && rc.cols() > 1024 {
            rc.c2 = ec.max(rc.c1);
        }
        rc
    }

    pub fn area_array(&mut self, sheet: usize, rc: Rect) -> Array {
        let rows = rc.rows() as usize;
        let cols = rc.cols() as usize;
        let mut data = vec![Value::Empty; rows * cols];
        if (rows * cols) as u64 > 50_000_000 {
            return Array::new(1, 1, vec![Value::Err(XlErr::Num)]);
        }
        for (r, c, v) in self.ctx.cells_in(sheet, rc) {
            data[(r - rc.r1) as usize * cols + (c - rc.c1) as usize] = v;
        }
        Array::new(rows, cols, data)
    }

    // ---- operators
    fn map1(&mut self, v: Value, f: impl Fn(f64) -> Value) -> Res {
        let d = self.d1904();
        Res::V(match v {
            Value::Arr(a) => Value::Arr(Rc::new(Array::new(a.rows, a.cols, a.data.iter().map(|x| match to_num(x, d) {
                Ok(n) => f(n),
                Err(e) => Value::Err(e),
            }).collect()))),
            v => match to_num(&v, d) {
                Ok(n) => f(n),
                Err(e) => Value::Err(e),
            },
        })
    }

    pub fn binary(&mut self, op: BinOp, a: Value, b: Value) -> Value {
        if let (Value::Arr(_), _) | (_, Value::Arr(_)) = (&a, &b) {
            return broadcast(&a, &b, |x, y| scalar_op(op, x, y, self.d1904()));
        }
        scalar_op(op, &a, &b, self.d1904())
    }
}

/// Element-wise combination of two values with broadcasting of single rows/columns.
pub fn broadcast(a: &Value, b: &Value, mut f: impl FnMut(&Value, &Value) -> Value) -> Value {
    let dims = |v: &Value| match v {
        Value::Arr(x) => (x.rows, x.cols),
        _ => (1, 1),
    };
    let (ar, ac) = dims(a);
    let (br, bc) = dims(b);
    let rows = if ar == 1 { br } else if br == 1 { ar } else { ar.max(br) };
    let cols = if ac == 1 { bc } else if bc == 1 { ac } else { ac.max(bc) };
    let get = |v: &Value, r: usize, c: usize| -> Value {
        match v {
            Value::Arr(x) => {
                let rr = if x.rows == 1 { 0 } else { r };
                let cc = if x.cols == 1 { 0 } else { c };
                if rr < x.rows && cc < x.cols {
                    x.get(rr, cc).clone()
                } else {
                    Value::Err(XlErr::NA)
                }
            }
            v => v.clone(),
        }
    };
    let mut data = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            data.push(f(&get(a, r, c), &get(b, r, c)));
        }
    }
    Value::Arr(Rc::new(Array::new(rows, cols, data)))
}

pub fn scalar_op(op: BinOp, a: &Value, b: &Value, d1904: bool) -> Value {
    match op {
        BinOp::Cat => {
            if let Value::Err(e) = a {
                return Value::Err(*e);
            }
            if let Value::Err(e) = b {
                return Value::Err(*e);
            }
            let s = format!("{}{}", a, b);
            if s.chars().count() > 32767 {
                return Value::Err(XlErr::Value);
            }
            Value::string(s)
        }
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
            if let Value::Err(e) = a {
                return Value::Err(*e);
            }
            if let Value::Err(e) = b {
                return Value::Err(*e);
            }
            let o = compare(a, b);
            use std::cmp::Ordering::*;
            Value::Bool(match op {
                BinOp::Eq => o == Equal,
                BinOp::Ne => o != Equal,
                BinOp::Lt => o == Less,
                BinOp::Le => o != Greater,
                BinOp::Gt => o == Greater,
                _ => o != Less,
            })
        }
        _ => {
            let x = match to_num(a, d1904) {
                Ok(x) => x,
                Err(e) => return Value::Err(e),
            };
            let y = match to_num(b, d1904) {
                Ok(y) => y,
                Err(e) => return Value::Err(e),
            };
            match op {
                BinOp::Add => Value::num(round15_add(x, y)),
                BinOp::Sub => Value::num(round15_add(x, -y)),
                BinOp::Mul => Value::num(x * y),
                BinOp::Div => {
                    if y == 0.0 {
                        Value::Err(XlErr::Div0)
                    } else {
                        Value::num(x / y)
                    }
                }
                BinOp::Pow => pow(x, y),
                _ => Value::Err(XlErr::Value),
            }
        }
    }
}

/// Addition that snaps results like 0.1+0.2-0.3 to zero, as the original does when the
/// result is tiny compared with the operands.
pub fn round15_add(x: f64, y: f64) -> f64 {
    let s = x + y;
    if s != 0.0 {
        let m = x.abs().max(y.abs());
        if s.abs() < m * 1e-15 {
            return 0.0;
        }
    }
    s
}

pub fn pow(x: f64, y: f64) -> Value {
    if x == 0.0 && y == 0.0 {
        return Value::Err(XlErr::Num);
    }
    if x == 0.0 && y < 0.0 {
        return Value::Err(XlErr::Div0);
    }
    if x < 0.0 && y.fract() != 0.0 {
        // odd roots of negatives: (-8)^(1/3) = -2
        let inv = 1.0 / y;
        if (inv.round() - inv).abs() < 1e-10 && (inv.round() as i64) % 2 != 0 {
            return Value::num(-(-x).powf(y));
        }
        return Value::Err(XlErr::Num);
    }
    Value::num(x.powf(y))
}
