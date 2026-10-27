//! Formatting commands: style changes, the Borders menu presets, Format Cells border
//! layouts, Format Painter, merges, and what Format Cells shows.

use crate::book::{Book, Delta};
use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::styles::{Change, Color, Styles};
use std::collections::HashMap;

pub type Pen = Option<(String, Option<u32>)>;

#[derive(Clone, Debug)]
pub enum StyleOp {
    Change(Change),
    /// a Borders menu preset: (preset, line style, colour)
    Preset(String, String, Option<u32>),
    /// Format Cells ▸ Border: left/right/top/bottom/insideV/insideH/diagUp/diagDown -> pen
    Layout(Vec<(String, Pen)>),
    /// set the style index outright (0 = Normal: Clear Formats)
    Set(u32),
    /// tile a block of style indices (Format Painter, Paste Formatting)
    Grid(Vec<Vec<u32>>),
}

fn whole_cols(rg: &Rect) -> bool {
    rg.r1 == 0 && rg.r2 >= MAX_ROWS - 1
}
fn whole_rows(rg: &Rect) -> bool {
    rg.c1 == 0 && rg.c2 >= MAX_COLS - 1
}

/// The sides a preset sets on a cell at (r, c) of range rg.
fn preset_sides(preset: &str, style: &str, color: Option<u32>, r: Option<u32>, c: Option<u32>, rg: &Rect) -> (Vec<(String, Pen)>, Option<(bool, bool)>) {
    let l = c == Some(rg.c1);
    let rt = c == Some(rg.c2);
    let t = r == Some(rg.r1);
    let b = r == Some(rg.r2);
    let pen = Some((style.to_string(), color));
    let medium = Some(("medium".to_string(), color));
    let double = Some(("double".to_string(), color));
    let mut d: Vec<(String, Pen)> = Vec::new();
    let mut put = |side: &str, p: &Pen| d.push((side.to_string(), p.clone()));
    let mut diag = None;
    match preset {
        "none" => {
            for s in ["left", "right", "top", "bottom", "diagonal"] {
                put(s, &None);
            }
            diag = Some((false, false));
        }
        "all" => {
            for s in ["left", "right", "top", "bottom"] {
                put(s, &pen);
            }
        }
        "outside" | "thick-outside" => {
            let p = if preset == "thick-outside" { &medium } else { &pen };
            if l {
                put("left", p);
            }
            if rt {
                put("right", p);
            }
            if t {
                put("top", p);
            }
            if b {
                put("bottom", p);
            }
        }
        "bottom" => {
            if b {
                put("bottom", &pen)
            }
        }
        "top" => {
            if t {
                put("top", &pen)
            }
        }
        "left" => {
            if l {
                put("left", &pen)
            }
        }
        "right" => {
            if rt {
                put("right", &pen)
            }
        }
        "bottom-double" => {
            if b {
                put("bottom", &double)
            }
        }
        "bottom-thick" => {
            if b {
                put("bottom", &medium)
            }
        }
        "top-bottom" => {
            if t {
                put("top", &pen);
            }
            if b {
                put("bottom", &pen);
            }
        }
        "top-thick-bottom" => {
            if t {
                put("top", &pen);
            }
            if b {
                put("bottom", &medium);
            }
        }
        "top-double-bottom" => {
            if t {
                put("top", &pen);
            }
            if b {
                put("bottom", &double);
            }
        }
        _ => {}
    }
    (d, diag)
}

fn layout_sides(lay: &[(String, Pen)], r: Option<u32>, c: Option<u32>, rg: &Rect) -> (Vec<(String, Pen)>, Option<(bool, bool)>) {
    let get = |k: &str| lay.iter().find(|(n, _)| n == k).map(|(_, p)| p.clone());
    let mut d: Vec<(String, Pen)> = Vec::new();
    let mut put = |side: &str, key: &str| {
        if let Some(p) = get(key) {
            d.push((side.to_string(), p));
        }
    };
    put("left", if c.is_none() || c == Some(rg.c1) { "left" } else { "insideV" });
    put("right", if c.is_none() || c == Some(rg.c2) { "right" } else { "insideV" });
    put("top", if r.is_none() || r == Some(rg.r1) { "top" } else { "insideH" });
    put("bottom", if r.is_none() || r == Some(rg.r2) { "bottom" } else { "insideH" });
    let mut diag = None;
    let up = get("diagUp");
    let down = get("diagDown");
    if up.is_some() || down.is_some() {
        let up = up.flatten();
        let down = down.flatten();
        let pen = up.clone().or_else(|| down.clone());
        d.push(("diagonal".into(), pen));
        diag = Some((up.is_some(), down.is_some()));
    }
    (d, diag)
}

