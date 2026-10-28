//! Calculation: which formulas depend on which cells, and recalculating what changed in
//! dependency order (formulas reached through INDIRECT, OFFSET or spills are pulled in
//! as they are read).

use crate::book::{Book, Key, Name};
use crate::calc::{Ctx, Eval};
use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::formula::Expr;
use crate::sheet::{FKind, Formula, Sheet};
use crate::value::{Array, Value, XlErr};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

// --------------------------------------------------------------------------- dependencies

#[derive(Default)]
pub struct Deps {
    prec: HashMap<Key, Vec<(usize, Rect)>>,
    single: HashMap<Key, Vec<Key>>,
    slots: Vec<Option<(usize, Rect, Key)>>,
    free: Vec<usize>,
    bucket: HashMap<(usize, u32, u32), Vec<usize>>,
    tall: HashMap<(usize, u32), Vec<usize>>,
    wide: HashMap<(usize, u32), Vec<usize>>,
    huge: Vec<usize>,
    owned: HashMap<Key, Vec<usize>>,
    pub volatile: HashSet<Key>,
    all: HashSet<Key>,
}

const RB: u32 = 8; // rows per bucket: 256
const CB: u32 = 3; // columns per bucket: 8

fn find_name<'a>(names: &'a [Name], name: &str, scope: usize) -> Option<&'a Name> {
    names.iter().find(|n| n.scope == Some(scope) && n.name.eq_ignore_ascii_case(name))
        .or_else(|| names.iter().find(|n| n.scope.is_none() && n.name.eq_ignore_ascii_case(name)))
}

fn sheet_idx(sheets: &[Sheet], name: &str) -> Option<usize> {
    sheets.iter().position(|s| s.name.eq_ignore_ascii_case(name))
}

thread_local! {
    /// the cell whose formula collect_areas is reading (for [@Column] references)
    pub static HERE_CELL: std::cell::Cell<(u32, u32)> = const { std::cell::Cell::new((0, 0)) };
}

/// The areas a formula reads (names expanded).
pub fn collect_areas(e: &Expr, sheets: &[Sheet], names: &[Name], here: usize, out: &mut Vec<(usize, Rect)>, depth: u32) {
    if depth > 20 {
        return;
    }
    match e {
        Expr::Ref(r) => {
            if r.kind == crate::formula::RefKind::Err {
                return;
            }
            let s = match &r.sheet {
                Some(n) => match sheet_idx(sheets, n) {
                    Some(i) => i,
                    None => return,
                },
                None => here,
            };
            let rect = Rect::new(r.r1, r.c1, r.r2, r.c2);
            match &r.sheet2 {
                Some(n2) => {
                    if let Some(j) = sheet_idx(sheets, n2) {
                        for k in s.min(j)..=s.max(j) {
                            out.push((k, rect));
                        }
                    }
                }
                None => out.push((s, rect)),
            }
        }
        Expr::Range(a, b) => {
            let mut x = Vec::new();
            collect_areas(a, sheets, names, here, &mut x, depth + 1);
            collect_areas(b, sheets, names, here, &mut x, depth + 1);
            // A1:B2 written as two references: one rectangle
            if let (Expr::Ref(_), Expr::Ref(_)) = (&**a, &**b) {
                if x.len() == 2 && x[0].0 == x[1].0 {
                    out.push((x[0].0, x[0].1.union(&x[1].1)));
                    return;
                }
            }
            out.extend(x);
        }
        Expr::Name(sheet, n) => {
            let scope = match sheet {
                Some(sn) => match sheet_idx(sheets, sn) {
                    Some(i) => i,
                    None => return,
                },
                None => here,
            };
            if let Some(def) = find_name(names, n, scope) {
                if let Some(x) = &def.expr {
                    collect_areas(x, sheets, names, def.scope.unwrap_or(here), out, depth + 1);
                }
            }
        }
        Expr::Func(_, args) => {
            for a in args {
                collect_areas(a, sheets, names, here, out, depth + 1);
            }
        }
        Expr::Neg(x) | Expr::Plus(x) | Expr::Percent(x) | Expr::Spill(x) | Expr::At(x) => collect_areas(x, sheets, names, here, out, depth + 1),
        Expr::Bin(_, a, b) | Expr::Isect(a, b) => {
            collect_areas(a, sheets, names, here, out, depth + 1);
            collect_areas(b, sheets, names, here, out, depth + 1);
        }
        Expr::Union(v) => {
            for x in v {
                collect_areas(x, sheets, names, here, out, depth + 1);
            }
        }
        Expr::Array(rows) => {
            for x in rows.iter().flatten() {
                collect_areas(x, sheets, names, here, out, depth + 1);
            }
        }
        Expr::Call(f, args) => {
            collect_areas(f, sheets, names, here, out, depth + 1);
            for a in args {
                collect_areas(a, sheets, names, here, out, depth + 1);
            }
        }
        Expr::Struct(text) => {
            let cell = HERE_CELL.with(|h| h.get());
            if let Some(x) = crate::tables::resolve(sheets, text, here, Some(cell)) {
                out.push(x);
            }
        }
        _ => {}
    }
}

