//! The workbook: sheets, styles, defined names, undo history, and every edit the
//! window makes. Calculation lives in recalc.rs, files in xlsx.rs, drawing in render.rs.

use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::formula::{self, Adjust, Expr};
use crate::input::{interpret, Typed};
use crate::recalc::Deps;
use crate::sheet::{Cell, ColSpan, FKind, Formula, RowInfo, Sheet};
use crate::styles::Styles;
use crate::value::{Value, XlErr};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

pub type Key = (usize, u32, u32);

/// Cells a single command may create.
pub const CELL_LIMIT: u64 = 2_000_000;

#[derive(Clone, Debug)]
pub struct Name {
    pub name: String,
    pub scope: Option<usize>,
    /// refers-to formula text without '='
    pub text: String,
    pub hidden: bool,
    pub comment: Option<String>,
    /// other attributes of <definedName> (function, vbProcedure, xlm...)
    pub extra: Vec<(String, String)>,
    pub expr: Option<Rc<Expr>>,
}

impl Name {
    pub fn new(name: &str, scope: Option<usize>, text: &str) -> Name {
        Name {
            name: name.to_string(),
            scope,
            text: text.to_string(),
            hidden: false,
            comment: None,
            extra: vec![],
            expr: formula::parse(text).ok().map(Rc::new),
        }
    }
    pub fn reparse(&mut self) {
        self.expr = formula::parse(&self.text).ok().map(Rc::new);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CalcSettings {
    pub manual: bool,
    pub iterate: bool,
    pub count: u32,
    pub delta: f64,
}

impl Default for CalcSettings {
    fn default() -> Self {
        CalcSettings { manual: false, iterate: false, count: 100, delta: 0.001 }
    }
}

/// Sheet settings that undo restores as one piece.
#[derive(Clone, Debug, PartialEq)]
pub struct SheetMeta {
    pub name: String,
    pub state: String,
    pub tab_color: Option<u32>,
    pub default_col_width: Option<f64>,
    pub base_col_width: u32,
    pub default_row_height: f64,
    pub custom_row_height: bool,
}

/// The cell data of a sheet, swapped whole by structural edits.
#[derive(Clone, Debug)]
pub struct Body {
    pub pieces: Vec<crate::sheet::Piece>,
    pub cells: std::collections::BTreeMap<(u32, u32), Cell>,
    pub rows: std::collections::BTreeMap<u32, RowInfo>,
    pub cols: Vec<ColSpan>,
    pub merges: Vec<Rect>,
    pub autofilter: Option<crate::book_data::AutoFilter>,
}

/// One reversible change. Applying a delta returns the delta that undoes it.
#[derive(Clone, Debug)]
pub enum Delta {
    Cell(usize, u32, u32, Option<Cell>),
    Row(usize, u32, Option<RowInfo>),
    Cols(usize, Vec<ColSpan>),
    Merges(usize, Vec<Rect>),
    Meta(usize, SheetMeta),
    Names(Vec<Name>),
    AddSheet(usize, Box<Sheet>),
    RemoveSheet(usize),
    MoveSheet(usize, usize),
    Body(usize, Box<Body>),
    Calc(CalcSettings),
    Filter(usize, Option<crate::book_data::AutoFilter>),
    Objects(usize, Vec<crate::drawing::Obj>),
    /// the worksheet's kept elements (conditional formats...)
    Pieces(usize, Vec<crate::sheet::Piece>),
    Tables(usize, Vec<crate::tables::Table>),
    Notes(usize, BTreeMap<(u32, u32), crate::notes::Note>),
    Controls(usize, Vec<crate::controls::FormControl>),
    Threads(usize, BTreeMap<(u32, u32), crate::notes::Thread>),
    Fit(usize, bool),
    /// the theme part (None: the default theme)
    Theme(Option<String>),
    Pivots(Vec<crate::pivot::Pivot>),
}

#[derive(Clone, Debug)]
pub struct Step {
    pub label: String,
    pub deltas: Vec<Delta>,
    pub before: u64,
    pub after: u64,
    /// the sheet the change happened on (the window goes back there on undo)
    pub sheet: usize,
}

pub struct Book {
    pub sheets: Vec<Sheet>,
    pub styles: Styles,
    pub names: Vec<Name>,
    pub date1904: bool,
    /// Protect Workbook set while open: Some(None) = taken off, Some(Some(attrs)) = on;
    /// None = as the file has it.
    pub wb_protection: Option<Option<Vec<(String, String)>>>,
    pub calc: CalcSettings,
    pub active: usize,
    pub file: crate::xlsx::FileParts,
    // ---- history
    pub undo_stack: Vec<Step>,
    pub redo_stack: Vec<Step>,
    open: Option<Step>,
    group_depth: u32,
    pub state: u64,
    next_state: u64,
    /// changes made (for the window to notice edits)
    pub edits: u64,
    pub saved: u64,
    // ---- calculation
    pub deps: Deps,
    /// positions whose value changed and formulas edited, waiting for the calculation
    pub changed: Vec<Key>,
    pub forced: BTreeSet<Key>,
    /// trying values (What-If Analysis): no data tables worked out meanwhile
    pub in_what_if: bool,
    /// the PivotTables (each knows its sheet)
    pub pivots: Vec<crate::pivot::Pivot>,
    pub rebuild: bool,
    pub pending: bool,
    pub circ: Vec<Key>,
    pub calc_count: usize,
    pub rng: u64,
    pub now_override: Option<f64>,
    pub(crate) todo: BTreeSet<Key>,
    pub(crate) todo_cols: BTreeSet<(usize, u32, u32)>,
    pub(crate) wave: Vec<Key>,
    pub(crate) chain: Vec<Key>,
    pub(crate) circ_hits: HashSet<Key>,
    pub(crate) extents: std::cell::RefCell<Vec<Option<(u32, u32)>>>,
    pub(crate) calc_depth: u32,
    pub layout: crate::layout::Layout,
    pub(crate) spark_cache: std::cell::RefCell<HashMap<usize, (u64, std::rc::Rc<Vec<crate::sparkline::SparkGroup>>)>>,
    pub(crate) cf_cache: std::cell::RefCell<HashMap<usize, (u64, std::rc::Rc<Vec<crate::cf::CfBlock>>)>>,
    pub(crate) dv_cache: std::cell::RefCell<HashMap<usize, (u64, std::rc::Rc<Vec<crate::validation::DvRule>>)>>,
    pub(crate) cf_stats: std::cell::RefCell<HashMap<(usize, usize, usize), ((u64, u64), std::rc::Rc<crate::cf::Stats>)>>,
}

impl Book {
    pub fn new_empty() -> Book {
        let mut b = Book {
            sheets: vec![],
            styles: Styles::parse(None).expect("default styles"),
            names: vec![],
            date1904: false,
            wb_protection: None,
            calc: CalcSettings::default(),
            active: 0,
            file: crate::xlsx::FileParts::default(),
            undo_stack: vec![],
            redo_stack: vec![],
            open: None,
            group_depth: 0,
            state: 0,
            next_state: 1,
            edits: 0,
            saved: 0,
            deps: Deps::default(),
            changed: vec![],
            forced: BTreeSet::new(),
            in_what_if: false,
            pivots: vec![],
            rebuild: false,
            pending: false,
            circ: vec![],
            calc_count: 0,
            rng: 0x2545F4914F6CDD1D ^ (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7)),
            now_override: None,
            todo: BTreeSet::new(),
            todo_cols: BTreeSet::new(),
            wave: vec![],
            chain: vec![],
            circ_hits: HashSet::new(),
            extents: std::cell::RefCell::new(vec![]),
            calc_depth: 0,
            layout: crate::layout::Layout::default(),
            spark_cache: Default::default(),
            cf_cache: Default::default(),
            dv_cache: Default::default(),
            cf_stats: Default::default(),
        };
        b.sheets.push(Sheet::new(1, "Sheet1"));
        b
    }

    // ------------------------------------------------------------------ sheets by name
    pub fn sheet_index(&self, name: &str) -> Option<usize> {
        self.sheets.iter().position(|s| s.name.eq_ignore_ascii_case(name))
    }

    pub fn sheet(&self, i: usize) -> Result<&Sheet, String> {
        self.sheets.get(i).ok_or_else(|| "no such sheet".to_string())
    }

    fn sheet_mut(&mut self, i: usize) -> Result<&mut Sheet, String> {
        self.sheets.get_mut(i).ok_or_else(|| "no such sheet".to_string())
    }

    pub fn meta(&self, i: usize) -> SheetMeta {
        let s = &self.sheets[i];
        SheetMeta {
            name: s.name.clone(),
            state: s.state.clone(),
            tab_color: s.tab_color,
            default_col_width: s.default_col_width,
            base_col_width: s.base_col_width,
            default_row_height: s.default_row_height,
            custom_row_height: s.custom_row_height,
        }
    }

    // ------------------------------------------------------------------ history
    pub fn begin(&mut self, label: &str, sheet: usize) {
        if self.open.is_none() {
            self.open = Some(Step { label: label.to_string(), deltas: vec![], before: self.state, after: self.state, sheet });
        }
    }

    /// Several commands as one undo step (until end_group).
    pub fn begin_group(&mut self, label: &str, sheet: usize) {
        self.begin(label, sheet);
        self.group_depth += 1;
    }

    pub fn end_group(&mut self) {
        if self.group_depth > 0 {
            self.group_depth -= 1;
        }
        if self.group_depth == 0 {
            self.commit();
        }
    }

    /// Apply a change and remember how to undo it.
    pub fn record(&mut self, d: Delta) {
        self.edits += 1;
        let inv = self.apply_delta(d);
        if let Some(st) = self.open.as_mut() {
            st.deltas.push(inv);
        }
    }

    /// Finish the open step: into the undo history, and calculate.
    pub fn commit(&mut self) {
        if self.group_depth > 0 {
            return;
        }
        if let Some(mut st) = self.open.take() {
            if !st.deltas.is_empty() {
                self.state = self.next_state;
                self.next_state += 1;
                st.after = self.state;
                self.undo_stack.push(st);
                self.redo_stack.clear();
                if self.undo_stack.len() > 1000 {
                    self.undo_stack.remove(0);
                }
            }
        }
        self.after_change();
    }

    /// Drop an open step, undoing what it did (a command that failed half way).
    pub fn abort(&mut self) {
        self.group_depth = 0;
        if let Some(st) = self.open.take() {
            for d in st.deltas.into_iter().rev() {
                self.apply_delta(d);
            }
        }
        self.after_change();
    }

    pub fn undo(&mut self) -> Option<(String, usize)> {
        self.commit_open();
        self.edits += 1;
        let st = self.undo_stack.pop()?;
        let mut inv = Vec::with_capacity(st.deltas.len());
        for d in st.deltas.into_iter().rev() {
            inv.push(self.apply_delta(d));
        }
        let label = st.label.clone();
        self.state = st.before;
        self.redo_stack.push(Step { label: st.label, deltas: inv, before: st.before, after: st.after, sheet: st.sheet });
        self.after_change();
        Some((label, st.sheet))
    }

    pub fn redo(&mut self) -> Option<(String, usize)> {
        self.commit_open();
        self.edits += 1;
        let st = self.redo_stack.pop()?;
        let mut inv = Vec::with_capacity(st.deltas.len());
        for d in st.deltas.into_iter().rev() {
            inv.push(self.apply_delta(d));
        }
        let label = st.label.clone();
        self.state = st.after;
        self.undo_stack.push(Step { label: st.label, deltas: inv, before: st.before, after: st.after, sheet: st.sheet });
        self.after_change();
        Some((label, st.sheet))
    }

    fn commit_open(&mut self) {
        self.group_depth = 0;
        if self.open.is_some() {
            self.commit();
        }
    }

    pub fn undo_labels(&self) -> Vec<String> {
        self.undo_stack.iter().rev().map(|s| s.label.clone()).collect()
    }
    pub fn redo_labels(&self) -> Vec<String> {
        self.redo_stack.iter().rev().map(|s| s.label.clone()).collect()
    }
    pub fn modified(&self) -> bool {
        self.state != self.saved
    }
    pub fn mark_saved(&mut self) {
        self.saved = self.state;
    }

    fn apply_delta(&mut self, d: Delta) -> Delta {
        match d {
            Delta::Cell(s, r, c, cell) => {
                let had_formula;
                let old = {
                    let sh = &mut self.sheets[s];
                    let old = sh.put(r, c, cell);
                    had_formula = old.as_ref().map(|x| x.f.is_some()).unwrap_or(false);
                    old
                };
                self.cell_changed(s, r, c, had_formula);
                Delta::Cell(s, r, c, old)
            }
            Delta::Row(s, r, info) => {
                let sh = &mut self.sheets[s];
                let old = match info {
                    Some(i) if !i.is_default() => sh.rows.insert(r, i),
                    _ => sh.rows.remove(&r),
                };
                Delta::Row(s, r, old)
            }
            Delta::Cols(s, cols) => {
                let old = std::mem::replace(&mut self.sheets[s].cols, cols);
                Delta::Cols(s, old)
            }
            Delta::Merges(s, m) => {
                let old = std::mem::replace(&mut self.sheets[s].merges, m);
                self.invalidate_extent(s);
                Delta::Merges(s, old)
            }
            Delta::Meta(s, m) => {
                let old = self.meta(s);
                let sh = &mut self.sheets[s];
                sh.name = m.name;
                sh.state = m.state;
                sh.tab_color = m.tab_color;
                sh.default_col_width = m.default_col_width;
                sh.base_col_width = m.base_col_width;
                sh.default_row_height = m.default_row_height;
                sh.custom_row_height = m.custom_row_height;
                if old.name != self.sheets[s].name {
                    self.rebuild = true;
                }
                Delta::Meta(s, old)
            }
            Delta::Pivots(v) => Delta::Pivots(std::mem::replace(&mut self.pivots, v)),
            Delta::Theme(x) => {
                let old = self.styles.theme_xml.clone();
                self.styles.use_theme(x.as_deref());
                self.styles.dirty = true;
                Delta::Theme(old)
            }
            Delta::Names(n) => {
                let old = std::mem::replace(&mut self.names, n);
                self.rebuild = true;
                Delta::Names(old)
            }
            Delta::AddSheet(at, sh) => {
                self.sheets.insert(at, *sh);
                self.shift_scopes(at, 1);
                for p in self.pivots.iter_mut() {
                    if p.sheet >= at {
                        p.sheet += 1;
                    }
                }
                if self.active >= at && self.sheets.len() > 1 && self.active + 1 < self.sheets.len() {
                    self.active += 1;
                }
                self.rebuild = true;
                Delta::RemoveSheet(at)
            }
            Delta::RemoveSheet(at) => {
                let sh = self.sheets.remove(at);
                self.shift_scopes(at, -1);
                for p in self.pivots.iter_mut() {
                    if p.sheet > at {
                        p.sheet -= 1;
                    }
                }
                if self.active >= self.sheets.len() {
                    self.active = self.sheets.len().saturating_sub(1);
                } else if self.active > at {
                    self.active -= 1;
                }
                self.rebuild = true;
                Delta::AddSheet(at, Box::new(sh))
            }
            Delta::MoveSheet(from, to) => {
                let sh = self.sheets.remove(from);
                self.sheets.insert(to, sh);
                // sheet-scoped names follow their sheet
                let map = |i: usize| -> usize {
                    if i == from {
                        to
                    } else if from < to && i > from && i <= to {
                        i - 1
                    } else if to < from && i >= to && i < from {
                        i + 1
                    } else {
                        i
                    }
                };
                for n in self.names.iter_mut() {
                    if let Some(s) = n.scope {
                        n.scope = Some(map(s));
                    }
                }
                self.active = map(self.active);
                for p in self.pivots.iter_mut() {
                    p.sheet = map(p.sheet);
                }
                self.rebuild = true;
                Delta::MoveSheet(to, from)
            }
            Delta::Body(s, body) => {
                let b = *body;
                let sh = &mut self.sheets[s];
                let old = Body {
                    pieces: std::mem::replace(&mut sh.pieces, b.pieces),
                    cells: std::mem::replace(&mut sh.cells, b.cells),
                    rows: std::mem::replace(&mut sh.rows, b.rows),
                    cols: std::mem::replace(&mut sh.cols, b.cols),
                    merges: std::mem::replace(&mut sh.merges, b.merges),
                    autofilter: std::mem::replace(&mut sh.autofilter, b.autofilter),
                };
                sh.colidx = sh.cells.keys().map(|(r, c)| (*c, *r)).collect();
                sh.spills.clear();
                sh.ghosts.clear();
                sh.blocked.clear();
                sh.arrays = sh.cells.iter().filter_map(|((r, c), cell)| match cell.f.as_ref().map(|f| &f.kind) {
                    Some(FKind::Array(rc)) => Some(((*r, *c), *rc)),
                    _ => None,
                }).collect();
                self.invalidate_extent(s);
                self.rebuild = true;
                Delta::Body(s, Box::new(old))
            }
            Delta::Calc(c) => {
                let old = std::mem::replace(&mut self.calc, c);
                Delta::Calc(old)
            }
            Delta::Fit(s, on) => {
                let old = std::mem::replace(&mut self.sheets[s].fit_to_page, on);
                Delta::Fit(s, old)
            }
            Delta::Notes(s, n) => {
                let old = std::mem::replace(&mut self.sheets[s].notes, n);
                Delta::Notes(s, old)
            }
            Delta::Controls(s, n) => {
                let old = std::mem::replace(&mut self.sheets[s].controls, n);
                Delta::Controls(s, old)
            }
            Delta::Threads(s, n) => {
                let old = std::mem::replace(&mut self.sheets[s].threads, n);
                Delta::Threads(s, old)
            }
            Delta::Tables(s, tables) => {
                let old = std::mem::replace(&mut self.sheets[s].tables, tables);
                // structured references read the tables: work the formulas out again
                self.rebuild = true;
                Delta::Tables(s, old)
            }
            Delta::Pieces(s, pieces) => {
                let old = std::mem::replace(&mut self.sheets[s].pieces, pieces);
                Delta::Pieces(s, old)
            }
            Delta::Objects(s, objs) => {
                let old = std::mem::replace(&mut self.sheets[s].objects, objs);
                Delta::Objects(s, old)
            }
            Delta::Filter(s, f) => {
                let old = std::mem::replace(&mut self.sheets[s].autofilter, f);
                Delta::Filter(s, old)
            }
        }
    }

    fn shift_scopes(&mut self, at: usize, d: i64) {
        for n in self.names.iter_mut() {
            if let Some(s) = n.scope {
                if s >= at {
                    n.scope = Some((s as i64 + d) as usize);
                }
            }
        }
    }

    /// Bookkeeping after a cell was replaced: array ranges, spills, calculation.
    fn cell_changed(&mut self, s: usize, r: u32, c: u32, had_formula: bool) {
        let sh = &mut self.sheets[s];
        let now = sh.cells.get(&(r, c)).map(|x| x.f.clone());
        let has_formula = matches!(now, Some(Some(_)));
        match now.as_ref().and_then(|f| f.as_ref()).map(|f| &f.kind) {
            Some(FKind::Array(rc)) => {
                sh.arrays.insert((r, c), *rc);
            }
            _ => {
                sh.arrays.remove(&(r, c));
            }
        }
        if !has_formula {
            // an anchor that stopped being a formula takes its spill with it
            if let Some(old) = sh.spills.remove(&(r, c)) {
                for rr in old.r1..=old.r2 {
                    for cc in old.c1..=old.c2 {
                        if sh.ghosts.remove(&(rr, cc)).is_some() {
                            self.changed.push((s, rr, cc));
                        }
                    }
                }
            }
            sh.blocked.remove(&(r, c));
        }
        // typing into a spill range (or clearing a blocker) makes its anchor spill again
        let sh = &self.sheets[s];
        let anchors: Vec<(u32, u32)> = sh.spills.iter().chain(sh.blocked.iter())
            .filter(|(a, rc)| **a != (r, c) && rc.contains(r, c)).map(|(a, _)| *a).collect();
        for a in anchors {
            self.forced.insert((s, a.0, a.1));
        }
        if has_formula {
            self.forced.insert((s, r, c));
        }
        if has_formula || had_formula {
            self.deps.update_cell(&self.sheets, &self.names, (s, r, c));
        }
        self.changed.push((s, r, c));
        self.note_extent(s, r, c);
    }

    pub fn invalidate_extent(&self, s: usize) {
        let mut e = self.extents.borrow_mut();
        if e.len() > s {
            e[s] = None;
        }
    }

    fn note_extent(&self, s: usize, _r: u32, _c: u32) {
        self.invalidate_extent(s);
    }

    /// After an edit: calculate (or remember to, in manual mode).
    pub fn after_change(&mut self) {
        if self.rebuild {
            self.rebuild = false;
            self.extents.borrow_mut().clear();
            for n in self.names.iter_mut() {
                n.reparse();
            }
            self.deps.rebuild(&self.sheets, &self.names);
            // everything may refer to anything renamed or moved: calculate all formulas
            let keys: Vec<Key> = self.deps.formulas().collect();
            self.forced.extend(keys);
            for (i, sh) in self.sheets.iter_mut().enumerate() {
                let _ = i;
                sh.spills.retain(|a, _| sh.cells.get(a).map(|c| c.f.is_some()).unwrap_or(false));
            }
        }
        if self.changed.is_empty() && self.forced.is_empty() {
            return;
        }
        if self.calc.manual {
            self.pending = true;
            return;
        }
        self.recalc(false);
        self.recalc_data_tables();
    }

    // ------------------------------------------------------------------ reading cells
    pub fn value(&self, s: usize, r: u32, c: u32) -> Value {
        self.sheets.get(s).map(|sh| sh.value(r, c)).unwrap_or(Value::Empty)
    }

    pub fn is_formula(&self, s: usize, r: u32, c: u32) -> bool {
        self.sheets.get(s).and_then(|sh| sh.get(r, c)).map(|x| x.f.is_some()).unwrap_or(false)
    }

    pub fn number_format(&self, s: usize, r: u32, c: u32) -> String {
        let sh = match self.sheets.get(s) {
            Some(x) => x,
            None => return "General".into(),
        };
        let xf = sh.style_at(r, c);
        self.styles.xf_numfmt(xf)
    }

    /// What the formula bar shows.
    pub fn input(&self, s: usize, r: u32, c: u32) -> String {
        if self.sheets.get(s).map(|sh| sh.has_data_tables.get()).unwrap_or(false) {
            if let Some(t) = self.data_table_text(s, r, c) {
                return t;
            }
        }
        let sh = match self.sheets.get(s) {
            Some(x) => x,
            None => return String::new(),
        };
        let cell = match sh.get(r, c) {
            Some(x) => x,
            None => return String::new(),
        };
        if let Some(f) = &cell.f {
            let text = if f.text.contains('[') {
                let here = crate::tables::table_at(&self.sheets, s, r, c).map(|i| self.sheets[s].tables[i].name.clone());
                crate::tables::to_display(&f.text, here.as_deref())
            } else {
                f.text.clone()
            };
            if let FKind::Array(_) = f.kind {
                return format!("{{={}}}", text);
            }
            return format!("={}", text);
        }
        match &cell.v {
            Value::Empty => String::new(),
            Value::Num(n) => {
                let code = self.styles.xf_numfmt(cell.s);
                let fmt = crate::numfmt::get(&code);
                crate::input::edit_text(*n, &fmt, &code, self.date1904)
            }
            Value::Str(t) => {
                if self.styles.xf(cell.s).quote_prefix || needs_quote(t) {
                    format!("'{}", t)
                } else {
                    t.to_string()
                }
            }
            Value::Bool(b) => if *b { "TRUE".into() } else { "FALSE".into() },
            Value::Err(e) => e.text().to_string(),
            Value::Arr(_) => String::new(),
        }
    }

    /// The text a cell shows (General numbers fit 11 characters).
    pub fn display(&self, s: usize, r: u32, c: u32) -> String {
        let sh = match self.sheets.get(s) {
            Some(x) => x,
            None => return String::new(),
        };
        let v = sh.value(r, c);
        let xf = sh.style_at(r, c);
        self.format_value(&v, xf, 11).text
    }

    pub fn format_value(&self, v: &Value, xf: u32, width: usize) -> crate::numfmt::Formatted {
        let code = self.styles.xf_numfmt(xf);
        self.format_value_code(v, &code, width)
    }

    pub fn format_value_code(&self, v: &Value, code: &str, width: usize) -> crate::numfmt::Formatted {
        let fmt = crate::numfmt::get(code);
        match v {
            Value::Num(n) if *n < 0.0 && fmt.is_date() => crate::numfmt::Formatted {
                // negative dates and times can't be shown
                text: "#".repeat(width.clamp(1, 255)),
                hash_on_overflow: true,
                ..Default::default()
            },
            Value::Num(n) => fmt.number(*n, self.date1904, width),
            Value::Str(t) => fmt.text(t),
            Value::Bool(b) => fmt.text(if *b { "TRUE" } else { "FALSE" }),
            Value::Err(e) => crate::numfmt::Formatted::plain(e.text()),
            Value::Empty => crate::numfmt::Formatted::plain(""),
            Value::Arr(_) => crate::numfmt::Formatted::plain(""),
        }
    }

    pub fn array_range(&self, s: usize, r: u32, c: u32) -> Option<Rect> {
        let sh = self.sheets.get(s)?;
        sh.arrays.values().find(|rc| rc.contains(r, c)).copied()
    }

    // ------------------------------------------------------------------ typing
    /// Type text into a cell. Errors are messages for the user.
    pub fn set_input(&mut self, s: usize, r: u32, c: u32, text: &str) -> Result<(), String> {
        self.check_cell(s, r, c)?;
        if let Some(rc) = self.array_range(s, r, c) {
            // the formula bar shows {=...}: entering that again changes the whole array
            if let Some(inner) = text.strip_prefix("{=").and_then(|t| t.strip_suffix('}')) {
                return self.set_array_formula(s, rc, inner);
            }
            if (rc.r1, rc.c1) != (r, c) || rc.count() > 1 {
                return Err("You can't change part of an array.".into());
            }
        }
        if !self.in_what_if && self.data_table_at(s, r, c).is_some() {
            return Err("Cannot change part of a data table.".into());
        }
        if let Some(i) = self.pivot_at(s, r, c) {
            return self.pivot_input(i, r, c, text);
        }
        let cur = self.sheets[s].get(r, c).cloned();
        let xf = cur.as_ref().map(|x| x.s).unwrap_or_else(|| self.sheets[s].style_at(r, c));
        let code = self.styles.xf_numfmt(xf);
        let text_fmt = code == "@";
        let mut cell = Cell { s: xf, ..Default::default() };
        if let Some(cur) = &cur {
            cell.extra = None;
            let _ = cur;
        }
        let mut new_fmt: Option<String> = None;
        let mut quote = false;
        match interpret(text, self.date1904, text_fmt) {
            Typed::Empty => {}
            Typed::Formula(f) => {
                let f = normalize_formula(&f);
                cell.f = Some(Rc::new(make_formula(&f, FKind::Dynamic)));
                // a formula over dates/currency takes the format of what it refers to, when the cell has none
                if code == "General" {
                    if let Some(fmt) = self.inherited_format(s, &f) {
                        new_fmt = Some(fmt);
                    }
                }
            }
            Typed::Value(v, fmt) => {
                cell.v = v;
                if let Some(f) = fmt {
                    if !matches!(code.as_str(), c if c == f) && keeps_typed_format(&code, &f) {
                        new_fmt = Some(f);
                    }
                }
            }
            Typed::QuotedText(t) => {
                cell.v = Value::string(t);
                quote = true;
            }
        }
        let was_quoted = self.styles.xf(xf).quote_prefix;
        if quote != was_quoted || new_fmt.is_some() {
            let ch = crate::styles::Change {
                numfmt: new_fmt,
                quote_prefix: if quote != was_quoted { Some(quote) } else { None },
                ..Default::default()
            };
            cell.s = self.styles.derive(xf, &ch);
        }
        let newcell = if cell.is_blank() && cell.s == self.inherited_xf(s, r, c) { None } else { Some(cell) };
        self.begin("Typing", s);
        self.record(Delta::Cell(s, r, c, newcell));
        self.tables_after_input(s, r, c);
        self.commit();
        Ok(())
    }

    /// Enter a formula as an array formula over a range (Ctrl+Shift+Enter).
    pub fn set_array_formula(&mut self, s: usize, rc: Rect, text: &str) -> Result<(), String> {
        let f = text.strip_prefix('=').unwrap_or(text);
        if let Some(msg) = formula::check(f) {
            return Err(msg);
        }
        for a in self.sheets[s].arrays.values() {
            if a.intersects(&rc) && !rc.contains_rect(a) {
                return Err("You can't change part of an array.".into());
            }
        }
        self.begin("Array Formula", s);
        let f = normalize_formula(f);
        for r in rc.r1..=rc.r2 {
            for c in rc.c1..=rc.c2 {
                let xf = self.sheets[s].style_at(r, c);
                let mut cell = Cell { s: xf, ..Default::default() };
                if (r, c) == (rc.r1, rc.c1) {
                    cell.f = Some(Rc::new(make_formula(&f, FKind::Array(rc))));
                }
                self.record(Delta::Cell(s, r, c, Some(cell)));
            }
        }
        self.commit();
        Ok(())
    }

    pub(crate) fn check_cell(&self, s: usize, r: u32, c: u32) -> Result<(), String> {
        if s >= self.sheets.len() || r >= MAX_ROWS || c >= MAX_COLS {
            return Err("no such cell".into());
        }
        if self.sheets[s].kind != "work" {
            return Err("This command works on worksheets only.".into());
        }
        Ok(())
    }

    /// The style an empty cell at (r, c) shows (row style, else column style).
    pub fn inherited_xf(&self, s: usize, r: u32, c: u32) -> u32 {
        let sh = &self.sheets[s];
        if let Some(ri) = sh.rows.get(&r) {
            if let Some(x) = ri.style {
                return x;
            }
        }
        sh.col_style(c)
    }

    /// =A1+1 where A1 is a date shows a date: the format of the first reference whose
    /// cell has a date, time, currency or percent format.
    fn inherited_format(&self, s: usize, f: &str) -> Option<String> {
        let toks = formula::tokenize(f).ok()?;
        // text results take no number format
        if toks.iter().any(|t| t.kind == formula::TokKind::Op && t.text == "&") {
            return None;
        }
        // only simple arithmetic picks up a format (functions decide their own)
        if toks.iter().any(|t| t.kind == formula::TokKind::Func) {
            let first = toks.iter().find(|t| t.kind == formula::TokKind::Func)?;
            let name = first.text.trim_end_matches('(').to_uppercase();
            const KEEP: [&str; 9] = ["SUM", "AVERAGE", "MIN", "MAX", "ROUND", "MEDIAN", "SUMIF", "SUMIFS", "IF"];
            if !KEEP.contains(&name.as_str()) {
                return match name.as_str() {
                    "TODAY" | "DATE" => Some("m/d/yyyy".into()),
                    "NOW" => Some("m/d/yyyy h:mm".into()),
                    "TIME" => Some("h:mm AM/PM".into()),
                    "PMT" | "PV" | "FV" | "NPV" | "IPMT" | "PPMT" | "SLN" | "SYD" | "DB" | "DDB" | "VDB" => Some(FMT_FIN.into()),
                    "RATE" | "IRR" | "MIRR" | "XIRR" => Some("0%".into()),
                    _ => None,
                };
            }
        }
        for t in &toks {
            if t.kind != formula::TokKind::Ref {
                continue;
            }
            let rt = formula::parse_ref_tok(&t.text)?;
            let si = match &rt.sheet {
                Some(n) => self.sheet_index(n)?,
                None => s,
            };
            let code = self.number_format(si, rt.r1, rt.c1);
            let fm = crate::numfmt::get(&code);
            if fm.is_date() || fm.is_percent() || code.contains('$') {
                return Some(code);
            }
            if code != "General" {
                return None;
            }
        }
        None
    }

    pub fn clear_contents(&mut self, s: usize, ranges: &[Rect], label: &str) -> Result<(), String> {
        self.check_cell(s, 0, 0)?;
        for rc in ranges {
            for a in self.sheets[s].arrays.values() {
                if a.intersects(rc) && !rc.contains_rect(a) {
                    return Err("You can't change part of an array.".into());
                }
            }
        }
        // a PivotTable goes when all of it is cleared; part of one can't be
        let mut pv_gone: Vec<usize> = Vec::new();
        for (k, p) in self.pivots.iter().enumerate().filter(|(_, p)| p.sheet == s) {
            if let (Some(o), Some(t)) = (p.out, p.table_rect()) {
                if ranges.iter().any(|rc| rc.contains_rect(&t) || rc.contains_rect(&o)) {
                    pv_gone.push(k);
                } else if ranges.iter().any(|rc| rc.intersects(&o) && p.roles.keys().any(|(r, c)| rc.contains(*r, *c))) {
                    return Err(crate::pivot::PIVOT_CHANGE.into());
                }
            }
        }
        self.begin(label, s);
        if !pv_gone.is_empty() {
            let mut all = self.pivots.clone();
            for k in pv_gone.iter().rev() {
                let o = all[*k].out.unwrap();
                for (r, c) in self.sheets[s].positions_in(o) {
                    let st = self.sheets[s].get(r, c).map(|x| x.s).unwrap_or(0);
                    self.record(Delta::Cell(s, r, c, if st != 0 { Some(Cell { s: st, ..Default::default() }) } else { None }));
                }
                all.remove(*k);
            }
            self.record(Delta::Pivots(all));
        }
        for rc in ranges {
            let rc = self.sheets[s].clamp_rect(*rc);
            let pos = self.sheets[s].positions_in(rc);
            for (r, c) in pos {
                let cell = self.sheets[s].get(r, c).cloned().unwrap();
                if cell.is_blank() && cell.rich.is_none() {
                    continue;
                }
                let mut n = Cell { s: cell.s, ..Default::default() };
                if self.styles.xf(cell.s).quote_prefix {
                    n.s = self.styles.derive(cell.s, &crate::styles::Change { quote_prefix: Some(false), ..Default::default() });
                }
                let keep = n.s != self.inherited_xf(s, r, c);
                self.record(Delta::Cell(s, r, c, if keep { Some(n) } else { None }));
            }
        }
        self.commit();
        Ok(())
    }

    /// Put a value (not text to interpret) into a cell; for paste, fill, sort.
    pub fn put_cell(&mut self, s: usize, r: u32, c: u32, cell: Option<Cell>) {
        self.record(Delta::Cell(s, r, c, cell));
    }

    // ------------------------------------------------------------------ formats
    /// Restyle cells: `f(styles, xf, row, col, range)` gives the new style. Whole rows and
    /// columns change the row/column styles and the cells that exist; a positional `f`
    /// (one that depends on where in the range a cell is) also gets the edge cells.
    pub fn restyle(&mut self, s: usize, ranges: &[Rect], positional: bool,
                   f: &mut dyn FnMut(&mut Styles, u32, Option<u32>, Option<u32>, Rect) -> u32) -> Result<(), String> {
        let mut created: u64 = 0;
        for rg in ranges {
            let rg = self.sheets[s].clamp_rect(*rg);
            let wc = rg.r1 == 0 && rg.r2 == MAX_ROWS - 1;
            let wr = rg.c1 == 0 && rg.c2 == MAX_COLS - 1;
            if wc {
                // column styles
                let mut cols = self.sheets[s].cols.clone();
                {
                    let styles = &mut self.styles;
                    let mut tmp = Sheet::new(0, "");
                    tmp.cols = std::mem::take(&mut cols);
                    let default_w = None;
                    let _: Option<f64> = default_w;
                    map_col_styles(&mut tmp, rg.c1, rg.c2, |c, st| f(styles, st, None, Some(c), rg));
                    cols = tmp.cols;
                }
                if cols != self.sheets[s].cols {
                    self.record(Delta::Cols(s, cols));
                }
                // styled rows get every cell; other rows only the cells that exist
                let row_keys: Vec<u32> = self.sheets[s].rows.iter().filter(|(_, ri)| ri.style.is_some()).map(|(r, _)| *r).collect();
                let mut targets: BTreeSet<(u32, u32)> = BTreeSet::new();
                for r in &row_keys {
                    for c in rg.c1..=rg.c2 {
                        targets.insert((*r, c));
                    }
                    created += rg.cols() as u64;
                    if created > CELL_LIMIT {
                        return Err("The selection is too large to format cell by cell.".into());
                    }
                }
                for (r, c) in self.sheets[s].positions_in(rg) {
                    targets.insert((r, c));
                }
                if positional && !wr {
                    for c in rg.c1..=rg.c2 {
                        for r in [rg.r1, rg.r2] {
                            if !self.sheets[s].cells.contains_key(&(r, c)) {
                                let cur = self.sheets[s].style_at(r, c);
                                let n = f(&mut self.styles, cur, Some(r), Some(c), rg);
                                if n != cur {
                                    targets.insert((r, c));
                                }
                            }
                        }
                    }
                }
                for (r, c) in targets {
                    self.restyle_cell(s, r, c, rg, f);
                }
                if wr {
                    // the whole sheet: styled rows too
                    for r in row_keys {
                        self.restyle_row(s, r, rg, f);
                    }
                }
            } else if wr {
                created += rg.rows() as u64;
                if created > CELL_LIMIT {
                    return Err("The selection is too large to format cell by cell.".into());
                }
                for r in rg.r1..=rg.r2 {
                    self.restyle_row(s, r, rg, f);
                    let cs: Vec<u32> = self.sheets[s].cells.range((r, 0)..=(r, MAX_COLS)).map(|((_, c), _)| *c).collect();
                    for c in cs {
                        self.restyle_cell(s, r, c, rg, f);
                    }
                    if positional {
                        for c in [rg.c1, rg.c2] {
                            if !self.sheets[s].cells.contains_key(&(r, c)) {
                                let cur = self.sheets[s].style_at(r, c);
                                let n = f(&mut self.styles, cur, Some(r), Some(c), rg);
                                if n != self.inherited_xf(s, r, c) {
                                    self.set_cell_xf(s, r, c, n);
                                }
                            }
                        }
                    }
                }
            } else {
                created += rg.count();
                if created > CELL_LIMIT {
                    return Err("The selection is too large to format cell by cell.".into());
                }
                for r in rg.r1..=rg.r2 {
                    for c in rg.c1..=rg.c2 {
                        self.restyle_cell(s, r, c, rg, f);
                    }
                }
            }
        }
        Ok(())
    }

    fn restyle_row(&mut self, s: usize, r: u32, rg: Rect,
                   f: &mut dyn FnMut(&mut Styles, u32, Option<u32>, Option<u32>, Rect) -> u32) {
        let mut ri = self.sheets[s].rows.get(&r).cloned().unwrap_or_default();
        let base = ri.style.unwrap_or(0);
        let n = f(&mut self.styles, base, Some(r), None, rg);
        if ri.style != Some(n) {
            ri.style = Some(n);
            self.record(Delta::Row(s, r, Some(ri)));
        }
    }

    fn restyle_cell(&mut self, s: usize, r: u32, c: u32, rg: Rect,
                    f: &mut dyn FnMut(&mut Styles, u32, Option<u32>, Option<u32>, Rect) -> u32) {
        let cur = self.sheets[s].style_at(r, c);
        let n = f(&mut self.styles, cur, Some(r), Some(c), rg);
        if n != cur || !self.sheets[s].cells.contains_key(&(r, c)) {
            self.set_cell_xf(s, r, c, n);
        }
    }

    pub fn set_cell_xf(&mut self, s: usize, r: u32, c: u32, xf: u32) {
        let cur = self.sheets[s].get(r, c).cloned();
        let inherited = self.inherited_xf(s, r, c);
        match cur {
            Some(mut cell) => {
                if cell.s == xf {
                    return;
                }
                cell.s = xf;
                let keep = !(cell.is_blank() && cell.rich.is_none() && xf == inherited);
                self.record(Delta::Cell(s, r, c, if keep { Some(cell) } else { None }));
            }
            None => {
                if xf == inherited {
                    return;
                }
                self.record(Delta::Cell(s, r, c, Some(Cell { s: xf, ..Default::default() })));
            }
        }
    }

    pub fn set_number_format(&mut self, s: usize, ranges: &[Rect], code: &str) -> Result<(), String> {
        self.check_cell(s, 0, 0)?;
        let ch = crate::styles::Change { numfmt: Some(code.to_string()), ..Default::default() };
        self.begin("Number Format", s);
        let r = self.restyle(s, ranges, false, &mut |st, xf, _, _, _| st.derive(xf, &ch));
        if let Err(e) = r {
            self.abort();
            return Err(e);
        }
        self.commit();
        Ok(())
    }

    /// Apply a style change to ranges as one step.
    pub fn apply_change(&mut self, s: usize, ranges: &[Rect], ch: &crate::styles::Change, label: &str) -> Result<(), String> {
        self.check_cell(s, 0, 0)?;
        self.begin(label, s);
        let mut memo: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        let r = self.restyle(s, ranges, false, &mut |st, xf, _, _, _| *memo.entry(xf).or_insert_with(|| st.derive(xf, ch)));
        if let Err(e) = r {
            self.abort();
            return Err(e);
        }
        self.commit();
        Ok(())
    }

    /// Increase/Decrease Decimal: from the active cell's format (or its value's digits).
    pub fn adjust_decimals(&mut self, s: usize, ranges: &[Rect], r: u32, c: u32, delta: i32) -> Result<(), String> {
        let code = self.number_format(s, r, c);
        let v = self.value(s, r, c);
        let new = crate::numfmt::change_decimals(&code, &v, delta, self.date1904);
        match new {
            Some(code) => self.set_number_format(s, ranges, &code),
            None => Ok(()),
        }
    }

    pub fn set_merges(&mut self, s: usize, merges: Vec<Rect>, label: &str) {
        self.begin(label, s);
        if merges != self.sheets[s].merges {
            self.record(Delta::Merges(s, merges));
        }
        self.commit();
    }

    // ------------------------------------------------------------------ rows and columns
    pub fn set_col_width_units(&mut self, s: usize, c1: u32, c2: u32, units: f64, label: &str) {
        let mut sh = Sheet::new(0, "");
        sh.cols = self.sheets[s].cols.clone();
        let hide = units <= 0.0;
        sh.map_cols(c1, c2.min(MAX_COLS - 1), |span| {
            if hide {
                span.hidden = true;
            } else {
                span.width = Some(units);
                span.custom = true;
                span.hidden = false;
            }
            span.best_fit = false;
        });
        self.begin(label, s);
        if sh.cols != self.sheets[s].cols {
            self.record(Delta::Cols(s, sh.cols));
        }
        self.commit();
    }

    pub fn set_cols_hidden(&mut self, s: usize, c1: u32, c2: u32, hidden: bool) {
        let mut sh = Sheet::new(0, "");
        sh.cols = self.sheets[s].cols.clone();
        let mdw = self.mdw(s);
        let dw = self.sheets[s].default_width_units(mdw);
        sh.map_cols(c1, c2.min(MAX_COLS - 1), |span| {
            span.hidden = hidden;
            if !hidden && span.width == Some(0.0) {
                span.width = Some(dw);
            }
        });
        self.begin(if hidden { "Hide Columns" } else { "Unhide Columns" }, s);
        if sh.cols != self.sheets[s].cols {
            self.record(Delta::Cols(s, sh.cols));
        }
        self.commit();
    }

    pub fn set_row_height_pt(&mut self, s: usize, r1: u32, r2: u32, pt: f64, label: &str, custom: bool) {
        self.begin(label, s);
        for r in r1..=r2.min(MAX_ROWS - 1) {
            let mut ri = self.sheets[s].rows.get(&r).cloned().unwrap_or_default();
            if pt <= 0.0 {
                ri.hidden = true;
            } else {
                ri.ht = Some(pt);
                ri.custom_height = custom;
                ri.hidden = false;
            }
            if self.sheets[s].rows.get(&r) != Some(&ri) {
                self.record(Delta::Row(s, r, Some(ri)));
            }
            if r2 - r1 > 1_000_000 {
                break;
            }
        }
        self.commit();
    }

    pub fn set_rows_hidden(&mut self, s: usize, r1: u32, r2: u32, hidden: bool) {
        self.begin(if hidden { "Hide Rows" } else { "Unhide Rows" }, s);
        let r2 = r2.min(MAX_ROWS - 1);
        if !hidden && r2 - r1 > 100_000 {
            // unhide over a huge span: only rows that have settings
            let keys: Vec<u32> = self.sheets[s].rows.range(r1..=r2).filter(|(_, ri)| ri.hidden).map(|(r, _)| *r).collect();
            for r in keys {
                let mut ri = self.sheets[s].rows[&r].clone();
                ri.hidden = false;
                if ri.ht == Some(0.0) {
                    ri.ht = None;
                }
                self.record(Delta::Row(s, r, Some(ri)));
            }
        } else {
            for r in r1..=r2 {
                let mut ri = self.sheets[s].rows.get(&r).cloned().unwrap_or_default();
                ri.hidden = hidden;
                if !hidden && ri.ht == Some(0.0) {
                    ri.ht = None;
                }
                if self.sheets[s].rows.get(&r).cloned().unwrap_or_default() != ri {
                    self.record(Delta::Row(s, r, Some(ri)));
                }
            }
        }
        self.commit();
    }

    pub fn set_default_col_width(&mut self, s: usize, chars: f64) {
        let mdw = self.mdw(s);
        let mut m = self.meta(s);
        m.default_col_width = Some(crate::sheet::chars_to_units(chars, mdw));
        self.begin("Default Width", s);
        if m != self.meta(s) {
            self.record(Delta::Meta(s, m));
        }
        self.commit();
    }

    pub fn mdw(&self, _s: usize) -> f64 {
        self.layout.mdw(&self.styles)
    }

    // ------------------------------------------------------------------ sheets
    pub fn check_sheet_name(&self, name: &str, me: Option<usize>) -> Option<String> {
        let bad = name.is_empty() || name.chars().count() > 31 || name.contains(['\\', '/', '?', '*', '[', ']', ':'])
            || name.starts_with('\'') || name.ends_with('\'');
        if bad {
            return Some("You typed an invalid name for a sheet or chart. Make sure that:\n\n\u{2022} The name that you type does not exceed 31 characters.\n\u{2022} The name does not contain any of the following characters: \\ / ? * [ or ]\n\u{2022} You did not leave the name blank.".into());
        }
        if name.eq_ignore_ascii_case("History") {
            return Some("History is a reserved name. Try a different one.".into());
        }
        if let Some(i) = self.sheet_index(name) {
            if Some(i) != me {
                return Some("That name is already taken. Try a different one.".into());
            }
        }
        None
    }

    pub fn insert_sheet(&mut self, at: usize, name: Option<&str>) -> Result<usize, String> {
        let at = at.min(self.sheets.len());
        let name = match name {
            Some(n) => {
                if let Some(m) = self.check_sheet_name(n, None) {
                    return Err(m);
                }
                n.to_string()
            }
            None => {
                let mut k = self.sheets.len() + 1;
                loop {
                    let n = format!("Sheet{}", k);
                    if self.sheet_index(&n).is_none() {
                        break n;
                    }
                    k += 1;
                }
            }
        };
        let id = self.sheets.iter().map(|s| s.id).max().unwrap_or(0) + 1;
        let mut sh = Sheet::new(id, &name);
        if let Some(first) = self.sheets.iter().find(|s| s.kind == "work") {
            sh.default_row_height = first.default_row_height;
            sh.base_col_width = first.base_col_width;
        }
        self.begin("Insert Sheet", at);
        self.record(Delta::AddSheet(at, Box::new(sh)));
        self.commit();
        Ok(at)
    }

    pub fn delete_sheet(&mut self, i: usize) -> Result<(), String> {
        if i >= self.sheets.len() {
            return Err("no such sheet".into());
        }
        let visible = self.sheets.iter().enumerate().filter(|(k, s)| *k != i && s.state == "visible").count();
        if visible == 0 {
            return Err("A workbook must contain at least one visible worksheet.".into());
        }
        let name = self.sheets[i].name.clone();
        self.begin("Delete Sheet", i);
        // references to the sheet become #REF!
        self.rewrite_formulas(&Adjust::DeleteSheet(&name), Some(i));
        // names scoped to it go; names referring to it get #REF!
        let mut names = self.names.clone();
        names.retain(|n| n.scope != Some(i));
        for n in names.iter_mut() {
            n.text = formula::adjust(&n.text, &Adjust::DeleteSheet(&name));
        }
        self.record(Delta::Names(names));
        if self.pivots.iter().any(|p| p.sheet == i) {
            let keep: Vec<crate::pivot::Pivot> = self.pivots.iter().filter(|p| p.sheet != i).cloned().collect();
            self.record(Delta::Pivots(keep));
        }
        self.record(Delta::RemoveSheet(i));
        self.commit();
        Ok(())
    }

    pub fn rename_sheet(&mut self, i: usize, name: &str) -> Result<(), String> {
        if i >= self.sheets.len() {
            return Err("no such sheet".into());
        }
        if let Some(m) = self.check_sheet_name(name, Some(i)) {
            return Err(m);
        }
        let old = self.sheets[i].name.clone();
        if old == name {
            return Ok(());
        }
        self.begin("Rename Sheet", i);
        self.rewrite_formulas(&Adjust::RenameSheet(&old, name), None);
        let mut names = self.names.clone();
        let mut changed = false;
        for n in names.iter_mut() {
            let t = formula::adjust(&n.text, &Adjust::RenameSheet(&old, name));
            if t != n.text {
                n.text = t;
                changed = true;
            }
        }
        if changed {
            self.record(Delta::Names(names));
        }
        if self.pivots.iter().any(|p| p.source_sheet == old) {
            let mut ps = self.pivots.clone();
            for p in ps.iter_mut() {
                if p.source_sheet == old {
                    p.source_sheet = name.to_string();
                    p.dirty = true;
                }
            }
            self.record(Delta::Pivots(ps));
        }
        let mut m = self.meta(i);
        m.name = name.to_string();
        self.record(Delta::Meta(i, m));
        self.commit();
        Ok(())
    }

    pub fn move_sheet(&mut self, from: usize, to: usize) -> Result<(), String> {
        if from >= self.sheets.len() {
            return Err("no such sheet".into());
        }
        let to = to.min(self.sheets.len() - 1);
        if from == to {
            return Ok(());
        }
        self.begin("Move Sheet", to);
        self.record(Delta::MoveSheet(from, to));
        self.commit();
        Ok(())
    }

    pub fn set_sheet_state(&mut self, i: usize, state: &str) -> Result<(), String> {
        if i >= self.sheets.len() {
            return Err("no such sheet".into());
        }
        if state != "visible" {
            let visible = self.sheets.iter().enumerate().filter(|(k, s)| *k != i && s.state == "visible").count();
            if visible == 0 {
                return Err("A workbook must contain at least one visible worksheet.".into());
            }
        }
        let mut m = self.meta(i);
        m.state = state.to_string();
        self.begin(if state == "visible" { "Unhide Sheet" } else { "Hide Sheet" }, i);
        if m != self.meta(i) {
            self.record(Delta::Meta(i, m));
        }
        self.commit();
        Ok(())
    }

    pub fn set_tab_color(&mut self, i: usize, rgb: Option<u32>) -> Result<(), String> {
        if i >= self.sheets.len() {
            return Err("no such sheet".into());
        }
        let mut m = self.meta(i);
        m.tab_color = rgb;
        self.begin("Tab Color", i);
        if m != self.meta(i) {
            self.record(Delta::Meta(i, m));
        }
        self.commit();
        Ok(())
    }

    /// Rewrite every formula in the workbook with `how` (recorded). `skip`: a sheet
    /// whose formulas don't matter (it is being deleted).
    pub fn rewrite_formulas(&mut self, how: &Adjust, skip: Option<usize>) {
        for s in 0..self.sheets.len() {
            if Some(s) == skip {
                continue;
            }
            let here = self.sheets[s].name.clone();
            let mut edits = Vec::new();
            for ((r, c), cell) in self.sheets[s].cells.iter() {
                if let Some(f) = &cell.f {
                    let how2 = rebase(how, &here);
                    let t = formula::adjust(&f.text, &how2);
                    if t != f.text {
                        let mut n = cell.clone();
                        let mut nf = (**f).clone();
                        nf.text = t;
                        nf.expr = formula::parse(&nf.text).ok();
                        n.f = Some(Rc::new(nf));
                        edits.push((*r, *c, n));
                    }
                }
            }
            for (r, c, n) in edits {
                self.record(Delta::Cell(s, r, c, Some(n)));
            }
        }
        self.adjust_charts(how);
    }

    // ------------------------------------------------------------------ names
    /// Hides or shows a defined name (hidden names, like the ones queries keep their settings in,
    /// don't appear in the Name Manager).
    pub fn set_name_hidden(&mut self, name: &str, scope: Option<usize>, hidden: bool) -> Result<(), String> {
        let mut names = self.names.clone();
        let n = names.iter_mut().find(|n| n.name.eq_ignore_ascii_case(name) && n.scope == scope).ok_or("There is no such name.")?;
        if n.hidden == hidden {
            return Ok(());
        }
        n.hidden = hidden;
        self.begin("Define Name", scope.unwrap_or(self.active));
        self.record(Delta::Names(names));
        self.commit();
        Ok(())
    }

    pub fn define_name(&mut self, name: &str, refers_to: &str, scope: Option<usize>, comment: Option<&str>,
                       old: Option<(String, Option<usize>)>) -> Result<(), String> {
        if !valid_name(name) {
            return Err("The name that you entered is not valid.".into());
        }
        let text = refers_to.trim();
        let text = text.strip_prefix('=').unwrap_or(text);
        if text.is_empty() || formula::check(text).is_some() {
            return Err("There's a problem with this formula.".into());
        }
        let mut names = self.names.clone();
        let (oname, oscope) = old.clone().unwrap_or((name.to_string(), scope));
        let pos = names.iter().position(|n| n.name.eq_ignore_ascii_case(&oname) && n.scope == oscope);
        if names.iter().enumerate().any(|(i, n)| Some(i) != pos && n.name.eq_ignore_ascii_case(name) && n.scope == scope) {
            return Err("The name already exists. Enter a unique name.".into());
        }
        let mut n = match pos {
            Some(p) => names.remove(p),
            None => Name::new(name, scope, text),
        };
        n.name = name.to_string();
        n.scope = scope;
        n.text = text.to_string();
        n.comment = comment.map(|s| s.to_string()).filter(|s| !s.is_empty()).or(if comment.is_some() { None } else { n.comment.take() });
        n.reparse();
        names.push(n);
        names.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.scope.cmp(&b.scope)));
        self.begin("Define Name", scope.unwrap_or(self.active));
        self.record(Delta::Names(names));
        self.commit();
        Ok(())
    }

    pub fn delete_name(&mut self, name: &str, scope: Option<usize>) -> Result<(), String> {
        let mut names = self.names.clone();
        let before = names.len();
        names.retain(|n| !(n.name.eq_ignore_ascii_case(name) && n.scope == scope));
        if names.len() == before {
            return Err("no such name".into());
        }
        self.begin("Delete Name", self.active);
        self.record(Delta::Names(names));
        self.commit();
        Ok(())
    }

    pub fn find_name(&self, name: &str, scope: usize) -> Option<&Name> {
        self.names.iter().find(|n| n.scope == Some(scope) && n.name.eq_ignore_ascii_case(name))
            .or_else(|| self.names.iter().find(|n| n.scope.is_none() && n.name.eq_ignore_ascii_case(name)))
    }

    pub fn set_calc(&mut self, c: CalcSettings) {
        if c != self.calc {
            let was_manual = self.calc.manual;
            self.calc = c;
            if was_manual && !self.calc.manual && self.pending {
                self.recalc(false);
            }
        }
    }

    // ------------------------------------------------------------------ navigation
    /// Ctrl+arrow: the edge of the data block.
    pub fn ctrl_move(&self, s: usize, r: u32, c: u32, dr: i32, dc: i32) -> (u32, u32) {
        let sh = &self.sheets[s];
        let has = |r: i64, c: i64| -> bool {
            r >= 0 && c >= 0 && r < MAX_ROWS as i64 && c < MAX_COLS as i64 && sh.has_content(r as u32, c as u32)
        };
        let (mut r, mut c) = (r as i64, c as i64);
        let (dr, dc) = (dr as i64, dc as i64);
        let inside = |r: i64, c: i64| r >= 0 && c >= 0 && r < MAX_ROWS as i64 && c < MAX_COLS as i64;
        if !inside(r + dr, c + dc) {
            return (r as u32, c as u32);
        }
        let here = has(r, c);
        let next = has(r + dr, c + dc);
        if here && next {
            // to the last filled cell of the run
            while inside(r + dr, c + dc) && has(r + dr, c + dc) {
                r += dr;
                c += dc;
            }
            return (r as u32, c as u32);
        }
        // to the next filled cell, or the edge
        let (er, ec) = sh.data_extent().map(|(a, b)| (a as i64, b as i64)).unwrap_or((-1, -1));
        r += dr;
        c += dc;
        loop {
            if has(r, c) {
                return (r as u32, c as u32);
            }
            let past = (dr > 0 && r > er) || (dc > 0 && c > ec);
            if past || !inside(r + dr, c + dc) {
                if dr > 0 {
                    return ((MAX_ROWS - 1), c as u32);
                }
                if dc > 0 {
                    return (r as u32, MAX_COLS - 1);
                }
                if !inside(r + dr, c + dc) {
                    return (r as u32, c as u32);
                }
            }
            r += dr;
            c += dc;
        }
    }

    /// The block of filled cells around a cell (Ctrl+A, Ctrl+*).
    pub fn current_region(&self, s: usize, r: u32, c: u32) -> Rect {
        let sh = &self.sheets[s];
        let mut rc = Rect::cell(r, c);
        loop {
            let mut grown = rc;
            let r1 = rc.r1.saturating_sub(1);
            let c1 = rc.c1.saturating_sub(1);
            let r2 = (rc.r2 + 1).min(MAX_ROWS - 1);
            let c2 = (rc.c2 + 1).min(MAX_COLS - 1);
            // the ring around rc
            let ring = Rect::new(r1, c1, r2, c2);
            for (rr, cc) in sh.positions_in(ring) {
                if rc.contains(rr, cc) {
                    continue;
                }
                if sh.has_content(rr, cc) {
                    grown = grown.union(&Rect::cell(rr, cc));
                }
            }
            for (rr, cc) in sh.ghosts.keys() {
                if ring.contains(*rr, *cc) && !rc.contains(*rr, *cc) {
                    grown = grown.union(&Rect::cell(*rr, *cc));
                }
            }
            for m in &sh.merges {
                if m.intersects(&ring) {
                    grown = grown.union(m);
                }
            }
            if grown == rc {
                break;
            }
            rc = grown;
        }
        rc
    }

    pub fn aggregates(&mut self, s: usize, ranges: &[Rect]) -> (usize, usize, f64, f64, f64) {
        let mut count = 0usize;
        let mut nums = 0usize;
        let (mut sum, mut min, mut max) = (0.0, f64::INFINITY, f64::NEG_INFINITY);
        let mut seen: HashSet<(u32, u32)> = HashSet::new();
        let sh = &self.sheets[s];
        for rc in ranges {
            let rc = sh.clamp_rect(*rc);
            let mut visit = |r: u32, c: u32, v: &Value| {
                if !seen.insert((r, c)) || sh.row_hidden(r) {
                    return;
                }
                match v {
                    Value::Empty => {}
                    Value::Num(n) => {
                        count += 1;
                        nums += 1;
                        sum += n;
                        min = min.min(*n);
                        max = max.max(*n);
                    }
                    _ => count += 1,
                }
            };
            for ((r, c), cell) in sh.cells_in(rc) {
                visit(*r, *c, &cell.v);
            }
            for ((r, c), v) in sh.ghosts.iter() {
                if rc.contains(*r, *c) {
                    visit(*r, *c, v);
                }
            }
        }
        (count, nums, sum, min, max)
    }
}