impl Book {
    /// Apply formatting operations to ranges as one undoable step.
    pub fn restyle_ops(&mut self, s: usize, ranges: &[Rect], ops: &[StyleOp], label: &str) -> Result<bool, String> {
        if s >= self.sheets.len() || self.sheets[s].kind != "work" {
            return Err("This command works on worksheets only.".into());
        }
        let before = self.undo_stack.len();
        let edits0 = self.edits;
        self.begin(label, s);
        for op in ops {
            let r = self.apply_op(s, ranges, op);
            if let Err(e) = r {
                self.abort();
                return Err(e);
            }
        }
        self.commit();
        let _ = before;
        Ok(self.edits != edits0)
    }

    fn apply_op(&mut self, s: usize, ranges: &[Rect], op: &StyleOp) -> Result<(), String> {
        match op {
            StyleOp::Change(ch) => {
                let mut memo: HashMap<u32, u32> = HashMap::new();
                self.restyle(s, ranges, false, &mut |st, xf, _, _, _| *memo.entry(xf).or_insert_with(|| st.derive(xf, ch)))
            }
            StyleOp::Set(x) => {
                let x = *x;
                self.restyle(s, ranges, false, &mut |_, _, _, _, _| x)
            }
            StyleOp::Grid(grid) => {
                let h = grid.len().max(1);
                let w = grid.first().map(|r| r.len()).unwrap_or(1).max(1);
                self.restyle(s, ranges, true, &mut |_, _, r, c, rg| {
                    let i = r.map(|r| (r - rg.r1) as usize % h).unwrap_or(0);
                    let j = c.map(|c| (c - rg.c1) as usize % w).unwrap_or(0);
                    grid.get(i).and_then(|row| row.get(j)).copied().unwrap_or(0)
                })
            }
            StyleOp::Preset(preset, style, color) => {
                let mut memo: HashMap<(u32, String), u32> = HashMap::new();
                let p = preset.clone();
                let r = self.restyle(s, ranges, true, &mut |st, xf, r, c, rg| {
                    let (sides, diag) = preset_sides(&p, style, *color, r, c, &rg);
                    if sides.is_empty() && diag.is_none() {
                        return xf;
                    }
                    let key = (xf, format!("{:?}{:?}", sides, diag));
                    if let Some(v) = memo.get(&key) {
                        return *v;
                    }
                    let v = st.derive(xf, &Change { border: sides, diag, ..Default::default() });
                    memo.insert(key, v);
                    v
                });
                r?;
                if preset == "none" {
                    self.clear_neighbour_edges(s, ranges);
                }
                Ok(())
            }
            StyleOp::Layout(lay) => {
                let mut memo: HashMap<(u32, String), u32> = HashMap::new();
                self.restyle(s, ranges, true, &mut |st, xf, r, c, rg| {
                    let (sides, diag) = layout_sides(lay, r, c, &rg);
                    if sides.is_empty() && diag.is_none() {
                        return xf;
                    }
                    let key = (xf, format!("{:?}{:?}", sides, diag));
                    if let Some(v) = memo.get(&key) {
                        return *v;
                    }
                    let v = st.derive(xf, &Change { border: sides, diag, ..Default::default() });
                    memo.insert(key, v);
                    v
                })
            }
        }
    }

    /// No Border: the cells around lose the edges they share with the selection.
    fn clear_neighbour_edges(&mut self, s: usize, ranges: &[Rect]) {
        for rg in ranges {
            let mut nb: Vec<(Rect, &str)> = Vec::new();
            if rg.r1 > 0 && !whole_cols(rg) {
                nb.push((Rect::new(rg.r1 - 1, rg.c1, rg.r1 - 1, rg.c2), "bottom"));
            }
            if rg.r2 < MAX_ROWS - 1 && !whole_cols(rg) {
                nb.push((Rect::new(rg.r2 + 1, rg.c1, rg.r2 + 1, rg.c2), "top"));
            }
            if rg.c1 > 0 && !whole_rows(rg) {
                nb.push((Rect::new(rg.r1, rg.c1 - 1, rg.r2.min(rg.r1 + 100_000), rg.c1 - 1), "right"));
            }
            if rg.c2 < MAX_COLS - 1 && !whole_rows(rg) {
                nb.push((Rect::new(rg.r1, rg.c2 + 1, rg.r2.min(rg.r1 + 100_000), rg.c2 + 1), "left"));
            }
            for (area, side) in nb {
                let pos = self.sheets[s].positions_in(area);
                for (r, c) in pos {
                    let xf = self.sheets[s].cells[&(r, c)].s;
                    if xf == 0 {
                        continue;
                    }
                    let b = self.styles.border(self.styles.xf(xf).border).clone();
                    let has = match side {
                        "bottom" => b.bottom.is_some(),
                        "top" => b.top.is_some(),
                        "right" => b.right.is_some(),
                        _ => b.left.is_some(),
                    };
                    if has {
                        let n = self.styles.derive(xf, &Change { border: vec![(side.to_string(), None)], ..Default::default() });
                        self.set_cell_xf(s, r, c, n);
                    }
                }
            }
        }
    }