fn uses_volatile_name(e: &Expr, sheets: &[Sheet], names: &[Name], here: usize, depth: u32) -> bool {
    if depth > 20 {
        return false;
    }
    match e {
        Expr::Name(sheet, n) => {
            let scope = sheet.as_ref().and_then(|s| sheet_idx(sheets, s)).unwrap_or(here);
            find_name(names, n, scope).and_then(|d| d.expr.as_ref()).map(|x| crate::book::is_volatile(x) || uses_volatile_name(x, sheets, names, here, depth + 1)).unwrap_or(false)
        }
        Expr::Func(_, args) => args.iter().any(|a| uses_volatile_name(a, sheets, names, here, depth + 1)),
        Expr::Neg(x) | Expr::Plus(x) | Expr::Percent(x) | Expr::Spill(x) | Expr::At(x) => uses_volatile_name(x, sheets, names, here, depth + 1),
        Expr::Bin(_, a, b) | Expr::Range(a, b) | Expr::Isect(a, b) => uses_volatile_name(a, sheets, names, here, depth + 1) || uses_volatile_name(b, sheets, names, here, depth + 1),
        Expr::Union(v) => v.iter().any(|x| uses_volatile_name(x, sheets, names, here, depth + 1)),
        Expr::Call(f, args) => uses_volatile_name(f, sheets, names, here, depth + 1) || args.iter().any(|a| uses_volatile_name(a, sheets, names, here, depth + 1)),
        _ => false,
    }
}