/// The currency format financial functions give their cells.
pub const FMT_FIN: &str = "\"$\"#,##0.00_);\\(\"$\"#,##0.00\\)";

/// Text that would be read as something else gets an apostrophe in the formula bar.
fn needs_quote(t: &str) -> bool {
    if t.is_empty() {
        return false;
    }
    t.starts_with('=') || crate::input::parse_number_text(t, false).is_some() || t.eq_ignore_ascii_case("TRUE")
        || t.eq_ignore_ascii_case("FALSE") || XlErr::parse(t.trim()).is_some()
}

/// A typed number's format is kept unless the cell already has one of the same kind.
fn keeps_typed_format(cur: &str, typed: &str) -> bool {
    if cur == "General" {
        return true;
    }
    let c = crate::numfmt::get(cur);
    let t = crate::numfmt::get(typed);
    // a date typed into a date cell keeps the cell's date format
    if c.is_date() && t.is_date() {
        return false;
    }
    if c.is_percent() && t.is_percent() {
        return false;
    }
    if cur.contains('$') && typed.contains('$') {
        return false;
    }
    !(cur.contains("E+") && typed.contains("E+"))
}

/// Formula text as typed: function names upper-cased, missing closing parentheses added.
pub fn normalize_formula(f: &str) -> String {
    let toks = match formula::tokenize(f) {
        Ok(t) => t,
        Err(_) => return f.to_string(),
    };
    let mut out = String::new();
    let mut depth: i32 = 0;
    for t in &toks {
        match t.kind {
            formula::TokKind::Func => out.push_str(&t.text.to_uppercase()),
            formula::TokKind::Ref => out.push_str(&upper_ref(&t.text)),
            formula::TokKind::Bool => out.push_str(&t.text.to_uppercase()),
            formula::TokKind::Err => out.push_str(&t.text.to_uppercase()),
            formula::TokKind::LParen => {
                depth += 1;
                out.push_str(&t.text);
            }
            formula::TokKind::RParen => {
                depth -= 1;
                out.push_str(&t.text);
            }
            _ => out.push_str(&t.text),
        }
    }
    let funcs = toks.iter().filter(|t| t.kind == formula::TokKind::Func && !t.text.ends_with('(')).count() as i32;
    let _ = funcs;
    while depth > 0 {
        out.push(')');
        depth -= 1;
    }
    out
}

