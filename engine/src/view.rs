//! Where things are on screen: row and column positions, the bands the grid shows,
//! hit testing, and AutoFit.

use crate::book::Book;
use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::layout::font_of;
use crate::sheet::{pt_to_px, px_to_pt, units_to_px, Sheet};
use crate::value::Value;

/// What the window passes in for drawing and hit testing.
#[derive(Clone, Debug)]
pub struct ViewParams {
    pub width: f64,
    pub height: f64,
    pub zoom: f64,
    pub top_row: u32,
    pub left_col: u32,
    pub freeze_rows: u32,
    pub freeze_cols: u32,
    /// Split windows: the top (left) pane shows rows pane_top..freeze_rows (columns
    /// pane_left..freeze_cols) and scrolls on its own; 0 for frozen panes.
    pub pane_top: u32,
    pub pane_left: u32,
    /// draw split bars instead of freeze lines
    pub split: bool,
    pub show_grid: bool,
    pub show_headers: bool,
    pub sel: Vec<Rect>,
    pub active: (u32, u32),
    pub editing: Option<(u32, u32)>,
    pub copy: Option<Rect>,
    pub ants_phase: f64,
    pub refs: Vec<(Rect, u32)>,
    pub show_formulas: bool,
    pub arrows: Vec<(Rect, (u32, u32), u32)>,
    pub dark: bool,
    /// the range the fill handle is being dragged over
    pub fill_target: Option<Rect>,
    /// selected objects (charts) and the outline of one being moved or sized (screen px)
    pub sel_objects: Vec<usize>,
    pub obj_preview: Option<(f64, f64, f64, f64)>,
    /// the note shown while the pointer is over its cell, and one being edited (not drawn)
    pub hover_note: Option<(u32, u32)>,
    pub edit_note: Option<(u32, u32)>,
    /// drawing for paper: no selection, buttons or markers
    pub print: bool,
    /// page break preview: the print area and page breaks over the cells
    pub page_breaks: bool,
    /// Circle Invalid Data: the cells to ring in red
    pub dv_circles: Vec<(u32, u32)>,
}