impl Deps {
    pub fn formulas(&self) -> impl Iterator<Item = Key> + '_ {
        self.all.iter().copied()
    }

    pub fn rebuild(&mut self, sheets: &[Sheet], names: &[Name]) {
        *self = Deps::default();
        for (s, sh) in sheets.iter().enumerate() {
            for ((r, c), cell) in sh.cells.iter() {
                if cell.f.is_some() {
                    self.add(sheets, names, (s, *r, *c));
                }
            }
        }
    }

    /// The formula at `k` changed (or went away).
    pub fn update_cell(&mut self, sheets: &[Sheet], names: &[Name], k: Key) {
        self.remove(k);
        let has = sheets.get(k.0).and_then(|sh| sh.get(k.1, k.2)).map(|c| c.f.is_some()).unwrap_or(false);
        if has {
            self.add(sheets, names, k);
        }
    }

    fn remove(&mut self, k: Key) {
        if !self.all.remove(&k) {
            return;
        }
        self.volatile.remove(&k);
        if let Some(areas) = self.prec.remove(&k) {
            for (s, rc) in areas {
                if rc.count() == 1 {
                    if let Some(v) = self.single.get_mut(&(s, rc.r1, rc.c1)) {
                        v.retain(|x| *x != k);
                        if v.is_empty() {
                            self.single.remove(&(s, rc.r1, rc.c1));
                        }
                    }
                }
            }
        }
        if let Some(slots) = self.owned.remove(&k) {
            for i in slots {
                if let Some((s, rc, _)) = self.slots[i].take() {
                    self.unplace(i, s, rc);
                    self.free.push(i);
                }
            }
        }
    }

    fn add(&mut self, sheets: &[Sheet], names: &[Name], k: Key) {
        let f: Rc<Formula> = match sheets[k.0].get(k.1, k.2).and_then(|c| c.f.clone()) {
            Some(f) => f,
            None => return,
        };
        self.all.insert(k);
        let mut areas = Vec::new();
        if let Some(e) = &f.expr {
            HERE_CELL.with(|h| h.set((k.1, k.2)));
            collect_areas(e, sheets, names, k.0, &mut areas, 0);
            if f.volatile || uses_volatile_name(e, sheets, names, k.0, 0) {
                self.volatile.insert(k);
            }
        }
        areas.sort_by_key(|(s, r)| (*s, r.r1, r.c1, r.r2, r.c2));
        areas.dedup();
        let mut owned = Vec::new();
        for (s, rc) in &areas {
            if rc.count() == 1 {
                self.single.entry((*s, rc.r1, rc.c1)).or_default().push(k);
            } else {
                let i = match self.free.pop() {
                    Some(i) => {
                        self.slots[i] = Some((*s, *rc, k));
                        i
                    }
                    None => {
                        self.slots.push(Some((*s, *rc, k)));
                        self.slots.len() - 1
                    }
                };
                self.place(i, *s, *rc);
                owned.push(i);
            }
        }
        if !owned.is_empty() {
            self.owned.insert(k, owned);
        }
        self.prec.insert(k, areas);
    }

    fn place(&mut self, i: usize, s: usize, rc: Rect) {
        if rc.rows() >= 65536 && rc.cols() <= 64 {
            for c in rc.c1..=rc.c2 {
                self.tall.entry((s, c)).or_default().push(i);
            }
        } else if rc.cols() >= 1024 && rc.rows() <= 64 {
            for r in rc.r1..=rc.r2 {
                self.wide.entry((s, r)).or_default().push(i);
            }
        } else {
            let n = ((rc.r2 >> RB) - (rc.r1 >> RB) + 1) as u64 * ((rc.c2 >> CB) - (rc.c1 >> CB) + 1) as u64;
            if n <= 4096 {
                for br in (rc.r1 >> RB)..=(rc.r2 >> RB) {
                    for bc in (rc.c1 >> CB)..=(rc.c2 >> CB) {
                        self.bucket.entry((s, br, bc)).or_default().push(i);
                    }
                }
            } else {
                self.huge.push(i);
            }
        }
    }

    fn unplace(&mut self, i: usize, s: usize, rc: Rect) {
        let drop = |v: &mut Vec<usize>| v.retain(|x| *x != i);
        if rc.rows() >= 65536 && rc.cols() <= 64 {
            for c in rc.c1..=rc.c2 {
                if let Some(v) = self.tall.get_mut(&(s, c)) {
                    drop(v);
                }
            }
        } else if rc.cols() >= 1024 && rc.rows() <= 64 {
            for r in rc.r1..=rc.r2 {
                if let Some(v) = self.wide.get_mut(&(s, r)) {
                    drop(v);
                }
            }
        } else {
            let n = ((rc.r2 >> RB) - (rc.r1 >> RB) + 1) as u64 * ((rc.c2 >> CB) - (rc.c1 >> CB) + 1) as u64;
            if n <= 4096 {
                for br in (rc.r1 >> RB)..=(rc.r2 >> RB) {
                    for bc in (rc.c1 >> CB)..=(rc.c2 >> CB) {
                        if let Some(v) = self.bucket.get_mut(&(s, br, bc)) {
                            drop(v);
                        }
                    }
                }
            } else {
                drop(&mut self.huge);
            }
        }
    }

    /// Formulas that read the cell at `p`.
    pub fn dependents(&self, p: Key, out: &mut Vec<Key>) {
        let (s, r, c) = p;
        if let Some(v) = self.single.get(&p) {
            out.extend(v.iter().copied());
        }
        let mut check = |i: &usize| {
            if let Some((ss, rc, k)) = &self.slots[*i] {
                if *ss == s && rc.contains(r, c) {
                    out.push(*k);
                }
            }
        };
        if let Some(v) = self.bucket.get(&(s, r >> RB, c >> CB)) {
            v.iter().for_each(&mut check);
        }
        if let Some(v) = self.tall.get(&(s, c)) {
            v.iter().for_each(&mut check);
        }
        if let Some(v) = self.wide.get(&(s, r)) {
            v.iter().for_each(&mut check);
        }
        self.huge.iter().for_each(&mut check);
    }

    pub fn precedents(&self, k: Key) -> Vec<(usize, Rect)> {
        self.prec.get(&k).cloned().unwrap_or_default()
    }
}

// --------------------------------------------------------------------------- the calculation

impl Book {
    /// Calculate what changed (and volatile formulas). Returns the number of formulas
    /// calculated. `full`: every formula.
    pub fn recalc(&mut self, full: bool) -> usize {
        self.calc_count = 0;
        self.circ.clear();
        let mut seeds: Vec<Key> = std::mem::take(&mut self.changed);
        let mut forced: BTreeSet<Key> = std::mem::take(&mut self.forced);
        if full {
            forced.extend(self.deps.formulas());
        }
        forced.extend(self.deps.volatile.iter().copied());
        let ns = self.sheets.len();
        seeds.retain(|k| k.0 < ns);
        forced.retain(|k| k.0 < ns);
        let mut circular: BTreeSet<Key> = BTreeSet::new();
        let mut computed: HashSet<Key> = HashSet::new();
        for _wave in 0..20 {
            // everything downstream of the seeds
            let mut stack: Vec<Key> = forced.iter().copied().collect();
            forced.clear();
            let mut tmp = Vec::new();
            for p in seeds.drain(..) {
                self.deps.dependents(p, &mut tmp);
            }
            stack.extend(tmp.drain(..));
            let mut todo: BTreeSet<Key> = BTreeSet::new();
            while let Some(k) = stack.pop() {
                if !todo.insert(k) {
                    continue;
                }
                self.downstream(k, &mut tmp);
                stack.extend(tmp.drain(..).filter(|x| !todo.contains(x)));
            }
            if todo.is_empty() {
                break;
            }
            let order = self.order(&todo, &mut circular);
            self.todo = todo;
            self.todo_cols = self.todo.iter().map(|(s, r, c)| (*s, *c, *r)).collect();
            for k in order {
                self.compute(k);
                computed.insert(k);
            }
            self.todo.clear();
            self.todo_cols.clear();
            // spills that moved: the cells they cover changed too
            let wave = std::mem::take(&mut self.wave);
            if wave.is_empty() {
                break;
            }
            seeds = wave;
        }
        // circular references: 0 unless iteration is on
        circular.extend(self.circ_hits.drain());
        if !circular.is_empty() {
            if self.calc.iterate {
                self.iterate(&circular);
            } else {
                for k in &circular {
                    self.set_value(*k, Value::num(0.0));
                }
            }
        }
        self.circ = if self.calc.iterate { vec![] } else { circular.into_iter().collect() };
        self.pending = false;
        self.changed.clear();
        computed.len().max(self.calc_count)
    }

