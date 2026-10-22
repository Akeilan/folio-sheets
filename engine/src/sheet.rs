//! One sheet: cells, row and column settings, merges, view state, and the parts of its
//! XML the engine doesn't model (kept for writing back unchanged).

use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::formula::Expr;
use crate::value::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum FKind {
    /// written without t= : ranges in single-value places are intersected with the cell's row/column
    Normal,
    /// typed here, or a spilling formula from a file: arrays spill
    Dynamic,
    /// a fixed-size array formula (Ctrl+Shift+Enter) over the rectangle
    Array(Rect),
    /// a what-if data table; kept as written
    DataTable(String),
}

#[derive(Clone, Debug)]
pub struct Formula {
    /// as shown in the formula bar, without '='
    pub text: String,
    pub expr: Option<Expr>,
    pub kind: FKind,
    pub volatile: bool,
    /// other <f> attributes to keep (aca, ca...)
    pub extra: Vec<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct Cell {
    pub v: Value,
    pub f: Option<Rc<Formula>>,
    pub s: u32,
    /// rich text run XML (the <si> body) when the text has formatting runs
    pub rich: Option<Rc<str>>,
    /// cell attributes kept as written (cm, vm, ph)
    pub extra: Option<Rc<Vec<(String, String)>>>,
}

impl Cell {
    pub fn is_blank(&self) -> bool {
        self.f.is_none() && self.v.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RowInfo {
    pub ht: Option<f64>,
    pub custom_height: bool,
    pub hidden: bool,
    pub style: Option<u32>,
    pub outline: u8,
    pub collapsed: bool,
    pub extra: Vec<(String, String)>,
}

impl RowInfo {
    pub fn is_default(&self) -> bool {
        self.ht.is_none() && !self.hidden && self.style.is_none() && self.outline == 0 && !self.collapsed && self.extra.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColSpan {
    pub min: u32,
    pub max: u32,
    pub width: Option<f64>,
    pub custom: bool,
    pub hidden: bool,
    pub style: u32,
    pub best_fit: bool,
    pub outline: u8,
    pub collapsed: bool,
    pub extra: Vec<(String, String)>,
}

impl ColSpan {
    pub fn same_settings(&self, o: &ColSpan) -> bool {
        self.width == o.width && self.custom == o.custom && self.hidden == o.hidden && self.style == o.style
            && self.best_fit == o.best_fit && self.outline == o.outline && self.collapsed == o.collapsed && self.extra == o.extra
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub top_row: u32,
    pub left_col: u32,
    pub zoom: u32,
    pub freeze_rows: u32,
    pub freeze_cols: u32,
    pub show_grid: bool,
    pub show_headers: bool,
    pub active: (u32, u32),
    pub sel: Vec<Rect>,
    pub selected: bool,
    pub mode: String,
    pub changed: bool,
}

impl Default for View {
    fn default() -> Self {
        View {
            top_row: 0,
            left_col: 0,
            zoom: 100,
            freeze_rows: 0,
            freeze_cols: 0,
            show_grid: true,
            show_headers: true,
            active: (0, 0),
            sel: vec![Rect::cell(0, 0)],
            selected: false,
            mode: "normal".into(),
            changed: false,
        }
    }
}

/// A piece of the worksheet XML: an element the engine rewrites, or text kept as is.
#[derive(Clone, Debug)]
pub enum Piece {
    /// an element the engine writes itself (by local name)
    Known(String),
    /// (local name, the element's XML exactly as read)
    Raw(String, String),
}

#[derive(Clone, Debug)]
pub struct Sheet {
    pub id: u32,
    pub name: String,
    pub state: String,
    pub kind: String,
    pub tab_color: Option<u32>,
    pub cells: BTreeMap<(u32, u32), Cell>,
    pub colidx: BTreeSet<(u32, u32)>,
    pub rows: BTreeMap<u32, RowInfo>,
    pub cols: Vec<ColSpan>,
    pub default_col_width: Option<f64>,
    pub base_col_width: u32,
    pub default_row_height: f64,
    pub custom_row_height: bool,
    pub merges: Vec<Rect>,
    pub view: View,
    /// spilled ranges by anchor, and the values spilled into the other cells
    pub spills: HashMap<(u32, u32), Rect>,
    /// anchors whose spill is blocked, and the range they need
    pub blocked: HashMap<(u32, u32), Rect>,
    pub ghosts: BTreeMap<(u32, u32), Value>,
    /// legacy array formulas: anchor -> range
    pub arrays: HashMap<(u32, u32), Rect>,
    /// the AutoFilter range and its column filters
    pub autofilter: Option<crate::book_data::AutoFilter>,
    /// charts and other objects over the cells
    pub objects: Vec<crate::drawing::Obj>,
    /// the drawing part as read (None when the sheet had none, or it couldn't be read)
    pub drawing: Option<crate::drawing::DrawingFile>,
    // ---- file
    pub part: String,
    pub prolog: String,
    pub root_open: String,
    pub root_close: String,
    pub pieces: Vec<Piece>,
    /// the whole part for chart and dialog sheets (written back untouched)
    pub raw_part: Option<Vec<u8>>,
    /// raw sheetView / sheetFormatPr / sheetPr elements, rewritten only when changed
    pub raw_views: Option<String>,
    pub raw_sheetpr: Option<crate::xml::Element>,
    pub raw_format_pr: Option<crate::xml::Element>,
    pub error: Option<String>,
}

impl Sheet {
    pub fn new(id: u32, name: &str) -> Sheet {
        Sheet {
            id,
            name: name.to_string(),
            state: "visible".into(),
            kind: "work".into(),
            tab_color: None,
            cells: BTreeMap::new(),
            colidx: BTreeSet::new(),
            rows: BTreeMap::new(),
            cols: Vec::new(),
            default_col_width: None,
            base_col_width: 8,
            default_row_height: 15.0,
            custom_row_height: false,
            merges: Vec::new(),
            view: View::default(),
            spills: HashMap::new(),
            blocked: HashMap::new(),
            ghosts: BTreeMap::new(),
            arrays: HashMap::new(),
            autofilter: None,
            objects: Vec::new(),
            drawing: None,
            part: String::new(),
            prolog: String::new(),
            root_open: String::new(),
            root_close: String::new(),
            pieces: Vec::new(),
            raw_part: None,
            raw_views: None,
            raw_sheetpr: None,
            raw_format_pr: None,
            error: None,
        }
    }

    // ---- cells
    pub fn get(&self, r: u32, c: u32) -> Option<&Cell> {
        self.cells.get(&(r, c))
    }
    pub fn put(&mut self, r: u32, c: u32, cell: Option<Cell>) -> Option<Cell> {
        match cell {
            Some(cell) => {
                self.colidx.insert((c, r));
                self.cells.insert((r, c), cell)
            }
            None => {
                self.colidx.remove(&(c, r));
                self.cells.remove(&(r, c))
            }
        }
    }
    /// A cell's value (spilled values included).
    pub fn value(&self, r: u32, c: u32) -> Value {
        match self.cells.get(&(r, c)) {
            Some(cell) if !cell.is_blank() => cell.v.clone(),
            _ => self.ghosts.get(&(r, c)).cloned().unwrap_or(Value::Empty),
        }
    }
    pub fn has_content(&self, r: u32, c: u32) -> bool {
        match self.cells.get(&(r, c)) {
            Some(cell) if !cell.is_blank() => true,
            _ => self.ghosts.contains_key(&(r, c)),
        }
    }
    /// The style a cell has, or that an empty cell would get.
    pub fn style_at(&self, r: u32, c: u32) -> u32 {
        if let Some(cell) = self.cells.get(&(r, c)) {
            return cell.s;
        }
        if let Some(ri) = self.rows.get(&r) {
            if let Some(s) = ri.style {
                return s;
            }
        }
        self.col_style(c)
    }
    pub fn col_span(&self, c: u32) -> Option<&ColSpan> {
        // spans are sorted and don't overlap
        let i = self.cols.partition_point(|s| s.max < c);
        self.cols.get(i).filter(|s| s.min <= c)
    }
    pub fn col_style(&self, c: u32) -> u32 {
        self.col_span(c).map(|s| s.style).unwrap_or(0)
    }

    /// Bottom-right of the cells with content or formatting: (row, col).
    pub fn used_extent(&self) -> Option<(u32, u32)> {
        let mut mr: Option<u32> = None;
        let mut mc: Option<u32> = None;
        for ((r, c), cell) in self.cells.iter() {
            if cell.is_blank() && cell.s == 0 {
                continue;
            }
            mr = Some(mr.map_or(*r, |x| x.max(*r)));
            mc = Some(mc.map_or(*c, |x| x.max(*c)));
        }
        for (r, c) in self.ghosts.keys() {
            mr = Some(mr.map_or(*r, |x| x.max(*r)));
            mc = Some(mc.map_or(*c, |x| x.max(*c)));
        }
        for m in &self.merges {
            mr = Some(mr.map_or(m.r2, |x| x.max(m.r2)));
            mc = Some(mc.map_or(m.c2, |x| x.max(m.c2)));
        }
        match (mr, mc) {
            (Some(r), Some(c)) => Some((r, c)),
            _ => None,
        }
    }
    /// Extent of cells holding values only.
    pub fn data_extent(&self) -> Option<(u32, u32)> {
        let mut mr: Option<u32> = None;
        let mut mc: Option<u32> = None;
        for ((r, c), cell) in self.cells.iter() {
            if cell.is_blank() {
                continue;
            }
            mr = Some(mr.map_or(*r, |x| x.max(*r)));
            mc = Some(mc.map_or(*c, |x| x.max(*c)));
        }
        for (r, c) in self.ghosts.keys() {
            mr = Some(mr.map_or(*r, |x| x.max(*r)));
            mc = Some(mc.map_or(*c, |x| x.max(*c)));
        }
        match (mr, mc) {
            (Some(r), Some(c)) => Some((r, c)),
            _ => None,
        }
    }

    pub fn merge_at(&self, r: u32, c: u32) -> Option<Rect> {
        self.merges.iter().find(|m| m.contains(r, c)).copied()
    }

    // ---- rows and columns (sizes in points / character widths)
    pub fn row_hidden(&self, r: u32) -> bool {
        self.rows.get(&r).map(|x| x.hidden).unwrap_or(false)
    }
    pub fn col_hidden(&self, c: u32) -> bool {
        self.col_span(c).map(|x| x.hidden).unwrap_or(false)
    }
    /// Row height in points (hidden rows count as their stored height).
    pub fn row_height_pt(&self, r: u32) -> f64 {
        match self.rows.get(&r).and_then(|x| x.ht) {
            Some(h) => h,
            None => self.default_row_height,
        }
    }
    /// Column width in file units (characters + padding), not counting hidden.
    pub fn col_width_units(&self, c: u32, mdw: f64) -> f64 {
        match self.col_span(c).and_then(|s| s.width) {
            Some(w) => w,
            None => self.default_width_units(mdw),
        }
    }
    pub fn default_width_units(&self, mdw: f64) -> f64 {
        match self.default_col_width {
            Some(w) => w,
            None => {
                // baseColWidth characters plus 5 pixels of padding, rounded up to a
                // multiple of 8 pixels (64 px for 11 pt Calibri)
                let px = ((self.base_col_width as f64 * mdw + 5.0) / 8.0).ceil() * 8.0;
                ((px / mdw) * 256.0).trunc() / 256.0
            }
        }
    }

    /// Change the settings of columns c1..=c2 with `f`, keeping spans compact.
    pub fn map_cols(&mut self, c1: u32, c2: u32, mut f: impl FnMut(&mut ColSpan)) {
        let mut out: Vec<ColSpan> = Vec::new();
        let template = |min: u32, max: u32| ColSpan {
            min, max, width: None, custom: false, hidden: false, style: 0, best_fit: false, outline: 0, collapsed: false, extra: vec![],
        };
        let mut cur = c1;
        for s in self.cols.iter() {
            if s.max < c1 || s.min > c2 {
                out.push(s.clone());
                continue;
            }
            if s.min < c1 {
                let mut a = s.clone();
                a.max = c1 - 1;
                out.push(a);
            }
            if s.min > cur {
                let mut g = template(cur, s.min - 1);
                f(&mut g);
                out.push(g);
            }
            let mut m = s.clone();
            m.min = s.min.max(c1);
            m.max = s.max.min(c2);
            f(&mut m);
            out.push(m);
            cur = s.max.min(c2) + 1;
            if s.max > c2 {
                let mut b = s.clone();
                b.min = c2 + 1;
                out.push(b);
            }
        }
        if cur <= c2 && cur >= c1 {
            let mut g = template(cur, c2);
            f(&mut g);
            out.push(g);
        }
        out.sort_by_key(|s| s.min);
        // merge neighbours with the same settings; drop all-default spans
        let mut merged: Vec<ColSpan> = Vec::new();
        for s in out {
            let is_default = s.width.is_none() && !s.hidden && s.style == 0 && s.outline == 0 && !s.collapsed && s.extra.is_empty();
            if is_default {
                continue;
            }
            if let Some(last) = merged.last_mut() {
                if last.max + 1 == s.min && last.same_settings(&s) {
                    last.max = s.max;
                    continue;
                }
            }
            merged.push(s);
        }
        self.cols = merged;
    }

    /// Cells in a rectangle (row-major), skipping empty positions.
    pub fn cells_in(&self, rc: Rect) -> impl Iterator<Item = (&(u32, u32), &Cell)> {
        self.cells.range((rc.r1, rc.c1)..=(rc.r2, rc.c2)).filter(move |((_, c), _)| *c >= rc.c1 && *c <= rc.c2)
    }

    /// Positions of cells with content in a rectangle, cheaply for tall thin ranges.
    pub fn positions_in(&self, rc: Rect) -> Vec<(u32, u32)> {
        let width = rc.cols() as u64;
        if width <= 4 && rc.rows() as u64 > 1000 {
            let mut v = Vec::new();
            for c in rc.c1..=rc.c2 {
                for (cc, r) in self.colidx.range((c, rc.r1)..=(c, rc.r2)) {
                    let _ = cc;
                    v.push((*r, c));
                }
            }
            v.sort();
            return v;
        }
        self.cells_in(rc).map(|(k, _)| *k).collect()
    }

    pub fn clamp_rect(&self, rc: Rect) -> Rect {
        Rect::new(rc.r1.min(MAX_ROWS - 1), rc.c1.min(MAX_COLS - 1), rc.r2.min(MAX_ROWS - 1), rc.c2.min(MAX_COLS - 1))
    }
}

/// Characters of a column width without the padding, as Column Width shows it.
pub fn units_to_chars(units: f64, mdw: f64) -> f64 {
    let px = units_to_px(units, mdw);
    if px <= 0.0 {
        return 0.0;
    }
    let chars = ((px - 5.0) / mdw * 100.0 + 0.5).trunc() / 100.0;
    chars.max(0.0)
}

/// Pixels (at 100 %) of a width in file units.
pub fn units_to_px(units: f64, mdw: f64) -> f64 {
    ((256.0 * units + (128.0 / mdw).trunc()) / 256.0 * mdw).trunc()
}

/// File units for a width in characters typed into Column Width.
pub fn chars_to_units(chars: f64, mdw: f64) -> f64 {
    if chars <= 0.0 {
        return 0.0;
    }
    ((chars * mdw + 5.0) / mdw * 256.0).trunc() / 256.0
}

pub fn px_to_units(px: f64, mdw: f64) -> f64 {
    if px <= 0.0 {
        return 0.0;
    }
    ((px / mdw) * 256.0).trunc() / 256.0
}

pub fn pt_to_px(pt: f64) -> f64 {
    (pt * 96.0 / 72.0).round()
}

pub fn px_to_pt(px: f64) -> f64 {
    (px * 72.0 / 96.0 * 4.0).round() / 4.0
}