    /// Replace the merged areas that touch `ranges` (one step).
    pub fn set_merges_cmd(&mut self, s: usize, merges: Vec<Rect>, label: &str) {
        self.begin(label, s);
        let mut m: Vec<Rect> = merges.into_iter().filter(|r| r.count() > 1).collect();
        m.dedup();
        if m != self.sheets[s].merges {
            self.record(Delta::Merges(s, m));
        }
        self.commit();
    }

    /// Style indices of a block, row by row.
    pub fn xf_grid(&self, s: usize, rg: Rect) -> Vec<Vec<u32>> {
        let sh = &self.sheets[s];
        let r2 = rg.r2.min(rg.r1 + 999);
        let c2 = rg.c2.min(rg.c1 + 255);
        (rg.r1..=r2).map(|r| (rg.c1..=c2).map(|c| sh.style_at(r, c)).collect()).collect()
    }
}

/// Everything Format Cells shows about a style.
pub struct Spec {
    pub font_name: String,
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: Option<String>,
    pub color: Option<u32>,
    pub vert: Option<String>,
    pub fill_pattern: String,
    pub fill_fg: Option<u32>,
    pub fill_bg: Option<u32>,
    pub border: Vec<(String, String, Option<u32>)>,
    pub diag_up: bool,
    pub diag_down: bool,
    pub halign: String,
    pub valign: String,
    pub wrap: bool,
    pub shrink: bool,
    pub indent: u32,
    pub rotation: u32,
    pub reading: u32,
    pub locked: bool,
    pub hidden: bool,
    pub numfmt: String,
}

fn color_opt(st: &Styles, c: &Color) -> Option<u32> {
    match c {
        Color::Auto => None,
        Color::Indexed(64) | Color::Indexed(65) => None,
        // theme colours stay theme references (they follow the theme)
        Color::Theme(i, t) => Some(crate::styles::theme_ref(*i, *t)),
        c => Some(st.rgb(c, 0)),
    }
}

pub fn spec(st: &Styles, xi: u32) -> Spec {
    let x = st.xf(xi);
    let f = st.font(x.font);
    let fill = st.fill(x.fill);
    let b = st.border(x.border);
    let mut border = Vec::new();
    for (name, side) in [("left", &b.left), ("right", &b.right), ("top", &b.top), ("bottom", &b.bottom), ("diagonal", &b.diagonal)] {
        if let Some((style, c)) = side {
            border.push((name.to_string(), style.clone(), color_opt(st, c)));
        }
    }
    Spec {
        font_name: st.font_name(f),
        size: f.size,
        bold: f.bold,
        italic: f.italic,
        strike: f.strike,
        underline: match f.underline {
            0 => None,
            1 => Some("single".into()),
            2 => Some("double".into()),
            3 => Some("singleAccounting".into()),
            _ => Some("doubleAccounting".into()),
        },
        color: color_opt(st, &f.color),
        vert: match f.vert {
            1 => Some("superscript".into()),
            2 => Some("subscript".into()),
            _ => None,
        },
        fill_pattern: if fill.pattern.is_empty() { "none".into() } else { fill.pattern.clone() },
        fill_fg: fill.fg.as_ref().and_then(|c| color_opt(st, c)),
        fill_bg: fill.bg.as_ref().and_then(|c| color_opt(st, c)),
        border,
        diag_up: b.diag_up,
        diag_down: b.diag_down,
        halign: if x.halign.is_empty() { "general".into() } else { x.halign.clone() },
        valign: if x.valign.is_empty() { "bottom".into() } else { x.valign.clone() },
        wrap: x.wrap,
        shrink: x.shrink,
        indent: x.indent,
        rotation: x.rotation,
        reading: x.reading,
        locked: x.locked,
        hidden: x.hidden,
        numfmt: st.numfmt_code(x.numfmt),
    }
}