    fn iterate(&mut self, keys: &BTreeSet<Key>) {
        let n = self.calc.count.max(1);
        for i in 0..n {
            let mut change: f64 = 0.0;
            for k in keys {
                let before = self.value(k.0, k.1, k.2);
                if i == 0 {
                    // the first pass already happened in the main calculation
                    continue;
                }
                self.todo.insert(*k);
                self.compute(*k);
                let after = self.value(k.0, k.1, k.2);
                if let (Value::Num(a), Value::Num(b)) = (&before, &after) {
                    change = change.max((a - b).abs());
                } else if before != after {
                    change = f64::INFINITY;
                }
            }
            self.circ_hits.clear();
            if i > 0 && change < self.calc.delta {
                break;
            }
        }
    }

    /// Formulas reading the cells `k`'s formula fills (itself, its spill, its array).
    fn downstream(&self, k: Key, out: &mut Vec<Key>) {
        self.deps.dependents(k, out);
        let sh = &self.sheets[k.0];
        let mut area = None;
        if let Some(rc) = sh.spills.get(&(k.1, k.2)) {
            area = Some(*rc);
        }
        if let Some(rc) = sh.arrays.get(&(k.1, k.2)) {
            area = Some(*rc);
        }
        if let Some(rc) = area {
            if rc.count() <= 1_000_000 {
                for r in rc.r1..=rc.r2 {
                    for c in rc.c1..=rc.c2 {
                        if (r, c) != (k.1, k.2) {
                            self.deps.dependents((k.0, r, c), out);
                        }
                    }
                }
            }
        }
    }

    /// Dependency order of `todo` (precedents first); back edges are circular references.
    fn order(&self, todo: &BTreeSet<Key>, circular: &mut BTreeSet<Key>) -> Vec<Key> {
        let cols: BTreeSet<(usize, u32, u32)> = todo.iter().map(|(s, r, c)| (*s, *c, *r)).collect();
        let mut state: HashMap<Key, u8> = HashMap::with_capacity(todo.len());
        let mut out = Vec::with_capacity(todo.len());
        for &start in todo {
            if state.contains_key(&start) {
                continue;
            }
            // iterative DFS: (node, its precedents in todo, next index)
            let mut stack: Vec<(Key, Vec<Key>, usize)> = vec![(start, self.prec_in(start, todo, &cols), 0)];
            state.insert(start, 1);
            while let Some((node, prec, i)) = stack.last_mut() {
                if *i < prec.len() {
                    let p = prec[*i];
                    *i += 1;
                    match state.get(&p) {
                        None => {
                            state.insert(p, 1);
                            let pp = self.prec_in(p, todo, &cols);
                            stack.push((p, pp, 0));
                        }
                        Some(1) => {
                            // a cycle: everything on the stack from p up
                            let mut on = false;
                            for (n, _, _) in stack.iter() {
                                if *n == p {
                                    on = true;
                                }
                                if on {
                                    circular.insert(*n);
                                }
                            }
                        }
                        _ => {}
                    }
                } else {
                    let n = *node;
                    state.insert(n, 2);
                    out.push(n);
                    stack.pop();
                }
            }
        }
        out
    }

    fn prec_in(&self, k: Key, todo: &BTreeSet<Key>, cols: &BTreeSet<(usize, u32, u32)>) -> Vec<Key> {
        let mut v = Vec::new();
        if let Some(areas) = self.deps.prec.get(&k) {
            for (s, rc) in areas {
                if rc.count() <= 16 {
                    for r in rc.r1..=rc.r2 {
                        for c in rc.c1..=rc.c2 {
                            if todo.contains(&(*s, r, c)) {
                                v.push((*s, r, c));
                            }
                        }
                    }
                } else if rc.cols() <= 64 {
                    for c in rc.c1..=rc.c2 {
                        for (ss, cc, r) in cols.range((*s, c, rc.r1)..=(*s, c, rc.r2)) {
                            v.push((*ss, *r, *cc));
                        }
                    }
                } else {
                    for (ss, r, c) in todo.range((*s, rc.r1, 0)..=(*s, rc.r2, MAX_COLS)) {
                        if *c >= rc.c1 && *c <= rc.c2 {
                            v.push((*ss, *r, *c));
                        }
                    }
                }
            }
        }
        v.retain(|x| *x != k);
        v
    }