fn upper_ref(t: &str) -> String {
    match t.rfind('!') {
        Some(i) => format!("{}{}", &t[..=i], t[i + 1..].to_uppercase()),
        None => t.to_uppercase(),
    }
}

pub fn make_formula(text: &str, kind: FKind) -> Formula {
    let expr = formula::parse(text).ok();
    let volatile = expr.as_ref().map(is_volatile).unwrap_or(false);
    Formula { text: text.to_string(), expr, kind, volatile, extra: vec![] }
}

pub fn is_volatile(e: &Expr) -> bool {
    match e {
        Expr::Func(n, args) => crate::fns::is_volatile(n) || matches!(&**n, "OFFSET" | "INDIRECT" | "CELL" | "INFO") || args.iter().any(is_volatile),
        Expr::Neg(x) | Expr::Plus(x) | Expr::Percent(x) | Expr::Spill(x) | Expr::At(x) => is_volatile(x),
        Expr::Bin(_, a, b) | Expr::Range(a, b) | Expr::Isect(a, b) => is_volatile(a) || is_volatile(b),
        Expr::Union(v) => v.iter().any(is_volatile),
        Expr::Array(rows) => rows.iter().flatten().any(is_volatile),
        Expr::Call(f, args) => is_volatile(f) || args.iter().any(is_volatile),
        _ => false,
    }
}

