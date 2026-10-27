//! Solver (SH-20): the best value of an objective cell by changing variable cells, subject to
//! constraints — Simplex LP for linear models, GRG Nonlinear (here: a penalty method with
//! quasi-Newton steps) for smooth ones, and Evolutionary for the rest. Integer and binary
//! constraints by branch and bound. The model is kept as the desktop spreadsheet keeps it, in
//! hidden sheet-level names (solver_opt, solver_adj, solver_lhs1 ...), so it travels with the file.

use crate::book::{Book, Delta, Name};
use crate::cellref::{parse_range, Rect};
use crate::value::Value;
use crate::whatif::Key;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Goal {
    Max,
    Min,
    Value(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rel {
    Le,
    Eq,
    Ge,
    Int,
    Bin,
    Dif,
}

impl Rel {
    pub fn code(&self) -> u32 {
        match self {
            Rel::Le => 1,
            Rel::Eq => 2,
            Rel::Ge => 3,
            Rel::Int => 4,
            Rel::Bin => 5,
            Rel::Dif => 6,
        }
    }
    pub fn from_code(c: u32) -> Rel {
        match c {
            2 => Rel::Eq,
            3 => Rel::Ge,
            4 => Rel::Int,
            5 => Rel::Bin,
            6 => Rel::Dif,
            _ => Rel::Le,
        }
    }
    pub fn text(&self) -> &'static str {
        match self {
            Rel::Le => "<=",
            Rel::Eq => "=",
            Rel::Ge => ">=",
            Rel::Int => "int",
            Rel::Bin => "bin",
            Rel::Dif => "dif",
        }
    }
    pub fn parse(t: &str) -> Rel {
        match t {
            "=" => Rel::Eq,
            ">=" => Rel::Ge,
            "int" => Rel::Int,
            "bin" => Rel::Bin,
            "dif" => Rel::Dif,
            _ => Rel::Le,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Constraint {
    pub lhs: Rect,
    pub rel: Rel,
    /// a number, or a reference/formula on this sheet
    pub rhs: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Method {
    Grg,
    Simplex,
    Evolutionary,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub objective: Option<(u32, u32)>,
    pub goal: Goal,
    pub vars: Vec<Rect>,
    pub constraints: Vec<Constraint>,
    pub nonneg: bool,
    pub method: Method,
    pub max_iter: u32,
    /// constraint precision
    pub precision: f64,
    /// integer optimality (%)
    pub int_gap: f64,
}

impl Default for Model {
    fn default() -> Model {
        Model { objective: None, goal: Goal::Max, vars: vec![], constraints: vec![], nonneg: true, method: Method::Grg,
                max_iter: 100, precision: 1e-6, int_gap: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Optimal,
    Converged,
    Infeasible,
    Unbounded,
    NotLinear,
    NotConverged,
    Error(String),
}

impl Status {
    pub fn message(&self) -> String {
        match self {
            Status::Optimal => "Solver found a solution. All Constraints and optimality conditions are satisfied.".into(),
            Status::Converged => "Solver has converged to the current solution. All Constraints are satisfied.".into(),
            Status::Infeasible => "Solver could not find a feasible solution.".into(),
            Status::Unbounded => "The Objective Cell values do not converge.".into(),
            Status::NotLinear => "The linearity conditions required by this LP Solver are not satisfied.".into(),
            Status::NotConverged => "Solver cannot improve the current solution. All Constraints are satisfied.".into(),
            Status::Error(e) => e.clone(),
        }
    }
    pub fn found(&self) -> bool {
        matches!(self, Status::Optimal | Status::Converged | Status::NotConverged)
    }
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub status: Status,
    pub start: Vec<f64>,
    pub x: Vec<f64>,
    pub objective_start: f64,
    pub objective: f64,
    pub cells: Vec<(u32, u32)>,
}

// ---------------------------------------------------------------------- the model in names

const PREFIX: &str = "solver_";

fn sheet_ref(sheet: &str, r: &Rect) -> String {
    let q = if sheet.chars().all(|c| c.is_alphanumeric() || c == '_') { sheet.to_string() } else { format!("'{}'", sheet.replace('\'', "''")) };
    let abs = |r: u32, c: u32| format!("${}${}", crate::cellref::col_name(c), r + 1);
    if r.r1 == r.r2 && r.c1 == r.c2 {
        format!("{}!{}", q, abs(r.r1, r.c1))
    } else {
        format!("{}!{}:{}", q, abs(r.r1, r.c1), abs(r.r2, r.c2))
    }
}

fn strip_sheet(t: &str) -> String {
    let t = t.trim().trim_start_matches('=');
    let t = match t.rfind('!') {
        Some(i) => &t[i + 1..],
        None => t,
    };
    t.replace('$', "")
}

impl Book {
    fn solver_name(&self, s: usize, n: &str) -> Option<String> {
        self.names.iter().find(|x| x.scope == Some(s) && x.name.eq_ignore_ascii_case(&format!("{}{}", PREFIX, n))).map(|x| x.text.clone())
    }

    /// The Solver model saved with the sheet (empty when there is none).
    pub fn solver_model(&self, s: usize) -> Model {
        let mut m = Model::default();
        let num = |n: &str| self.solver_name(s, n).and_then(|t| t.trim().parse::<f64>().ok());
        m.objective = self.solver_name(s, "opt").and_then(|t| parse_range(&strip_sheet(&t))).map(|r| (r.r1, r.c1));
        m.goal = match num("typ").map(|v| v as u32) {
            Some(2) => Goal::Min,
            Some(3) => Goal::Value(num("val").unwrap_or(0.0)),
            _ => Goal::Max,
        };
        if let Some(adj) = self.solver_name(s, "adj") {
            m.vars = adj.split(',').filter_map(|p| parse_range(&strip_sheet(p))).collect();
        }
        let n = num("num").unwrap_or(0.0) as usize;
        for k in 1..=n {
            let lhs = match self.solver_name(s, &format!("lhs{}", k)).and_then(|t| parse_range(&strip_sheet(&t))) {
                Some(r) => r,
                None => continue,
            };
            let rel = Rel::from_code(num(&format!("rel{}", k)).unwrap_or(1.0) as u32);
            let rhs = self.solver_name(s, &format!("rhs{}", k)).map(|t| {
                if t.contains('!') { strip_sheet(&t) } else { t.trim().trim_start_matches('=').to_string() }
            }).unwrap_or_default();
            m.constraints.push(Constraint { lhs, rel, rhs });
        }
        m.nonneg = num("neg").map(|v| v as u32 != 2).unwrap_or(true);
        m.method = match num("eng").map(|v| v as u32) {
            Some(2) => Method::Simplex,
            Some(3) => Method::Evolutionary,
            _ => Method::Grg,
        };
        if let Some(v) = num("itr") {
            m.max_iter = v.max(1.0) as u32;
        }
        if let Some(v) = num("pre") {
            m.precision = v;
        }
        if let Some(v) = num("tol") {
            m.int_gap = v * 100.0;
        }
        m
    }

    /// Save the Solver model with the sheet.
    pub fn set_solver_model(&mut self, s: usize, m: &Model) -> Result<(), String> {
        let sheet = self.sheets.get(s).ok_or("No such sheet.")?.name.clone();
        let mut names: Vec<Name> = self.names.iter().filter(|n| !(n.scope == Some(s) && n.name.to_lowercase().starts_with(PREFIX))).cloned().collect();
        let mut add = |n: &str, text: String| {
            let mut x = Name::new(&format!("{}{}", PREFIX, n), Some(s), &text);
            x.hidden = true;
            names.push(x);
        };
        if let Some(o) = m.objective {
            add("opt", sheet_ref(&sheet, &Rect::cell(o.0, o.1)));
        }
        let (typ, val) = match m.goal {
            Goal::Max => (1, 0.0),
            Goal::Min => (2, 0.0),
            Goal::Value(v) => (3, v),
        };
        add("typ", typ.to_string());
        add("val", crate::xlsx::num_text(val));
        if !m.vars.is_empty() {
            add("adj", m.vars.iter().map(|r| sheet_ref(&sheet, r)).collect::<Vec<_>>().join(","));
        }
        add("num", m.constraints.len().to_string());
        for (k, c) in m.constraints.iter().enumerate() {
            add(&format!("lhs{}", k + 1), sheet_ref(&sheet, &c.lhs));
            add(&format!("rel{}", k + 1), c.rel.code().to_string());
            let rhs = match (c.rel, c.rhs.trim().parse::<f64>()) {
                (Rel::Int, _) => "\"integer\"".to_string(),
                (Rel::Bin, _) => "\"binary\"".to_string(),
                (Rel::Dif, _) => "\"AllDifferent\"".to_string(),
                (_, Ok(v)) => crate::xlsx::num_text(v),
                (_, Err(_)) => match parse_range(&c.rhs.replace('$', "")) {
                    Some(r) => sheet_ref(&sheet, &r),
                    None => c.rhs.trim().trim_start_matches('=').to_string(),
                },
            };
            add(&format!("rhs{}", k + 1), rhs);
        }
        add("neg", if m.nonneg { "1" } else { "2" }.to_string());
        add("eng", match m.method { Method::Grg => "1", Method::Simplex => "2", Method::Evolutionary => "3" }.to_string());
        add("itr", m.max_iter.to_string());
        add("pre", crate::xlsx::num_text(m.precision));
        add("tol", crate::xlsx::num_text(m.int_gap / 100.0));
        add("ver", "3".into());
        add("est", "1".into());
        names.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.scope.cmp(&b.scope)));
        self.begin("Solver Parameters", s);
        self.record(Delta::Names(names));
        self.commit();
        Ok(())
    }

    /// Solve: the variable cells' best values (not put in the sheet: see `Outcome`).
    pub fn solve(&mut self, s: usize, m: &Model) -> Outcome {
        let cells: Vec<(u32, u32)> = m.vars.iter().flat_map(|r| (r.r1..=r.r2).flat_map(move |rr| (r.c1..=r.c2).map(move |cc| (rr, cc)))).collect();
        let start: Vec<f64> = cells.iter().map(|(r, c)| match self.value(s, *r, *c) { Value::Num(x) => x, _ => 0.0 }).collect();
        let mut out = Outcome { status: Status::Infeasible, start: start.clone(), x: start.clone(), objective_start: 0.0, objective: 0.0, cells: cells.clone() };
        let obj = match m.objective {
            Some(o) => o,
            None if matches!(m.goal, Goal::Value(_)) => {
                out.status = Status::Error("Objective Cell must be a single cell on the active sheet.".into());
                return out;
            }
            None => (u32::MAX, u32::MAX),
        };
        if cells.is_empty() {
            out.status = Status::Error("By Changing Variable Cells must be cells on the active sheet.".into());
            return out;
        }
        if cells.len() > 200 {
            out.status = Status::Error("There are too many Variable Cells (the limit is 200).".into());
            return out;
        }
        for (r, c) in &cells {
            if self.sheets[s].get(*r, *c).map(|x| x.f.is_some()).unwrap_or(false) {
                out.status = Status::Error("By Changing Variable Cells must be constants, not formulas.".into());
                return out;
            }
        }
        let mut p = Problem::new(self, s, m, cells.clone(), obj);
        out.objective_start = p.eval(self, &start).0;
        let r = match m.method {
            Method::Simplex => p.simplex(self, &start),
            Method::Grg => p.branch(self, &start, false),
            Method::Evolutionary => p.branch(self, &start, true),
        };
        out.status = r.0;
        out.x = r.1;
        out.objective = p.eval(self, &out.x).0;
        out
    }
}

// ---------------------------------------------------------------------- the problem

struct Row {
    /// the left side's cells, the right side (cells or a constant), the relation
    lhs: Vec<Key>,
    rhs: Rhs,
    rel: Rel,
}

enum Rhs {
    Num(f64),
    Cells(Vec<Key>),
}

struct Problem {
    s: usize,
    cells: Vec<(u32, u32)>,
    obj: Option<Key>,
    goal: Goal,
    rows: Vec<Row>,
    ints: Vec<bool>,
    bins: Vec<bool>,
    nonneg: bool,
    precision: f64,
    max_iter: u32,
    int_gap: f64,
    outputs: Vec<Key>,
}

impl Problem {
    fn new(b: &Book, s: usize, m: &Model, cells: Vec<(u32, u32)>, obj: (u32, u32)) -> Problem {
        let mut rows = Vec::new();
        let mut ints = vec![false; cells.len()];
        let mut bins = vec![false; cells.len()];
        for c in &m.constraints {
            let lhs: Vec<Key> = (c.lhs.r1..=c.lhs.r2).flat_map(|r| (c.lhs.c1..=c.lhs.c2).map(move |cc| (s, r, cc))).collect();
            match c.rel {
                Rel::Int | Rel::Bin => {
                    for k in &lhs {
                        if let Some(i) = cells.iter().position(|x| (s, x.0, x.1) == *k) {
                            ints[i] = true;
                            if c.rel == Rel::Bin {
                                bins[i] = true;
                            }
                        }
                    }
                }
                Rel::Dif => {}
                _ => {
                    let rhs = match c.rhs.trim().parse::<f64>() {
                        Ok(v) => Rhs::Num(v),
                        Err(_) => match parse_range(&c.rhs.replace('$', "")) {
                            Some(r) => Rhs::Cells((r.r1..=r.r2).flat_map(|rr| (r.c1..=r.c2).map(move |cc| (s, rr, cc))).collect()),
                            None => Rhs::Num(0.0),
                        },
                    };
                    rows.push(Row { lhs, rhs, rel: c.rel });
                }
            }
        }
        let obj = if obj.0 == u32::MAX { None } else { Some((s, obj.0, obj.1)) };
        let mut outputs: Vec<Key> = obj.into_iter().collect();
        for r in &rows {
            outputs.extend(r.lhs.iter().copied());
            if let Rhs::Cells(c) = &r.rhs {
                outputs.extend(c.iter().copied());
            }
        }
        let _ = b;
        Problem { s, cells, obj, goal: m.goal, rows, ints, bins, nonneg: m.nonneg, precision: m.precision.max(1e-12),
                  max_iter: m.max_iter.max(1), int_gap: m.int_gap, outputs }
    }

    fn n(&self) -> usize {
        self.cells.len()
    }

    /// The objective (as minimised: Max is negated; Value Of is 0) and the constraint
    /// values g (g <= 0 for <=, g >= 0 for >=, g = 0 for =) for many points at once.
    fn eval_many(&self, b: &mut Book, xs: &[Vec<f64>]) -> Vec<(f64, Vec<(f64, Rel)>)> {
        let runs: Vec<Vec<(Key, Value)>> = xs.iter().map(|x| {
            let mut x2 = x.clone();
            for i in 0..x2.len() {
                if self.bins[i] {
                    x2[i] = x2[i].clamp(0.0, 1.0);
                }
            }
            self.cells.iter().zip(x2).map(|((r, c), v)| ((self.s, *r, *c), Value::num(v))).collect()
        }).collect();
        let vals = b.what_if(&runs, &self.outputs);
        vals.into_iter().map(|v| {
            let num = |x: &Value| match x {
                Value::Num(n) => *n,
                Value::Bool(t) => if *t { 1.0 } else { 0.0 },
                Value::Empty => 0.0,
                _ => f64::NAN,
            };
            let mut k = 0;
            let mut o = 0.0;
            if self.obj.is_some() {
                o = num(&v[0]);
                k = 1;
            }
            let o = match self.goal {
                Goal::Max => -o,
                Goal::Min => o,
                Goal::Value(_) => 0.0,
            };
            let mut g = Vec::new();
            let mut gv: Vec<(f64, Rel)> = Vec::new();
            if let (Goal::Value(t), Some(_)) = (self.goal, self.obj) {
                gv.push((num(&v[0]) - t, Rel::Eq));
            }
            for row in &self.rows {
                let l: Vec<f64> = (0..row.lhs.len()).map(|i| num(&v[k + i])).collect();
                k += row.lhs.len();
                let r: Vec<f64> = match &row.rhs {
                    Rhs::Num(x) => vec![*x; l.len()],
                    Rhs::Cells(c) => {
                        let rr: Vec<f64> = (0..c.len()).map(|i| num(&v[k + i])).collect();
                        k += c.len();
                        if rr.len() == 1 { vec![rr[0]; l.len()] } else { rr }
                    }
                };
                for (a, bb) in l.iter().zip(r.iter()) {
                    g.push((a - bb, row.rel));
                }
            }
            gv.extend(g);
            (o, gv)
        }).collect()
    }

    fn eval(&self, b: &mut Book, x: &[f64]) -> (f64, Vec<(f64, Rel)>) {
        let o = self.eval_many(b, &[x.to_vec()]).pop().unwrap();
        // the objective as the cell shows it
        let shown = match self.goal {
            Goal::Max => -o.0,
            Goal::Min => o.0,
            Goal::Value(_) => o.0,
        };
        (shown, o.1)
    }

    fn violation(&self, g: &[(f64, Rel)]) -> f64 {
        g.iter().map(|(v, r)| match r {
            Rel::Le => v.max(0.0),
            Rel::Ge => (-v).max(0.0),
            Rel::Eq => v.abs(),
            _ => 0.0,
        }).map(|v| if v.is_nan() { 1e30 } else { v * v }).sum()
    }

    fn feasible(&self, g: &[(f64, Rel)], x: &[f64]) -> bool {
        let tol = self.precision.max(1e-9) * 10.0;
        let ok = g.iter().all(|(v, r)| match r {
            Rel::Le => *v <= tol,
            Rel::Ge => *v >= -tol,
            Rel::Eq => v.abs() <= tol.max(1e-6),
            _ => true,
        });
        ok && (!self.nonneg || x.iter().all(|v| *v >= -tol)) && x.iter().zip(&self.ints).all(|(v, i)| !*i || (v - v.round()).abs() < 1e-6)
    }

    /// The constraints met, integers aside (a branch-and-bound subproblem).
    fn feasible_cont(&self, g: &[(f64, Rel)], x: &[f64]) -> bool {
        let tol = self.precision.max(1e-9) * 10.0;
        g.iter().all(|(v, r)| match r {
            Rel::Le => *v <= tol,
            Rel::Ge => *v >= -tol,
            Rel::Eq => v.abs() <= tol.max(1e-6),
            _ => true,
        }) && (!self.nonneg || x.iter().all(|v| *v >= -tol))
    }

    // -------------------------------------------------------------- Simplex LP

    /// The model as a linear program (its coefficients from the start and a step along each
    /// variable), or why it isn't one.
    fn linearize(&self, b: &mut Book, start: &[f64]) -> Result<Lp, Status> {
        let n = self.n();
        // the model at the start and one step along each variable gives its coefficients
        let base: Vec<f64> = start.to_vec();
        let mut pts = vec![base.clone()];
        for i in 0..n {
            let mut p = base.clone();
            p[i] += 1.0;
            pts.push(p);
        }
        // and a point elsewhere to check it is linear
        let check: Vec<f64> = base.iter().enumerate().map(|(i, v)| v + 1.7 + 0.37 * i as f64).collect();
        pts.push(check.clone());
        let e = self.eval_many(b, &pts);
        if e.iter().any(|(o, g)| o.is_nan() || g.iter().any(|(v, _)| v.is_nan())) {
            return Err(Status::Error("Solver encountered an error value in a target or constraint cell.".into()));
        }
        let m = e[0].1.len();
        let c: Vec<f64> = (0..n).map(|i| e[i + 1].0 - e[0].0).collect();
        let c0 = e[0].0 - c.iter().zip(&base).map(|(a, x)| a * x).sum::<f64>();
        let a: Vec<Vec<f64>> = (0..m).map(|j| (0..n).map(|i| e[i + 1].1[j].0 - e[0].1[j].0).collect()).collect();
        let a0: Vec<f64> = (0..m).map(|j| e[0].1[j].0 - a[j].iter().zip(&base).map(|(p, x)| p * x).sum::<f64>()).collect();
        // linear?
        let ck = &e[n + 1];
        let pred_o = c0 + c.iter().zip(&check).map(|(p, x)| p * x).sum::<f64>();
        let scale = |v: f64| 1.0 + v.abs();
        if (pred_o - ck.0).abs() > 1e-6 * scale(ck.0) {
            return Err(Status::NotLinear);
        }
        for j in 0..m {
            let pred = a0[j] + a[j].iter().zip(&check).map(|(p, x)| p * x).sum::<f64>();
            if (pred - ck.1[j].0).abs() > 1e-6 * scale(ck.1[j].0) {
                return Err(Status::NotLinear);
            }
        }
        let rels: Vec<Rel> = e[0].1.iter().map(|(_, r)| *r).collect();
        Ok(Lp { c, a, b: a0.iter().map(|v| -v).collect(), rels, nonneg: self.nonneg, ints: self.ints.clone(), bins: self.bins.clone() })
    }

    fn simplex(&mut self, b: &mut Book, start: &[f64]) -> (Status, Vec<f64>) {
        match self.linearize(b, start) {
            Err(st) => (st, start.to_vec()),
            Ok(lp) => match lp.solve_int(self.int_gap) {
                LpResult::Optimal(x) => (Status::Optimal, x),
                LpResult::Infeasible => (Status::Infeasible, start.to_vec()),
                LpResult::Unbounded => (Status::Unbounded, start.to_vec()),
            },
        }
    }

    // -------------------------------------------------------------- GRG / Evolutionary

    /// Integer variables by branch and bound over the continuous method.
    fn branch(&mut self, b: &mut Book, start: &[f64], evo: bool) -> (Status, Vec<f64>) {
        let n = self.n();
        if !evo {
            // a model that turns out linear is solved exactly
            if let Ok(lp) = self.linearize(b, start) {
                return match lp.solve_int(self.int_gap) {
                    LpResult::Optimal(x) => (Status::Optimal, x),
                    LpResult::Infeasible => (Status::Infeasible, start.to_vec()),
                    LpResult::Unbounded => (Status::Unbounded, start.to_vec()),
                };
            }
        }
        let mut lo: Vec<f64> = vec![if self.nonneg { 0.0 } else { f64::NEG_INFINITY }; n];
        let mut hi: Vec<f64> = vec![f64::INFINITY; n];
        for i in 0..n {
            if self.bins[i] {
                lo[i] = 0.0;
                hi[i] = 1.0;
            }
        }
        if !self.ints.iter().any(|x| *x) {
            return if evo { self.evolve(b, start, &lo, &hi) } else { self.grg(b, start, &lo, &hi) };
        }
        let mut best: Option<(f64, Vec<f64>)> = None;
        let mut stack = vec![(lo, hi)];
        let mut nodes = 0;
        while let Some((lo, hi)) = stack.pop() {
            nodes += 1;
            if nodes > 200 {
                break;
            }
            let x0: Vec<f64> = start.iter().enumerate().map(|(i, v)| v.clamp(lo[i], hi[i])).collect();
            let (st, x) = if evo { self.evolve(b, &x0, &lo, &hi) } else { self.grg(b, &x0, &lo, &hi) };
            if !st.found() {
                continue;
            }
            let (o, _) = self.eval(b, &x);
            let om = match self.goal { Goal::Max => -o, _ => o };
            if let Some((bo, _)) = &best {
                if om >= *bo - self.int_gap / 100.0 * bo.abs() {
                    continue;
                }
            }
            match (0..n).filter(|i| self.ints[*i] && (x[*i] - x[*i].round()).abs() > 1e-6).max_by(|a, bb| {
                let fa = (x[*a] - x[*a].round()).abs();
                let fb = (x[*bb] - x[*bb].round()).abs();
                fa.partial_cmp(&fb).unwrap()
            }) {
                None => {
                    let xr: Vec<f64> = x.iter().enumerate().map(|(i, v)| if self.ints[i] { v.round() } else { *v }).collect();
                    let g = self.eval(b, &xr).1;
                    if self.feasible(&g, &xr) {
                        best = Some((om, xr));
                    }
                }
                Some(i) => {
                    let (mut l2, mut h2) = (lo.clone(), hi.clone());
                    h2[i] = x[i].floor();
                    l2[i] = x[i].ceil();
                    stack.push((lo.clone(), h2));
                    let mut lo3 = lo.clone();
                    lo3[i] = l2[i];
                    stack.push((lo3, hi.clone()));
                }
            }
        }
        match best {
            Some((_, x)) => (Status::Optimal, x),
            None => (Status::Infeasible, start.to_vec()),
        }
    }

    /// The augmented Lagrangian (constraints as c <= 0 and h = 0).
    fn lagrangian(&self, o: f64, g: &[(f64, Rel)], lam: &[f64], mu: f64) -> f64 {
        if o.is_nan() {
            return f64::INFINITY;
        }
        let mut v = o;
        for (k, (gv, rel)) in g.iter().enumerate() {
            if gv.is_nan() {
                return f64::INFINITY;
            }
            let l = lam.get(k).copied().unwrap_or(0.0);
            match rel {
                Rel::Le | Rel::Ge => {
                    let c = if *rel == Rel::Le { *gv } else { -gv };
                    let t = (c + l / mu).max(0.0);
                    v += mu / 2.0 * t * t - l * l / (2.0 * mu);
                }
                Rel::Eq => v += l * gv + mu / 2.0 * gv * gv,
                _ => {}
            }
        }
        v
    }

    fn penalised(&self, o: f64, g: &[(f64, Rel)], mu: f64) -> f64 {
        if o.is_nan() {
            return f64::INFINITY;
        }
        o + mu * self.violation(g)
    }

    /// A penalty method with quasi-Newton (BFGS) steps and forward-difference gradients.
    fn grg(&self, b: &mut Book, start: &[f64], lo: &[f64], hi: &[f64]) -> (Status, Vec<f64>) {
        let n = self.n();
        let clip = |x: &mut Vec<f64>| {
            for i in 0..n {
                x[i] = x[i].clamp(lo[i], hi[i]);
            }
        };
        let mut x = start.to_vec();
        clip(&mut x);
        let mut mu = 10.0;
        let mut iters = 0u32;
        let mut last = f64::INFINITY;
        let m0 = self.eval_many(b, &[x.clone()]).pop().map(|e| e.1.len()).unwrap_or(0);
        let mut lam = vec![0.0; m0];
        let mut last_viol = f64::INFINITY;
        for _outer in 0..20 {
            let lam_now = lam.clone();
            let f = |b: &mut Book, pts: &[Vec<f64>]| -> Vec<f64> {
                self.eval_many(b, pts).into_iter().map(|(o, g)| self.lagrangian(o, &g, &lam_now, mu)).collect()
            };
            let mut h: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
            let grad = |b: &mut Book, x: &[f64]| -> (f64, Vec<f64>) {
                let mut pts = vec![x.to_vec()];
                let hs: Vec<f64> = x.iter().map(|v| 1e-6 * v.abs().max(1.0)).collect();
                for i in 0..n {
                    let mut p = x.to_vec();
                    p[i] += hs[i];
                    pts.push(p);
                }
                let v = f(b, &pts);
                (v[0], (0..n).map(|i| (v[i + 1] - v[0]) / hs[i]).collect())
            };
            let (mut fx, mut gx) = grad(b, &x);
            for _ in 0..200 {
                iters += 1;
                if iters > self.max_iter * 20 {
                    break;
                }
                let gn: f64 = gx.iter().map(|v| v * v).sum::<f64>().sqrt();
                if !gn.is_finite() || gn < 1e-9 {
                    break;
                }
                // direction -H g, projected at the bounds
                let mut d: Vec<f64> = (0..n).map(|i| -(0..n).map(|j| h[i][j] * gx[j]).sum::<f64>()).collect();
                for i in 0..n {
                    if (x[i] <= lo[i] && d[i] < 0.0) || (x[i] >= hi[i] && d[i] > 0.0) {
                        d[i] = 0.0;
                    }
                }
                if d.iter().zip(&gx).map(|(a, bb)| a * bb).sum::<f64>() >= 0.0 {
                    d = gx.iter().map(|v| -v).collect();
                    h = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
                }
                // backtracking along d: several step lengths at once
                let steps: Vec<f64> = (0..12).map(|k| 0.5f64.powi(k)).collect();
                let pts: Vec<Vec<f64>> = steps.iter().map(|t| {
                    let mut p: Vec<f64> = x.iter().zip(&d).map(|(a, bb)| a + t * bb).collect();
                    clip(&mut p);
                    p
                }).collect();
                let vals = f(b, &pts);
                let mut k = None;
                for (i, v) in vals.iter().enumerate() {
                    if *v < fx - 1e-4 * steps[i] * gn * gn.min(1.0) || (*v < fx && i == vals.len() - 1) {
                        k = Some(i);
                        break;
                    }
                }
                let k = match k {
                    Some(k) => k,
                    None => {
                        let fresh = h.iter().enumerate().all(|(i, row)| row.iter().enumerate().all(|(j, v)| *v == if i == j { 1.0 } else { 0.0 }));
                        if fresh {
                            break;
                        }
                        h = (0..n).map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
                        continue;
                    }
                };
                let xn = pts[k].clone();
                let (fnew, gnew) = grad(b, &xn);
                let sv: Vec<f64> = xn.iter().zip(&x).map(|(a, bb)| a - bb).collect();
                let yv: Vec<f64> = gnew.iter().zip(&gx).map(|(a, bb)| a - bb).collect();
                let sy: f64 = sv.iter().zip(&yv).map(|(a, bb)| a * bb).sum();
                if sy > 1e-12 {
                    // BFGS update of the inverse Hessian
                    let hy: Vec<f64> = (0..n).map(|i| (0..n).map(|j| h[i][j] * yv[j]).sum()).collect();
                    let yhy: f64 = yv.iter().zip(&hy).map(|(a, bb)| a * bb).sum();
                    for i in 0..n {
                        for j in 0..n {
                            h[i][j] += (sy + yhy) * sv[i] * sv[j] / (sy * sy) - (hy[i] * sv[j] + sv[i] * hy[j]) / sy;
                        }
                    }
                }
                let done = (fx - fnew).abs() <= self.precision * (1.0 + fx.abs()) && sv.iter().map(|v| v.abs()).fold(0.0, f64::max) < 1e-7;
                x = xn;
                fx = fnew;
                gx = gnew;
                if done {
                    break;
                }
            }
            let (o, g) = self.eval_many(b, &[x.clone()]).pop().unwrap();
            if !o.is_finite() && o.is_sign_negative() {
                return (Status::Unbounded, x);
            }
            let v = self.violation(&g).sqrt();
            if v <= self.precision.max(1e-9) && (o - last).abs() <= self.precision * (1.0 + o.abs()) {
                break;
            }
            last = o;
            if o < -1e15 {
                return (Status::Unbounded, x);
            }
            // the multipliers move; the penalty grows when the constraints aren't getting closer
            for (k, (gv, rel)) in g.iter().enumerate() {
                if k >= lam.len() {
                    break;
                }
                match rel {
                    Rel::Le => lam[k] = (lam[k] + mu * gv).max(0.0),
                    Rel::Ge => lam[k] = (lam[k] - mu * gv).max(0.0),
                    Rel::Eq => lam[k] += mu * gv,
                    _ => {}
                }
            }
            if v > 0.25 * last_viol {
                mu = (mu * 10.0).min(1e10);
            }
            last_viol = v;
        }
        let g = self.eval(b, &x).1;
        if self.feasible_cont(&g, &x) {
            (Status::Converged, x)
        } else {
            (Status::Infeasible, x)
        }
    }

    /// Differential evolution within the bounds (every variable needs both).
    fn evolve(&self, b: &mut Book, start: &[f64], lo0: &[f64], hi0: &[f64]) -> (Status, Vec<f64>) {
        let n = self.n();
        // bounds from the constraints on the variable cells themselves
        let mut lo = lo0.to_vec();
        let mut hi = hi0.to_vec();
        let e = self.eval_many(b, &[start.to_vec()]).pop().unwrap();
        let mut gi = if matches!(self.goal, Goal::Value(_)) && self.obj.is_some() { 1 } else { 0 };
        for row in &self.rows {
            for k in &row.lhs {
                if let Some(i) = self.cells.iter().position(|c| (self.s, c.0, c.1) == *k) {
                    let val = start[i] - e.1[gi].0;
                    match row.rel {
                        Rel::Le => hi[i] = hi[i].min(val),
                        Rel::Ge => lo[i] = lo[i].max(val),
                        Rel::Eq => {
                            lo[i] = val;
                            hi[i] = val;
                        }
                        _ => {}
                    }
                }
                gi += 1;
            }
        }
        if (0..n).any(|i| !lo[i].is_finite() || !hi[i].is_finite()) {
            return (Status::Error("All variables must have both upper and lower bounds.".into()), start.to_vec());
        }
        let np = (10 * n).clamp(20, 60);
        let mut seed: u64 = 0x9E3779B97F4A7C15;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let round_ints = |x: &mut Vec<f64>| {
            for i in 0..n {
                if self.ints[i] {
                    x[i] = x[i].round();
                }
            }
        };
        let mut pop: Vec<Vec<f64>> = (0..np).map(|k| {
            if k == 0 {
                start.iter().enumerate().map(|(i, v)| v.clamp(lo[i], hi[i])).collect()
            } else {
                (0..n).map(|i| lo[i] + rnd() * (hi[i] - lo[i])).collect()
            }
        }).collect();
        for p in pop.iter_mut() {
            round_ints(p);
        }
        let mu = 1e6;
        let score = |b: &mut Book, ps: &[Vec<f64>]| -> Vec<f64> { self.eval_many(b, ps).into_iter().map(|(o, g)| self.penalised(o, &g, mu)).collect() };
        let mut fit = score(b, &pop);
        let gens = (self.max_iter * 3).clamp(60, 400);
        let mut stall = 0;
        let mut best = fit.iter().cloned().fold(f64::INFINITY, f64::min);
        for _ in 0..gens {
            let mut trials = Vec::with_capacity(np);
            for k in 0..np {
                let pick = |r: f64| ((r * np as f64) as usize).min(np - 1);
                let (a, bb, c) = (pick(rnd()), pick(rnd()), pick(rnd()));
                let jr = pick(rnd()) % n.max(1);
                let mut t = pop[k].clone();
                for i in 0..n {
                    if rnd() < 0.9 || i == jr {
                        t[i] = (pop[a][i] + 0.6 * (pop[bb][i] - pop[c][i])).clamp(lo[i], hi[i]);
                    }
                }
                round_ints(&mut t);
                trials.push(t);
            }
            let tf = score(b, &trials);
            for k in 0..np {
                if tf[k] <= fit[k] {
                    pop[k] = trials[k].clone();
                    fit[k] = tf[k];
                }
            }
            let nb = fit.iter().cloned().fold(f64::INFINITY, f64::min);
            if nb < best - 1e-9 * (1.0 + best.abs()) {
                best = nb;
                stall = 0;
            } else {
                stall += 1;
                if stall > 40 {
                    break;
                }
            }
        }
        let k = (0..np).min_by(|a, bb| fit[*a].partial_cmp(&fit[*bb]).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(0);
        let x = pop[k].clone();
        let g = self.eval(b, &x).1;
        if self.feasible_cont(&g, &x) {
            (Status::Converged, x)
        } else {
            (Status::Infeasible, x)
        }
    }
}

// ---------------------------------------------------------------------- linear programs

enum LpResult {
    Optimal(Vec<f64>),
    Infeasible,
    Unbounded,
}

/// minimise c·x subject to a_j·x (rel_j) b_j, x >= 0 when nonneg (else free)
struct Lp {
    c: Vec<f64>,
    a: Vec<Vec<f64>>,
    b: Vec<f64>,
    rels: Vec<Rel>,
    nonneg: bool,
    ints: Vec<bool>,
    bins: Vec<bool>,
}

impl Lp {
    fn solve_int(&self, gap: f64) -> LpResult {
        let n = self.c.len();
        let mut lo = vec![if self.nonneg { 0.0 } else { f64::NEG_INFINITY }; n];
        let mut hi = vec![f64::INFINITY; n];
        for i in 0..n {
            if self.bins[i] {
                lo[i] = 0.0;
                hi[i] = 1.0;
            }
        }
        let mut best: Option<(f64, Vec<f64>)> = None;
        let mut stack = vec![(lo, hi)];
        let mut unbounded = false;
        let mut nodes = 0;
        while let Some((lo, hi)) = stack.pop() {
            nodes += 1;
            if nodes > 5000 {
                break;
            }
            match self.solve_bounded(&lo, &hi) {
                LpResult::Optimal(x) => {
                    let v: f64 = self.c.iter().zip(&x).map(|(a, b)| a * b).sum();
                    if let Some((bv, _)) = &best {
                        if v >= *bv - gap / 100.0 * bv.abs() - 1e-12 {
                            continue;
                        }
                    }
                    let frac = (0..n).filter(|i| self.ints[*i] && (x[*i] - x[*i].round()).abs() > 1e-7)
                        .max_by(|a, b| (x[*a] - x[*a].round()).abs().partial_cmp(&(x[*b] - x[*b].round()).abs()).unwrap());
                    match frac {
                        None => {
                            let xr = x.iter().enumerate().map(|(i, v)| if self.ints[i] { v.round() } else { *v }).collect();
                            best = Some((v, xr));
                        }
                        Some(i) => {
                            let mut h2 = hi.clone();
                            h2[i] = x[i].floor();
                            let mut l2 = lo.clone();
                            l2[i] = x[i].ceil();
                            stack.push((lo.clone(), h2));
                            stack.push((l2, hi.clone()));
                        }
                    }
                }
                LpResult::Unbounded => {
                    if best.is_none() {
                        unbounded = true;
                    }
                }
                LpResult::Infeasible => {}
            }
        }
        match best {
            Some((_, x)) => LpResult::Optimal(x),
            None if unbounded => LpResult::Unbounded,
            None => LpResult::Infeasible,
        }
    }

    /// Two-phase simplex (Bland's rule) with the variable bounds as rows; free variables split.
    fn solve_bounded(&self, lo: &[f64], hi: &[f64]) -> LpResult {
        let n = self.c.len();
        // x_i = lo_i + y_i (y >= 0) when lo is finite, else x_i = y+ - y-
        let mut cols: Vec<(usize, f64, f64)> = Vec::new(); // (var, sign, offset)
        for i in 0..n {
            if lo[i].is_finite() {
                cols.push((i, 1.0, lo[i]));
            } else {
                cols.push((i, 1.0, 0.0));
                cols.push((i, -1.0, 0.0));
            }
        }
        let nc = cols.len();
        // rows: a x (rel) b  ->  in y
        let mut rows: Vec<(Vec<f64>, Rel, f64)> = Vec::new();
        for (j, aj) in self.a.iter().enumerate() {
            let mut r = vec![0.0; nc];
            let mut shift = 0.0;
            for (k, (i, sg, off)) in cols.iter().enumerate() {
                r[k] = aj[*i] * sg;
                if *sg > 0.0 {
                    shift += aj[*i] * off;
                }
            }
            // offsets counted once per variable
            let shift: f64 = (0..n).map(|i| if lo[i].is_finite() { aj[i] * lo[i] } else { 0.0 }).sum::<f64>() + 0.0 * shift;
            rows.push((r, self.rels[j], self.b[j] - shift));
        }
        for i in 0..n {
            if hi[i].is_finite() {
                let mut r = vec![0.0; nc];
                for (k, (v, sg, _)) in cols.iter().enumerate() {
                    if *v == i {
                        r[k] = *sg;
                    }
                }
                let off = if lo[i].is_finite() { lo[i] } else { 0.0 };
                rows.push((r, Rel::Le, hi[i] - off));
            }
            if lo[i].is_finite() && hi[i] < lo[i] {
                return LpResult::Infeasible;
            }
        }
        let mut cost = vec![0.0; nc];
        for (k, (i, sg, _)) in cols.iter().enumerate() {
            cost[k] = self.c[*i] * sg;
        }
        match simplex(&cost, &rows) {
            LpResult::Optimal(y) => {
                let mut x = vec![0.0; n];
                for i in 0..n {
                    if lo[i].is_finite() {
                        x[i] = lo[i];
                    }
                }
                for (k, (i, sg, _)) in cols.iter().enumerate() {
                    x[*i] += sg * y[k];
                }
                LpResult::Optimal(x)
            }
            other => other,
        }
    }
}

/// minimise cost·y, rows (r·y rel b), y >= 0. Dense tableau, two phases.
fn simplex(cost: &[f64], rows: &[(Vec<f64>, Rel, f64)]) -> LpResult {
    let n = cost.len();
    let m = rows.len();
    // normalise b >= 0
    let mut rs: Vec<(Vec<f64>, Rel, f64)> = rows.iter().map(|(r, rel, b)| {
        if *b < 0.0 {
            let rel2 = match rel { Rel::Le => Rel::Ge, Rel::Ge => Rel::Le, o => *o };
            (r.iter().map(|v| -v).collect(), rel2, -b)
        } else {
            (r.clone(), *rel, *b)
        }
    }).collect();
    // columns: n originals, slacks/surpluses, artificials
    let n_slack = rs.iter().filter(|(_, r, _)| matches!(r, Rel::Le | Rel::Ge)).count();
    let n_art = rs.iter().filter(|(_, r, _)| matches!(r, Rel::Ge | Rel::Eq)).count();
    let w = n + n_slack + n_art;
    let mut t = vec![vec![0.0; w + 1]; m];
    let mut basis = vec![0usize; m];
    let (mut si, mut ai) = (n, n + n_slack);
    let mut arts = Vec::new();
    for (j, (r, rel, b)) in rs.iter_mut().enumerate() {
        t[j][..n].copy_from_slice(r);
        t[j][w] = *b;
        match rel {
            Rel::Le => {
                t[j][si] = 1.0;
                basis[j] = si;
                si += 1;
            }
            Rel::Ge => {
                t[j][si] = -1.0;
                si += 1;
                t[j][ai] = 1.0;
                basis[j] = ai;
                arts.push(ai);
                ai += 1;
            }
            _ => {
                t[j][ai] = 1.0;
                basis[j] = ai;
                arts.push(ai);
                ai += 1;
            }
        }
    }
    let run = |t: &mut Vec<Vec<f64>>, basis: &mut Vec<usize>, obj: &[f64], allowed: usize| -> Option<bool> {
        // reduced costs row
        for _ in 0..5000 {
            let mut z = vec![0.0; allowed];
            for k in 0..allowed {
                let mut v = obj[k];
                for j in 0..m {
                    v -= obj[basis[j]] * t[j][k];
                }
                z[k] = v;
            }
            let enter = (0..allowed).find(|k| z[*k] < -1e-9);
            let e = match enter {
                Some(e) => e,
                None => return Some(true),
            };
            let mut leave: Option<usize> = None;
            let mut best = f64::INFINITY;
            for j in 0..m {
                if t[j][e] > 1e-9 {
                    let ratio = t[j][w] / t[j][e];
                    if ratio < best - 1e-12 || (ratio <= best + 1e-12 && leave.map(|l| basis[j] < basis[l]).unwrap_or(true)) {
                        best = ratio;
                        leave = Some(j);
                    }
                }
            }
            let l = match leave {
                Some(l) => l,
                None => return None, // unbounded
            };
            let pv = t[l][e];
            for k in 0..=w {
                t[l][k] /= pv;
            }
            for j in 0..m {
                if j != l && t[j][e].abs() > 0.0 {
                    let f = t[j][e];
                    for k in 0..=w {
                        t[j][k] -= f * t[l][k];
                    }
                }
            }
            basis[l] = e;
        }
        Some(false)
    };
    // phase 1: drive the artificials out
    if !arts.is_empty() {
        let mut obj1 = vec![0.0; w];
        for a in &arts {
            obj1[*a] = 1.0;
        }
        if run(&mut t, &mut basis, &obj1, w).is_none() {
            return LpResult::Infeasible;
        }
        let infeas: f64 = (0..m).filter(|j| arts.contains(&basis[*j])).map(|j| t[j][w]).sum();
        if infeas > 1e-7 {
            return LpResult::Infeasible;
        }
        // artificials still basic at 0: swap them out where possible
        for j in 0..m {
            if arts.contains(&basis[j]) {
                if let Some(k) = (0..n + n_slack).find(|k| t[j][*k].abs() > 1e-9) {
                    let pv = t[j][k];
                    for q in 0..=w {
                        t[j][q] /= pv;
                    }
                    for i in 0..m {
                        if i != j {
                            let f = t[i][k];
                            for q in 0..=w {
                                t[i][q] -= f * t[j][q];
                            }
                        }
                    }
                    basis[j] = k;
                }
            }
        }
    }
    let mut obj2 = vec![0.0; w];
    obj2[..n].copy_from_slice(cost);
    for a in &arts {
        obj2[*a] = 1e9;
    }
    match run(&mut t, &mut basis, &obj2, n + n_slack) {
        None => LpResult::Unbounded,
        Some(_) => {
            let mut y = vec![0.0; n];
            for j in 0..m {
                if basis[j] < n {
                    y[basis[j]] = t[j][w];
                }
            }
            LpResult::Optimal(y)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lp_small() {
        // max 3x + 5y st x <= 4, 2y <= 12, 3x + 2y <= 18  -> x=2, y=6
        let lp = Lp { c: vec![-3.0, -5.0], a: vec![vec![1.0, 0.0], vec![0.0, 2.0], vec![3.0, 2.0]], b: vec![4.0, 12.0, 18.0],
                      rels: vec![Rel::Le; 3], nonneg: true, ints: vec![false; 2], bins: vec![false; 2] };
        match lp.solve_int(0.0) {
            LpResult::Optimal(x) => assert!((x[0] - 2.0).abs() < 1e-7 && (x[1] - 6.0).abs() < 1e-7, "{:?}", x),
            _ => panic!(),
        }
        // integer: max x + y st 2x + 2y <= 5 -> 2
        let lp = Lp { c: vec![-1.0, -1.0], a: vec![vec![2.0, 2.0]], b: vec![5.0], rels: vec![Rel::Le], nonneg: true,
                      ints: vec![true; 2], bins: vec![false; 2] };
        match lp.solve_int(0.0) {
            LpResult::Optimal(x) => assert!((x[0] + x[1] - 2.0).abs() < 1e-9, "{:?}", x),
            _ => panic!(),
        }
        // infeasible: x >= 3, x <= 1
        let lp = Lp { c: vec![1.0], a: vec![vec![1.0], vec![1.0]], b: vec![3.0, 1.0], rels: vec![Rel::Ge, Rel::Le], nonneg: true,
                      ints: vec![false], bins: vec![false] };
        assert!(matches!(lp.solve_int(0.0), LpResult::Infeasible));
        // unbounded: max x
        let lp = Lp { c: vec![-1.0], a: vec![], b: vec![], rels: vec![], nonneg: true, ints: vec![false], bins: vec![false] };
        assert!(matches!(lp.solve_int(0.0), LpResult::Unbounded));
        // free variable, equality: min x st x = -2 (not non-negative)
        let lp = Lp { c: vec![1.0], a: vec![vec![1.0]], b: vec![-2.0], rels: vec![Rel::Eq], nonneg: false, ints: vec![false], bins: vec![false] };
        match lp.solve_int(0.0) {
            LpResult::Optimal(x) => assert!((x[0] + 2.0).abs() < 1e-9),
            _ => panic!(),
        }
    }
}