    /// Calculate one formula now (if it is waiting).
    pub(crate) fn compute(&mut self, k: Key) {
        if !self.todo.remove(&k) {
            return;
        }
        self.todo_cols.remove(&(k.0, k.2, k.1));
        let f = match self.sheets.get(k.0).and_then(|sh| sh.get(k.1, k.2)).and_then(|c| c.f.clone()) {
            Some(f) => f,
            None => return,
        };
        if let FKind::DataTable(_) = f.kind {
            return;
        }
        self.chain.push(k);
        self.calc_depth += 1;
        let v = match &f.expr {
            Some(e) => {
                let legacy = matches!(f.kind, FKind::Normal);
                let mut ev = Eval::new(self, k.0, k.1, k.2, legacy);
                ev.eval_top(e)
            }
            None => Value::Err(XlErr::Name),
        };
        self.calc_depth -= 1;
        self.chain.pop();
        self.calc_count += 1;
        self.store(k, &f, v);
    }

    fn set_value(&mut self, k: Key, v: Value) {
        if let Some(cell) = self.sheets[k.0].cells.get_mut(&(k.1, k.2)) {
            cell.v = v;
        }
    }

    fn store(&mut self, k: Key, f: &Formula, v: Value) {
        let (s, r, c) = k;
        match &f.kind {
            FKind::Dynamic => match v {
                Value::Arr(a) if a.rows * a.cols > 1 => self.spill(k, a),
                Value::Arr(a) => {
                    self.unspill(k);
                    self.set_value(k, a.data.first().cloned().unwrap_or(Value::Empty).or_zero());
                }
                v => {
                    self.unspill(k);
                    self.set_value(k, v.or_zero());
                }
            },
            FKind::Array(rc) => {
                let rc = *rc;
                let get = |rr: u32, cc: u32| -> Value {
                    match &v {
                        Value::Arr(a) => {
                            let (i, j) = ((rr - rc.r1) as usize, (cc - rc.c1) as usize);
                            let i2 = if a.rows == 1 { 0 } else { i };
                            let j2 = if a.cols == 1 { 0 } else { j };
                            if i2 < a.rows && j2 < a.cols {
                                a.get(i2, j2).clone().or_zero()
                            } else {
                                Value::Err(XlErr::NA)
                            }
                        }
                        x => x.clone().or_zero(),
                    }
                };
                for rr in rc.r1..=rc.r2 {
                    for cc in rc.c1..=rc.c2 {
                        let nv = get(rr, cc);
                        let sh = &mut self.sheets[s];
                        match sh.cells.get_mut(&(rr, cc)) {
                            Some(cell) => {
                                if cell.v != nv {
                                    cell.v = nv;
                                    if (rr, cc) != (r, c) {
                                        self.wave.push((s, rr, cc));
                                    }
                                }
                            }
                            None => {
                                sh.put(rr, cc, Some(crate::sheet::Cell { v: nv, ..Default::default() }));
                                self.wave.push((s, rr, cc));
                            }
                        }
                    }
                }
            }
            FKind::Normal => {
                let v = match v {
                    Value::Arr(a) => a.data.first().cloned().unwrap_or(Value::Empty),
                    v => v,
                };
                self.set_value(k, v.or_zero());
            }
            FKind::DataTable(_) => {}
        }
    }

    fn unspill(&mut self, k: Key) {
        let sh = &mut self.sheets[k.0];
        sh.blocked.remove(&(k.1, k.2));
        if let Some(old) = sh.spills.remove(&(k.1, k.2)) {
            for r in old.r1..=old.r2 {
                for c in old.c1..=old.c2 {
                    if sh.ghosts.remove(&(r, c)).is_some() {
                        self.wave.push((k.0, r, c));
                    }
                }
            }
            self.invalidate_extent(k.0);
        }
    }