/// The same adjustment with `here` set to the sheet a formula lives on.
pub fn rebase<'a>(how: &Adjust<'a>, here: &'a str) -> Adjust<'a> {
    match how {
        Adjust::Shift(a, b) => Adjust::Shift(*a, *b),
        Adjust::Rows { sheet, at, n, .. } => Adjust::Rows { sheet, here, at: *at, n: *n },
        Adjust::Cols { sheet, at, n, .. } => Adjust::Cols { sheet, here, at: *at, n: *n },
        Adjust::RowsIn { sheet, at, n, c1, c2, .. } => Adjust::RowsIn { sheet, here, at: *at, n: *n, c1: *c1, c2: *c2 },
        Adjust::ColsIn { sheet, at, n, r1, r2, .. } => Adjust::ColsIn { sheet, here, at: *at, n: *n, r1: *r1, r2: *r2 },
        Adjust::RenameSheet(a, b) => Adjust::RenameSheet(a, b),
        Adjust::DeleteSheet(a) => Adjust::DeleteSheet(a),
        Adjust::Move { sheet, src, dr, dc, .. } => Adjust::Move { sheet, here, src: *src, dr: *dr, dc: *dc },
    }
}

pub fn valid_name(name: &str) -> bool {
    let mut ch = name.chars();
    let first = match ch.next() {
        Some(c) => c,
        None => return false,
    };
    if !(first.is_alphabetic() || first == '_' || first == '\\') {
        return false;
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '\\' || c == '?') {
        return false;
    }
    if name.len() > 255 {
        return false;
    }
    let up = name.to_uppercase();
    if up == "R" || up == "C" || up == "TRUE" || up == "FALSE" {
        return false;
    }
    // looks like a cell reference (A1, XFD1048576) or R1C1
    if crate::cellref::parse_cell(&up).is_some() {
        return false;
    }
    let b = up.as_bytes();
    if b[0] == b'R' || b[0] == b'C' {
        let rest: String = up.chars().skip(1).collect();
        if rest.chars().all(|c| c.is_ascii_digit() || c == 'C' || c == 'R') && rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return false;
        }
    }
    true
}