impl Default for ViewParams {
    fn default() -> Self {
        ViewParams {
            width: 800.0,
            height: 600.0,
            zoom: 1.0,
            top_row: 0,
            left_col: 0,
            freeze_rows: 0,
            freeze_cols: 0,
            pane_top: 0,
            pane_left: 0,
            split: false,
            show_grid: true,
            show_headers: true,
            sel: vec![Rect::cell(0, 0)],
            active: (0, 0),
            editing: None,
            copy: None,
            ants_phase: 0.0,
            refs: vec![],
            show_formulas: false,
            arrows: vec![],
            dark: false,
            fill_target: None,
            sel_objects: vec![],
            obj_preview: None,
            hover_note: None,
            edit_note: None,
            print: false,
            page_breaks: false,
            dv_circles: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Geometry {
    pub hw: f64,
    pub hh: f64,
    /// (row, y, height) of every row on screen, frozen rows first
    pub rows: Vec<(u32, f64, f64)>,
    pub cols: Vec<(u32, f64, f64)>,
    /// the frozen band and the scrolling band: (start, end)
    pub row_spans: Vec<(f64, f64)>,
    pub col_spans: Vec<(f64, f64)>,
}

pub const HEADER_H: f64 = 20.0;

impl Book {
    // ------------------------------------------------------------------ sizes at 100 %
    pub fn default_row_px(&self, s: usize) -> f64 {
        pt_to_px(self.default_row_pt(s))
    }

    pub fn default_row_pt(&self, s: usize) -> f64 {
        self.sheets[s].default_row_height
    }

    pub fn row_px(&self, s: usize, r: u32) -> f64 {
        let sh = &self.sheets[s];
        match sh.rows.get(&r) {
            Some(ri) if ri.hidden => 0.0,
            Some(ri) => match ri.ht {
                Some(h) => pt_to_px(h),
                None => self.default_row_px(s),
            },
            None => self.default_row_px(s),
        }
    }

    pub fn row_height_pt(&self, s: usize, r: u32) -> f64 {
        let sh = &self.sheets[s];
        match sh.rows.get(&r).and_then(|ri| ri.ht) {
            Some(h) => h,
            None => sh.default_row_height,
        }
    }

    pub fn col_units(&self, s: usize, c: u32) -> f64 {
        let mdw = self.mdw(s);
        self.sheets[s].col_width_units(c, mdw)
    }

    pub fn col_px(&self, s: usize, c: u32) -> f64 {
        let sh = &self.sheets[s];
        if sh.col_hidden(c) {
            return 0.0;
        }
        let mdw = self.mdw(s);
        units_to_px(sh.col_width_units(c, mdw), mdw)
    }

    /// Top of row r in pixels at 100 % (headers not counted).
    pub fn row_y(&self, s: usize, r: u32) -> f64 {
        let dp = self.default_row_px(s);
        let mut y = r as f64 * dp;
        for (k, _) in self.sheets[s].rows.range(..r) {
            y += self.row_px(s, *k) - dp;
        }
        y
    }

    pub fn col_x(&self, s: usize, c: u32) -> f64 {
        let sh = &self.sheets[s];
        let mdw = self.mdw(s);
        let dw = units_to_px(sh.default_width_units(mdw), mdw);
        let mut x = c as f64 * dw;
        for span in &sh.cols {
            if span.min >= c {
                break;
            }
            let hi = span.max.min(c - 1);
            let n = (hi - span.min + 1) as f64;
            let w = if span.hidden { 0.0 } else { units_to_px(span.width.unwrap_or(sh.default_width_units(mdw)), mdw) };
            x += n * (w - dw);
        }
        x
    }

    pub fn row_at(&self, s: usize, y: f64) -> u32 {
        let dp = self.default_row_px(s);
        let mut base_r: u32 = 0;
        let mut base_y = 0.0;
        for (k, _) in self.sheets[s].rows.iter() {
            let h = self.row_px(s, *k);
            if h == dp {
                continue;
            }
            let end = base_y + (*k - base_r) as f64 * dp;
            if y < end {
                return (base_r + ((y - base_y) / dp).floor().max(0.0) as u32).min(MAX_ROWS - 1);
            }
            if y < end + h {
                return *k;
            }
            base_y = end + h;
            base_r = *k + 1;
        }
        ((base_r as f64 + ((y - base_y) / dp).floor().max(0.0)) as u64).min(MAX_ROWS as u64 - 1) as u32
    }

    pub fn col_at(&self, s: usize, x: f64) -> u32 {
        let sh = &self.sheets[s];
        let mdw = self.mdw(s);
        let dw = units_to_px(sh.default_width_units(mdw), mdw);
        let mut base_c: u32 = 0;
        let mut base_x = 0.0;
        for span in &sh.cols {
            let w = if span.hidden { 0.0 } else { units_to_px(span.width.unwrap_or(sh.default_width_units(mdw)), mdw) };
            let gap_end = base_x + (span.min - base_c) as f64 * dw;
            if x < gap_end {
                return (base_c + ((x - base_x) / dw).floor().max(0.0) as u32).min(MAX_COLS - 1);
            }
            let n = (span.max - span.min + 1) as f64;
            if w > 0.0 && x < gap_end + n * w {
                return span.min + ((x - gap_end) / w).floor() as u32;
            }
            base_x = gap_end + n * w;
            base_c = span.max + 1;
        }
        ((base_c as f64 + ((x - base_x) / dw).floor().max(0.0)) as u64).min(MAX_COLS as u64 - 1) as u32
    }

    // ------------------------------------------------------------------ the bands on screen
    fn header_width(&self, s: usize, last_row: u32, zoom: f64) -> f64 {
        let digits = (last_row + 1).to_string().len().max(3) as f64;
        let _ = s;
        ((digits * 7.0 + 6.0) * zoom).round()
    }

    pub fn geometry(&self, s: usize, p: &ViewParams) -> Geometry {
        let z = p.zoom;
        let hh = if p.show_headers { (HEADER_H * z).round() } else { 0.0 };
        let mut g = Geometry { hh, ..Default::default() };
        // rows
        let mut y = hh;
        let fr = p.freeze_rows.min(MAX_ROWS);
        for r in p.pane_top.min(fr)..fr {
            let h = self.row_px(s, r) * z;
            if h > 0.0 {
                g.rows.push((r, y, h));
            }
            y += h;
        }
        let split_y = y;
        let mut r = p.top_row.max(fr);
        while y < p.height && r < MAX_ROWS {
            let h = self.row_px(s, r) * z;
            if h > 0.0 {
                g.rows.push((r, y, h));
                y += h;
            }
            r += 1;
            if r - p.top_row > 100_000 {
                break;
            }
        }
        g.row_spans = if fr > 0 { vec![(hh, split_y), (split_y, p.height.max(split_y))] } else { vec![(hh, p.height)] };
        let last_row = g.rows.last().map(|x| x.0).unwrap_or(0);
        let hw = if p.show_headers { self.header_width(s, last_row, z) } else { 0.0 };
        g.hw = hw;
        let mut x = hw;
        let fc = p.freeze_cols.min(MAX_COLS);
        for c in p.pane_left.min(fc)..fc {
            let w = self.col_px(s, c) * z;
            if w > 0.0 {
                g.cols.push((c, x, w));
            }
            x += w;
        }
        let split_x = x;
        let mut c = p.left_col.max(fc);
        while x < p.width && c < MAX_COLS {
            let w = self.col_px(s, c) * z;
            if w > 0.0 {
                g.cols.push((c, x, w));
                x += w;
            }
            c += 1;
        }
        g.col_spans = if fc > 0 { vec![(hw, split_x), (split_x, p.width.max(split_x))] } else { vec![(hw, p.width)] };
        g
    }

    /// x of a column's left edge on screen (may be off screen), given the geometry.
    pub fn screen_x(&self, s: usize, p: &ViewParams, g: &Geometry, c: u32) -> f64 {
        let fc = p.freeze_cols;
        let pl = p.pane_left.min(fc);
        if c < fc {
            return g.hw + (self.col_x(s, c) - self.col_x(s, pl)) * p.zoom;
        }
        let split = g.hw + (self.col_x(s, fc) - self.col_x(s, pl)) * p.zoom;
        let left = p.left_col.max(fc);
        split + (self.col_x(s, c) - self.col_x(s, left)) * p.zoom
    }

    pub fn screen_y(&self, s: usize, p: &ViewParams, g: &Geometry, r: u32) -> f64 {
        let fr = p.freeze_rows;
        let pt = p.pane_top.min(fr);
        if r < fr {
            return g.hh + (self.row_y(s, r) - self.row_y(s, pt)) * p.zoom;
        }
        let split = g.hh + (self.row_y(s, fr) - self.row_y(s, pt)) * p.zoom;
        let top = p.top_row.max(fr);
        split + (self.row_y(s, r) - self.row_y(s, top)) * p.zoom
    }

    /// Screen rectangle of a cell (a merged area in whole), or None when off screen.
    pub fn cell_rect(&self, s: usize, r: u32, c: u32, p: &ViewParams) -> Option<(f64, f64, f64, f64)> {
        let g = self.geometry(s, p);
        let rc = self.sheets[s].merge_at(r, c).unwrap_or(Rect::cell(r, c));
        let x1 = self.screen_x(s, p, &g, rc.c1);
        let x2 = self.screen_x(s, p, &g, rc.c2) + self.col_px(s, rc.c2) * p.zoom;
        let y1 = self.screen_y(s, p, &g, rc.r1);
        let y2 = self.screen_y(s, p, &g, rc.r2) + self.row_px(s, rc.r2) * p.zoom;
        // in a scrolled-away part of the scrolling band?
        let fc = p.freeze_cols;
        let fr = p.freeze_rows;
        if rc.c2 >= fc && rc.c2 < p.left_col.max(fc) && rc.c1 >= fc {
            return None;
        }
        if rc.r2 >= fr && rc.r2 < p.top_row.max(fr) && rc.r1 >= fr {
            return None;
        }
        // above (left of) what a split's top (left) pane shows
        if rc.r2 < p.pane_top.min(fr) || rc.c2 < p.pane_left.min(fc) {
            return None;
        }
        if x1 >= p.width || y1 >= p.height || x2 <= g.hw || y2 <= g.hh {
            return None;
        }
        Some((x1, y1, x2 - x1, y2 - y1))
    }

    /// What is under a point.
    pub fn hit(&self, s: usize, x: f64, y: f64, p: &ViewParams) -> Vec<HitPart> {
        let g = self.geometry(s, p);
        if p.show_headers && x < g.hw && y < g.hh {
            return vec![HitPart::S("corner")];
        }
        let grip = 3.0;
        if p.show_headers && y < g.hh {
            // a column edge?
            for (c, cx, w) in g.cols.iter().rev() {
                if (x - (cx + w)).abs() <= grip {
                    return vec![HitPart::S("colresize"), HitPart::N(*c)];
                }
            }
            return match self.band_col(&g, x) {
                Some(c) => vec![HitPart::S("colhdr"), HitPart::N(c)],
                None => vec![HitPart::S("out")],
            };
        }
        if p.show_headers && x < g.hw {
            for (r, ry, h) in g.rows.iter().rev() {
                if (y - (ry + h)).abs() <= grip.min(*h / 2.0).max(2.0) {
                    return vec![HitPart::S("rowresize"), HitPart::N(*r)];
                }
            }
            return match self.band_row(&g, y) {
                Some(r) => vec![HitPart::S("rowhdr"), HitPart::N(r)],
                None => vec![HitPart::S("out")],
            };
        }
        // a filter button (sheet AutoFilter or a table's header)
        if let (Some(r), Some(c)) = (self.band_row(&g, y), self.band_col(&g, x)) {
            for (hr, c1, c2, _) in self.filter_headers(s) {
                if r == hr && c >= c1 && c <= c2 {
                    let (_, ry, rh) = *g.rows.iter().find(|x| x.0 == r).unwrap();
                    let (_, cx, cw) = *g.cols.iter().find(|x| x.0 == c).unwrap();
                    let (bx, by, bs) = crate::render::filter_button(cx, ry, cw, rh, p.zoom);
                    if x >= bx && x <= bx + bs && y >= by && y <= by + bs {
                        return vec![HitPart::S("filterbtn"), HitPart::N(r), HitPart::N(c)];
                    }
                }
            }
            // the validation list's drop-down beside the active cell
            let (ar, ac) = p.active;
            if c == ac + 1 && r == ar && self.dv_dropdown_at(s, ar, ac) {
                let (_, ry, rh) = *g.rows.iter().find(|x| x.0 == r).unwrap();
                let (_, cx, _) = *g.cols.iter().find(|x| x.0 == c).unwrap();
                let (_, by, bs) = crate::render::filter_button(0.0, ry, 40.0, rh, p.zoom);
                if x >= cx + 2.0 && x <= cx + 2.0 + bs && y >= by && y <= by + bs {
                    return vec![HitPart::S("dvbtn"), HitPart::N(ar), HitPart::N(ac)];
                }
            }
            // the totals drop-down beside the active cell
            if c == ac + 1 && r == ar && self.sheets[s].tables.iter().any(|t| t.totals_row() == Some(ar) && ac >= t.rect.c1 && ac <= t.rect.c2) {
                let (_, ry, rh) = *g.rows.iter().find(|x| x.0 == r).unwrap();
                let (_, cx, _) = *g.cols.iter().find(|x| x.0 == c).unwrap();
                let (_, by, bs) = crate::render::filter_button(0.0, ry, 40.0, rh, p.zoom);
                if x >= cx + 2.0 && x <= cx + 2.0 + bs && y >= by && y <= by + bs {
                    return vec![HitPart::S("totalsbtn"), HitPart::N(ar), HitPart::N(ac)];
                }
            }
        }
        // the fill handle at the bottom right of the selection
        if p.sel.len() == 1 {
            let rg = p.sel[0];
            let r2 = rg.r2.min(MAX_ROWS - 1);
            let c2 = rg.c2.min(MAX_COLS - 1);
            if let Some((cx, cy, cw, ch)) = self.cell_rect(s, r2, c2, p) {
                let (hx, hy) = (cx + cw, cy + ch);
                if (x - hx).abs() <= 4.0 && (y - hy).abs() <= 4.0 {
                    return vec![HitPart::S("fill")];
                }
            }
        }
        match (self.band_row(&g, y), self.band_col(&g, x)) {
            (Some(r), Some(c)) => vec![HitPart::S("cell"), HitPart::N(r), HitPart::N(c)],
            _ => vec![HitPart::S("out")],
        }
    }

    fn band_row(&self, g: &Geometry, y: f64) -> Option<u32> {
        for (r, ry, h) in &g.rows {
            if y >= *ry && y < ry + h {
                return Some(*r);
            }
        }
        // past the last row on screen (the bottom part row)
        if let Some((r, ry, h)) = g.rows.last() {
            if y >= ry + h {
                return Some(*r);
            }
        }
        None
    }

    fn band_col(&self, g: &Geometry, x: f64) -> Option<u32> {
        for (c, cx, w) in &g.cols {
            if x >= *cx && x < cx + w {
                return Some(*c);
            }
        }
        if let Some((c, cx, w)) = g.cols.last() {
            if x >= cx + w {
                return Some(*c);
            }
        }
        None
    }

    // ------------------------------------------------------------------ widths and heights
    pub fn col_width_chars(&self, s: usize, c: u32) -> f64 {
        let mdw = self.mdw(s);
        crate::sheet::units_to_chars(self.col_units(s, c), mdw)
    }

    pub fn default_col_width_chars(&self, s: usize) -> f64 {
        let mdw = self.mdw(s);
        crate::sheet::units_to_chars(self.sheets[s].default_width_units(mdw), mdw)
    }

    pub fn px_to_width(&self, s: usize, px: f64) -> f64 {
        let mdw = self.mdw(s);
        crate::sheet::units_to_chars(crate::sheet::px_to_units(px, mdw), mdw)
    }

    pub fn set_col_width(&mut self, s: usize, c1: u32, c2: u32, chars: f64) {
        let mdw = self.mdw(s);
        let units = crate::sheet::chars_to_units(chars, mdw);
        self.set_col_width_units(s, c1, c2, units, "Column Width");
    }

    pub fn set_col_px(&mut self, s: usize, c1: u32, c2: u32, px: f64) {
        let mdw = self.mdw(s);
        let units = crate::sheet::px_to_units(px.round(), mdw);
        self.set_col_width_units(s, c1, c2, units, "Column Width");
    }

    pub fn set_row_height(&mut self, s: usize, r1: u32, r2: u32, pt: f64) {
        // heights are whole pixels on screen: 32 pt is stored as 32.25 (43 px)
        let pt = if pt > 0.0 { px_to_pt(pt_to_px(pt)) } else { pt };
        self.set_row_height_pt(s, r1, r2, pt, "Row Height", true);
    }

    pub fn set_row_px(&mut self, s: usize, r1: u32, r2: u32, px: f64) {
        self.set_row_height_pt(s, r1, r2, px_to_pt(px.round()), "Row Height", true);
    }

    /// The width a column needs for what it shows (in pixels at 100 %), None when empty.
    fn needed_width(&self, s: usize, c: u32, rows: Option<(u32, u32)>) -> Option<f64> {
        let sh = &self.sheets[s];
        let rc = match rows {
            Some((a, b)) => Rect::new(a, c, b, c),
            None => Rect::new(0, c, MAX_ROWS - 1, c),
        };
        let mut best: Option<f64> = None;
        let mut visit = |r: u32, v: &Value, xf: u32, sh: &Sheet| {
            if matches!(v, Value::Empty) {
                return;
            }
            if let Some(m) = sh.merge_at(r, c) {
                if m.c1 != m.c2 || m.r1 != m.r2 {
                    return;
                }
            }
            let x = self.styles.xf(xf);
            let text = match v {
                Value::Num(_) => self.format_value(v, xf, 255).text,
                _ => self.format_value(v, xf, 255).text,
            };
            let f = font_of(&self.styles, xf);
            let mut w = if x.wrap {
                text.split('\n').map(|l| self.layout.text_width(l, &f)).fold(0.0, f64::max)
            } else {
                self.layout.text_width(&text.replace('\n', " "), &f)
            };
            w += x.indent as f64 * 3.0 * self.mdw(s);
            best = Some(best.map_or(w, |b: f64| b.max(w)));
        };
        for (r, cc) in sh.positions_in(rc) {
            let cell = &sh.cells[&(r, cc)];
            let v = sh.value(r, cc);
            visit(r, &v, cell.s, sh);
        }
        for ((r, cc), v) in sh.ghosts.iter() {
            if *cc == c && rc.contains(*r, *cc) && !sh.cells.contains_key(&(*r, *cc)) {
                visit(*r, v, sh.style_at(*r, *cc), sh);
            }
        }
        best
    }

    pub fn autofit_cols(&mut self, s: usize, c1: u32, c2: u32, rows: Option<(u32, u32)>) {
        let mdw = self.mdw(s);
        let mut widths = Vec::new();
        for c in c1..=c2.min(MAX_COLS - 1) {
            if self.sheets[s].col_hidden(c) {
                continue;
            }
            if let Some(w) = self.needed_width(s, c, rows) {
                // text plus the cell's padding, in whole pixels
                let px = (w + 7.0).ceil().max(1.0);
                widths.push((c, crate::sheet::px_to_units(px, mdw).min(255.0)));
            }
            if c2 - c1 > 20000 {
                break;
            }
        }
        if widths.is_empty() {
            return;
        }
        let mut sh = Sheet::new(0, "");
        sh.cols = self.sheets[s].cols.clone();
        for (c, units) in widths {
            sh.map_cols(c, c, |span| {
                span.width = Some(units);
                span.custom = true;
                span.best_fit = true;
            });
        }
        self.begin("AutoFit Column Width", s);
        if sh.cols != self.sheets[s].cols {
            self.record(crate::book::Delta::Cols(s, sh.cols));
        }
        self.commit();
    }

    /// The height a row needs (points).
    pub fn needed_height(&self, s: usize, r: u32) -> f64 {
        let sh = &self.sheets[s];
        let base = {
            let xf = sh.rows.get(&r).and_then(|ri| ri.style).unwrap_or(0);
            self.layout.row_height_pt(&font_of(&self.styles, xf))
        };
        let mut h: f64 = base.max(self.layout.row_height_pt(&font_of(&self.styles, 0)).min(base));
        let rc = Rect::new(r, 0, r, MAX_COLS - 1);
        let mdw = self.mdw(s);
        for ((rr, c), cell) in sh.cells_in(rc) {
            let _ = rr;
            let v = sh.value(r, *c);
            if matches!(v, Value::Empty) && cell.s == 0 {
                continue;
            }
            if let Some(m) = sh.merge_at(r, *c) {
                if m.r1 != m.r2 {
                    continue;
                }
            }
            let f = font_of(&self.styles, cell.s);
            let line = self.layout.row_height_pt(&f);
            let x = self.styles.xf(cell.s);
            let mut need = if matches!(v, Value::Empty) { line } else { line };
            if !matches!(v, Value::Empty) && (x.wrap || matches!(&v, Value::Str(t) if t.contains('\n'))) {
                let text = self.format_value(&v, cell.s, 255).text;
                let width_px = match sh.merge_at(r, *c) {
                    Some(m) => (m.c1..=m.c2).map(|k| self.col_px(s, k)).sum::<f64>(),
                    None => self.col_px(s, *c),
                } - 6.0;
                let lines = if x.wrap {
                    let (_, hpx) = self.layout.measure_wrapped(&text, &f, 1.0, width_px.max(1.0));
                    let (_, one) = self.layout.measure("Ag", &f, 1.0);
                    (hpx / one).round().max(1.0)
                } else {
                    text.split('\n').count() as f64
                };
                need = (lines * line * 4.0).round() / 4.0;
                if lines > 1.0 {
                    // lines after the first are a little tighter than the row padding
                    need = line + (lines - 1.0) * (self.layout.line_height(&f).0 * 0.75 * 4.0).round() / 4.0;
                }
            }
            if x.rotation != 0 && x.rotation != 255 && !matches!(v, Value::Empty) {
                let text = self.format_value(&v, cell.s, 255).text;
                let w = self.layout.text_width(&text, &f) * 0.75;
                let a = (x.rotation as f64 % 90.0).to_radians();
                need = need.max((w * a.sin() + line * a.cos()).ceil());
            }
            h = h.max(need);
        }
        let _ = mdw;
        h.min(409.5)
    }

    pub fn autofit_rows(&mut self, s: usize, r1: u32, r2: u32) {
        let def = self.sheets[s].default_row_height;
        self.begin("AutoFit Row Height", s);
        let r2 = r2.min(MAX_ROWS - 1);
        // rows without settings or contents are already at their natural height
        let mut rows: std::collections::BTreeSet<u32> = self.sheets[s].rows.range(r1..=r2).map(|(r, _)| *r).collect();
        for ((r, _), _) in self.sheets[s].cells.range((r1, 0)..=(r2, MAX_COLS)) {
            rows.insert(*r);
        }
        for r in rows {
            let need = self.needed_height(s, r);
            let mut ri = self.sheets[s].rows.get(&r).cloned().unwrap_or_default();
            if ri.hidden {
                continue;
            }
            if (need - def).abs() < 0.01 {
                ri.ht = None;
            } else {
                ri.ht = Some(need);
            }
            ri.custom_height = false;
            if self.sheets[s].rows.get(&r).cloned().unwrap_or_default() != ri {
                self.record(crate::book::Delta::Row(s, r, Some(ri)));
            }
        }
        self.commit();
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum HitPart {
    S(&'static str),
    N(u32),
}