    fn spill(&mut self, k: Key, a: Rc<Array>) {
        let (s, r, c) = k;
        let r2 = r as u64 + a.rows as u64 - 1;
        let c2 = c as u64 + a.cols as u64 - 1;
        if r2 >= MAX_ROWS as u64 || c2 >= MAX_COLS as u64 {
            self.unspill(k);
            self.set_value(k, Value::Err(XlErr::Spill));
            return;
        }
        let rc = Rect::new(r, c, r2 as u32, c2 as u32);
        let sh = &self.sheets[s];
        let mut blocked = sh.merges.iter().any(|m| m.intersects(&rc));
        if !blocked {
            blocked = sh.spills.iter().any(|(a2, o)| *a2 != (r, c) && o.intersects(&rc));
        }
        if !blocked {
            for (rr, cc) in sh.positions_in(rc) {
                if (rr, cc) != (r, c) && sh.cells.get(&(rr, cc)).map(|x| !x.is_blank()).unwrap_or(false) {
                    blocked = true;
                    break;
                }
            }
        }
        if blocked {
            self.unspill(k);
            self.sheets[s].blocked.insert((r, c), rc);
            self.set_value(k, Value::Err(XlErr::Spill));
            return;
        }
        let sh = &mut self.sheets[s];
        sh.blocked.remove(&(r, c));
        if let Some(old) = sh.spills.get(&(r, c)).copied() {
            if old != rc {
                for rr in old.r1..=old.r2 {
                    for cc in old.c1..=old.c2 {
                        if !rc.contains(rr, cc) && sh.ghosts.remove(&(rr, cc)).is_some() {
                            self.wave.push((s, rr, cc));
                        }
                    }
                }
            }
        }
        for i in 0..a.rows {
            for j in 0..a.cols {
                let (rr, cc) = (r + i as u32, c + j as u32);
                if (rr, cc) == (r, c) {
                    continue;
                }
                let v = a.get(i, j).clone().or_zero();
                if sh.ghosts.get(&(rr, cc)) != Some(&v) {
                    sh.ghosts.insert((rr, cc), v);
                    self.wave.push((s, rr, cc));
                }
            }
        }
        sh.spills.insert((r, c), rc);
        let first = a.data.first().cloned().unwrap_or(Value::Empty).or_zero();
        self.invalidate_extent(s);
        self.set_value(k, first);
    }

    /// Formula cells holding errors, row by row.
    pub fn error_cells(&self, s: usize) -> Vec<(u32, u32, &'static str)> {
        let mut v = Vec::new();
        if let Some(sh) = self.sheets.get(s) {
            for ((r, c), cell) in sh.cells.iter() {
                if cell.f.is_some() {
                    if let Value::Err(e) = &cell.v {
                        v.push((*r, *c, e.text()));
                    }
                }
            }
        }
        v
    }

    fn ensure(&mut self, s: usize, r: u32, c: u32) {
        let k = (s, r, c);
        if self.chain.is_empty() {
            return;
        }
        if self.chain.contains(&k) {
            // reading a formula that is being calculated: a circular reference
            let mut on = false;
            for x in self.chain.iter() {
                if *x == k {
                    on = true;
                }
                if on {
                    self.circ_hits.insert(*x);
                }
            }
            return;
        }
        if self.todo.is_empty() {
            return;
        }
        if self.todo.contains(&k) && self.calc_depth < 160 {
            self.compute(k);
            return;
        }
        // a spilled value: calculate its anchor first
        let sh = &self.sheets[s];
        if !sh.spills.is_empty() && !sh.cells.contains_key(&(r, c)) {
            let anchors: Vec<(u32, u32)> = sh.spills.iter().filter(|(_, rc)| rc.contains(r, c)).map(|(a, _)| *a).collect();
            for a in anchors {
                if self.todo.contains(&(s, a.0, a.1)) && self.calc_depth < 160 {
                    self.compute((s, a.0, a.1));
                }
            }
        }
    }

    pub fn data_extent_cached(&self, s: usize) -> (u32, u32) {
        let mut e = self.extents.borrow_mut();
        if e.len() < self.sheets.len() {
            e.resize(self.sheets.len(), None);
        }
        if let Some(x) = e[s] {
            return x;
        }
        let x = self.sheets[s].data_extent().unwrap_or((0, 0));
        e[s] = Some(x);
        x
    }
}

trait OrZero {
    fn or_zero(self) -> Value;
}

impl OrZero for Value {
    /// A formula that refers to an empty cell shows 0.
    fn or_zero(self) -> Value {
        match self {
            Value::Empty => Value::num(0.0),
            v => v,
        }
    }
}

// --------------------------------------------------------------------------- what formulas see

#[repr(C)]
struct Tm {
    sec: i32,
    min: i32,
    hour: i32,
    mday: i32,
    mon: i32,
    year: i32,
    wday: i32,
    yday: i32,
    isdst: i32,
    gmtoff: i64,
    zone: *const u8,
}

extern "C" {
    fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
}