/// Column styles (whole-column formatting) changed one by one, spans kept compact.
pub fn map_col_styles(sh: &mut Sheet, c1: u32, c2: u32, mut f: impl FnMut(u32, u32) -> u32) {
    // split the range into runs of equal current settings, then into runs of equal new style
    let mut runs: Vec<(u32, u32, u32)> = Vec::new(); // (start, end, new style)
    let mut c = c1;
    while c <= c2 {
        let cur = sh.col_span(c).cloned();
        let end = match &cur {
            Some(sp) => sp.max.min(c2),
            None => {
                // up to the next span
                let i = sh.cols.partition_point(|s| s.max < c);
                sh.cols.get(i).map(|s| s.min.saturating_sub(1)).unwrap_or(c2).min(c2)
            }
        };
        let base = cur.as_ref().map(|s| s.style).unwrap_or(0);
        // f may depend on the column (positional borders): evaluate edges separately
        let first = f(c, base);
        if end > c {
            let mid = f(c + 1, base);
            let last = f(end, base);
            runs.push((c, c, first));
            if end > c + 1 {
                runs.push((c + 1, end - 1, mid));
            }
            runs.push((end, end, last));
        } else {
            runs.push((c, c, first));
        }
        if end == u32::MAX || end >= MAX_COLS - 1 {
            break;
        }
        c = end + 1;
    }
    for (a, b, st) in runs {
        sh.map_cols(a, b, |span| span.style = st);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert!(valid_name("foo"));
        assert!(valid_name("_x.y"));
        assert!(!valid_name("A1"));
        assert!(!valid_name("x y"));
        assert!(!valid_name("R1C1"));
        assert!(!valid_name("1abc"));
        assert!(valid_name("Rate"));
    }
    #[test]
    fn normalize() {
        assert_eq!(normalize_formula("sum(a1:b2"), "SUM(A1:B2)");
        assert_eq!(normalize_formula("\"x\"&a1"), "\"x\"&A1");
    }
}