/// The local time as a serial date.
pub fn now_serial(date1904: bool) -> f64 {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs() as i64;
    let mut tm = Tm { sec: 0, min: 0, hour: 0, mday: 0, mon: 0, year: 0, wday: 0, yday: 0, isdst: 0, gmtoff: 0, zone: std::ptr::null() };
    let off = unsafe {
        if localtime_r(&secs, &mut tm).is_null() {
            0
        } else {
            tm.gmtoff
        }
    };
    let local = secs + off;
    let days = local.div_euclid(86400);
    let frac = (local.rem_euclid(86400) as f64 + d.subsec_millis() as f64 / 1000.0) / 86400.0;
    // 1970-01-01 is serial 25569 (1900 system)
    let base = if date1904 { 24107.0 } else { 25569.0 };
    base + days as f64 + frac
}

fn cell_format_code(code: &str) -> String {
    let fm = crate::numfmt::get(code);
    let dec = fm.decimals().unwrap_or(0);
    let c = code.to_lowercase();
    if fm.is_general() {
        return "G".into();
    }
    if fm.is_date() {
        if c.contains('y') && c.contains('d') && c.contains('m') && !c.contains("mmm") {
            return if c.contains('h') { "D4".into() } else { "D4".into() };
        }
        if c.contains("d-mmm-yy") || c.contains("dd-mmm-yy") {
            return "D1".into();
        }
        if c.contains("d-mmm") || c.contains("dd-mmm") {
            return "D2".into();
        }
        if c.contains("mmm-yy") {
            return "D3".into();
        }
        if c.contains("am/pm") {
            return if c.contains(":ss") { "D6".into() } else { "D7".into() };
        }
        if c.contains("h") {
            return if c.contains(":ss") { "D8".into() } else { "D9".into() };
        }
        if c.contains("mm/dd") || c.contains("m/d") {
            return "D5".into();
        }
        return "D4".into();
    }
    if fm.is_percent() {
        return format!("P{}", dec);
    }
    if c.contains("e+") {
        return format!("S{}", dec);
    }
    if c.contains('$') {
        return format!("C{}", dec);
    }
    if c.contains("#,##") {
        return format!(",{}", dec);
    }
    format!("F{}", dec)
}

impl Ctx for Book {
    fn external(&self, text: &str) -> Option<Value> {
        self.ext_value(text)
    }
    fn table_ref(&self, text: &str, sheet: usize, r: u32, c: u32) -> Option<(usize, Rect)> {
        crate::tables::resolve(&self.sheets, text, sheet, Some((r, c)))
    }
    fn value(&mut self, sheet: usize, r: u32, c: u32) -> Value {
        if sheet >= self.sheets.len() {
            return Value::Err(XlErr::Ref);
        }
        self.ensure(sheet, r, c);
        self.sheets[sheet].value(r, c)
    }
    fn sheet_index(&self, name: &str) -> Option<usize> {
        Book::sheet_index(self, name)
    }
    fn sheet_name(&self, i: usize) -> String {
        self.sheets.get(i).map(|s| s.name.clone()).unwrap_or_default()
    }
    fn sheet_count(&self) -> usize {
        self.sheets.len()
    }
    fn cells_in(&mut self, sheet: usize, rc: Rect) -> Vec<(u32, u32, Value)> {
        if sheet >= self.sheets.len() {
            return vec![];
        }
        if !self.todo.is_empty() {
            // formulas inside the range that are still waiting (INDIRECT, OFFSET...)
            let mut wait: Vec<Key> = Vec::new();
            if rc.cols() <= 64 {
                for c in rc.c1..=rc.c2 {
                    for (s, cc, r) in self.todo_cols.range((sheet, c, rc.r1)..=(sheet, c, rc.r2)) {
                        wait.push((*s, *r, *cc));
                    }
                }
            } else {
                for (s, r, c) in self.todo.range((sheet, rc.r1, 0)..=(sheet, rc.r2, MAX_COLS)) {
                    if *c >= rc.c1 && *c <= rc.c2 {
                        wait.push((*s, *r, *c));
                    }
                }
            }
            for k in wait {
                self.ensure(k.0, k.1, k.2);
            }
            let anchors: Vec<(u32, u32)> = self.sheets[sheet].spills.iter().filter(|(_, x)| x.intersects(&rc)).map(|(a, _)| *a).collect();
            for a in anchors {
                self.ensure(sheet, a.0, a.1);
            }
        }
        let sh = &self.sheets[sheet];
        let mut out: Vec<(u32, u32, Value)> = Vec::new();
        for (r, c) in sh.positions_in(rc) {
            let cell = &sh.cells[&(r, c)];
            if !cell.is_blank() {
                out.push((r, c, cell.v.clone()));
            }
        }
        if !sh.ghosts.is_empty() && sh.spills.values().any(|x| x.intersects(&rc)) {
            let mut extra = Vec::new();
            for (a, x) in sh.spills.iter() {
                if let Some(i) = x.intersect(&rc) {
                    for r in i.r1..=i.r2 {
                        for c in i.c1..=i.c2 {
                            if (r, c) == *a {
                                continue;
                            }
                            if let Some(v) = sh.ghosts.get(&(r, c)) {
                                if !sh.cells.get(&(r, c)).map(|x| !x.is_blank()).unwrap_or(false) {
                                    extra.push((r, c, v.clone()));
                                }
                            }
                        }
                    }
                }
            }
            if !extra.is_empty() {
                out.extend(extra);
                out.sort_by_key(|(r, c, _)| (*r, *c));
            }
        }
        out
    }
    fn extent(&self, sheet: usize) -> (u32, u32) {
        if sheet >= self.sheets.len() {
            return (0, 0);
        }
        self.data_extent_cached(sheet)
    }
    fn name(&self, name: &str, scope: usize) -> Option<(Rc<Expr>, Option<usize>)> {
        let n = self.find_name(name, scope)?;
        Some((n.expr.clone()?, n.scope))
    }
    fn date1904(&self) -> bool {
        self.date1904
    }
    fn formula_text(&self, sheet: usize, r: u32, c: u32) -> Option<String> {
        let f = self.sheets.get(sheet)?.get(r, c)?.f.as_ref()?;
        Some(format!("={}", f.text))
    }
    fn number_format(&self, sheet: usize, r: u32, c: u32) -> String {
        Book::number_format(self, sheet, r, c)
    }
    fn row_hidden(&self, sheet: usize, r: u32) -> bool {
        self.sheets.get(sheet).map(|s| s.row_hidden(r)).unwrap_or(false)
    }
    fn cell_info(&self, sheet: usize, r: u32, c: u32, what: &str) -> Value {
        let sh = match self.sheets.get(sheet) {
            Some(s) => s,
            None => return Value::Err(XlErr::Ref),
        };
        match what {
            "spill" => match sh.spills.get(&(r, c)) {
                Some(rc) => Value::Arr(Rc::new(Array::new(1, 4, vec![Value::num(rc.r1 as f64), Value::num(rc.c1 as f64), Value::num(rc.r2 as f64), Value::num(rc.c2 as f64)]))),
                None => {
                    if sh.get(r, c).map(|x| x.f.is_some()).unwrap_or(false) {
                        Value::Arr(Rc::new(Array::new(1, 4, vec![Value::num(r as f64), Value::num(c as f64), Value::num(r as f64), Value::num(c as f64)])))
                    } else {
                        Value::Empty
                    }
                }
            },
            "format" => Value::string(cell_format_code(&Book::number_format(self, sheet, r, c))),
            "color" => {
                let code = Book::number_format(self, sheet, r, c);
                let neg = code.split(';').nth(1).map(|s| s.contains('[')).unwrap_or(false);
                Value::num(if neg { 1.0 } else { 0.0 })
            }
            "parentheses" => {
                let code = Book::number_format(self, sheet, r, c);
                Value::num(if code.split(';').next().map(|s| s.contains('(')).unwrap_or(false) { 1.0 } else { 0.0 })
            }
            "prefix" => {
                let v = sh.value(r, c);
                if !matches!(v, Value::Str(_)) {
                    return Value::str("");
                }
                let xf = self.styles.xf(sh.style_at(r, c));
                Value::str(match xf.halign.as_str() {
                    "right" => "\"",
                    "center" => "^",
                    "fill" => "\\",
                    "left" => "'",
                    _ => "'",
                })
            }
            "protect" => Value::num(if self.styles.xf(sh.style_at(r, c)).locked { 1.0 } else { 0.0 }),
            "width" => {
                let mdw = self.mdw(sheet);
                Value::num(crate::sheet::units_to_chars(sh.col_width_units(c, mdw), mdw).round())
            }
            "filename" => {
                match &self.file.path {
                    Some(p) => {
                        let path = std::path::Path::new(p);
                        let dir = path.parent().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                        let f = path.file_name().map(|d| d.to_string_lossy().to_string()).unwrap_or_default();
                        Value::string(format!("{}/[{}]{}", dir, f, sh.name))
                    }
                    None => Value::str(""),
                }
            }
            _ => Value::Err(XlErr::Value),
        }
    }
    fn now(&self) -> f64 {
        self.now_override.unwrap_or_else(|| now_serial(self.date1904))
    }
    fn rand(&mut self) -> f64 {
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        let v = x.wrapping_mul(0x2545F4914F6CDD1D);
        (v >> 11) as f64 / (1u64 << 53) as f64
    }
    fn is_subtotal(&self, sheet: usize, r: u32, c: u32) -> bool {
        match self.sheets.get(sheet).and_then(|s| s.get(r, c)).and_then(|x| x.f.as_ref()) {
            Some(f) => {
                let t = f.text.trim_start().to_uppercase();
                t.starts_with("SUBTOTAL(") || t.starts_with("AGGREGATE(")
            }
            None => false,
        }
    }
}
