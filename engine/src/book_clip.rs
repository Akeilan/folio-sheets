//! Copy, cut and paste: Paste Special's parts (formulas, values, formats, widths...),
//! operations, skip blanks, transpose and paste link; cut moves cells and the
//! references to them. Copies between workbooks travel as a small .xlsx.

use crate::book::{Book, Delta};
use crate::book_fmt::spec;
use crate::cellref::{Rect, MAX_COLS, MAX_ROWS};
use crate::formula::{self, Adjust};
use crate::sheet::{Cell, FKind, Sheet};
use crate::styles::{AlignChange, Change, FontChange, Styles};
use crate::value::Value;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum What {
    All,
    Formulas,
    Values,
    Formats,
    NoBorders,
    Widths,
    FormulasNumfmt,
    ValuesNumfmt,
    /// everything, and the source's column widths
    AllWidths,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    None,
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug)]
pub struct PasteOpts {
    pub what: What,
    pub op: Op,
    pub skip_blanks: bool,
    pub transpose: bool,
    pub link: bool,
}

impl Default for PasteOpts {
    fn default() -> Self {
        PasteOpts { what: What::All, op: Op::None, skip_blanks: false, transpose: false, link: false }
    }
}

/// A copied block, independent of where it came from.
struct Block {
    h: u32,
    w: u32,
    /// (dr, dc) -> cell (values of spills included as constants)
    cells: HashMap<(u32, u32), Cell>,
    /// style of every position (blank cells too)
    styles: Vec<Vec<u32>>,
    merges: Vec<Rect>,
    widths: Vec<Option<f64>>,
    src: Rect,
    src_sheet_name: String,
}

fn snapshot(sh: &Sheet, src: Rect) -> Block {
    let mut cells = HashMap::new();
    for ((r, c), cell) in sh.cells_in(src) {
        cells.insert((r - src.r1, c - src.c1), cell.clone());
    }
    for ((r, c), v) in sh.ghosts.iter() {
        if src.contains(*r, *c) {
            let e = cells.entry((r - src.r1, c - src.c1)).or_insert_with(|| Cell { s: sh.style_at(*r, *c), ..Default::default() });
            if e.is_blank() {
                e.v = v.clone();
            }
        }
    }
    let styles = (src.r1..=src.r2).map(|r| (src.c1..=src.c2).map(|c| sh.style_at(r, c)).collect()).collect();
    let merges = sh.merges.iter().filter(|m| src.contains_rect(m)).map(|m| Rect::new(m.r1 - src.r1, m.c1 - src.c1, m.r2 - src.r1, m.c2 - src.c1)).collect();
    let widths = (src.c1..=src.c2).map(|c| sh.col_span(c).and_then(|s| s.width)).collect();
    Block { h: src.rows(), w: src.cols(), cells, styles, merges, widths, src, src_sheet_name: sh.name.clone() }
}

/// A style of another workbook as a change that recreates it here.
fn import_change(st: &Styles, xf: u32) -> Change {
    let sp = spec(st, xf);
    let mut border: Vec<(String, Option<(String, Option<u32>)>)> = Vec::new();
    for side in ["left", "right", "top", "bottom", "diagonal"] {
        border.push((side.to_string(), sp.border.iter().find(|(s, _, _)| s == side).map(|(_, style, c)| (style.clone(), *c))));
    }
    Change {
        reset: true,
        font: Some(FontChange {
            name: Some(sp.font_name.clone()),
            size: Some(sp.size),
            grow: 0,
            bold: Some(sp.bold),
            italic: Some(sp.italic),
            underline: Some(sp.underline.clone()),
            strike: Some(sp.strike),
            color: Some(sp.color),
            vert: Some(sp.vert.clone()),
        }),
        fill: Some(if sp.fill_pattern == "none" { None } else { Some((sp.fill_pattern.clone(), sp.fill_fg, sp.fill_bg)) }),
        border,
        diag: Some((sp.diag_up, sp.diag_down)),
        align: Some(AlignChange {
            horizontal: Some(sp.halign.clone()),
            vertical: Some(sp.valign.clone()),
            wrap: Some(sp.wrap),
            shrink: Some(sp.shrink),
            indent: Some(sp.indent),
            indent_step: 0,
            rotation: Some(sp.rotation),
            reading: Some(sp.reading),
        }),
        locked: Some(sp.locked),
        hidden: Some(sp.hidden),
        numfmt: Some(sp.numfmt.clone()),
        quote_prefix: Some(st.xf(xf).quote_prefix),
    }
}

fn arith(op: Op, a: f64, b: f64) -> Value {
    match op {
        Op::Add => Value::num(a + b),
        Op::Sub => Value::num(a - b),
        Op::Mul => Value::num(a * b),
        Op::Div => {
            if b == 0.0 {
                Value::Err(crate::value::XlErr::Div0)
            } else {
                Value::num(a / b)
            }
        }
        Op::None => Value::num(b),
    }
}

impl Book {
    /// Paste a block of this workbook. `dst` is the selection: a block it holds a whole
    /// number of times is tiled, otherwise the block goes at its top-left. Returns the
    /// pasted area.
    pub fn paste_cells(&mut self, src_s: usize, src: Rect, dst_s: usize, dst: Rect, o: PasteOpts, cut: bool) -> Result<Rect, String> {
        let src = self.clip_rect(src_s, src);
        if cut && src_s == dst_s && o.what == What::All && !o.transpose {
            return self.move_cells(src_s, src, dst.r1, dst.c1);
        }
        let block = snapshot(&self.sheets[src_s], src);
        let with_cf = src_s == dst_s && !o.transpose && matches!(o.what, What::All | What::Formats | What::NoBorders | What::AllWidths);
        if with_cf {
            self.begin_group("Paste", dst_s);
        }
        let out = self.paste_block(&block, None, dst_s, dst, o, if src_s == dst_s { None } else { Some(&block.src_sheet_name) }, "Paste");
        let out = match out {
            Ok(o) => o,
            Err(e) => {
                if with_cf {
                    self.end_group();
                }
                return Err(e);
            }
        };
        if with_cf {
            // conditional formats come along (tiled like the cells)
            self.begin("Paste", dst_s);
            let (h, w) = (src.rows() as u32, src.cols() as u32);
            let mut r = out.r1;
            while r <= out.r2 {
                let mut c = out.c1;
                while c <= out.c2 {
                    self.copy_cf(src_s, src, dst_s, Rect::new(r, c, r + h - 1, c + w - 1));
                    c += w;
                }
                r += h;
            }
            self.commit();
            self.end_group();
        }
        if cut {
            // a cut to another sheet: the source empties
            self.clear_all(src_s, src, "Paste")?;
        }
        Ok(out)
    }

    /// Paste a block from another workbook (a copy that came through the clipboard).
    pub fn paste_from(&mut self, other: &Book, src_s: usize, src: Rect, dst_s: usize, dst: Rect, o: PasteOpts) -> Result<Rect, String> {
        let src = other.clip_rect(src_s, src);
        let mut block = snapshot(&other.sheets[src_s], src);
        // a clipboard workbook keeps the values the formulas had where they were copied
        if matches!(o.what, What::Values | What::ValuesNumfmt) {
            if let Some(vs) = other.sheets.iter().find(|x| x.name == "__values") {
                for ((i, j), cell) in block.cells.iter_mut() {
                    if cell.f.is_some() {
                        cell.f = None;
                        cell.v = vs.value(src.r1 + i, src.c1 + j);
                    }
                }
            }
        }
        self.paste_block(&block, Some(&other.styles), dst_s, dst, o, None, "Paste")
    }

    fn clip_rect(&self, s: usize, rc: Rect) -> Rect {
        let sh = &self.sheets[s];
        let (er, ec) = sh.used_extent().unwrap_or((0, 0));
        let mut rc = sh.clamp_rect(rc);
        if rc.rows() > 65536 {
            rc.r2 = rc.r2.min(er.max(rc.r1));
        }
        if rc.cols() > 1024 {
            rc.c2 = rc.c2.min(ec.max(rc.c1));
        }
        rc
    }

    fn clear_all(&mut self, s: usize, rc: Rect, label: &str) -> Result<(), String> {
        self.begin(label, s);
        for (r, c) in self.sheets[s].positions_in(rc) {
            let inherited = self.inherited_xf(s, r, c);
            let keep = if inherited != 0 { Some(Cell { s: 0, ..Default::default() }) } else { None };
            self.record(Delta::Cell(s, r, c, keep));
        }
        let merges: Vec<Rect> = self.sheets[s].merges.iter().filter(|m| !rc.contains_rect(m)).copied().collect();
        if merges.len() != self.sheets[s].merges.len() {
            self.record(Delta::Merges(s, merges));
        }
        self.commit();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn paste_block(&mut self, b: &Block, other_styles: Option<&Styles>, s: usize, dst: Rect, o: PasteOpts, src_sheet: Option<&str>, label: &str) -> Result<Rect, String> {
        if s >= self.sheets.len() || self.sheets[s].kind != "work" {
            return Err("This command works on worksheets only.".into());
        }
        let (bh, bw) = if o.transpose { (b.w, b.h) } else { (b.h, b.w) };
        // tiles
        let (th, tw) = if dst.rows() % bh == 0 && dst.cols() % bw == 0 && (dst.rows() > bh || dst.cols() > bw) && dst.rows() < MAX_ROWS && dst.cols() < MAX_COLS {
            (dst.rows() / bh, dst.cols() / bw)
        } else {
            (1, 1)
        };
        let out = Rect::new(dst.r1, dst.c1, dst.r1 + bh * th - 1, dst.c1 + bw * tw - 1);
        if out.r2 >= MAX_ROWS || out.c2 >= MAX_COLS {
            return Err("The information cannot be pasted because the Copy area and the paste area are not the same size and shape.".into());
        }
        for a in self.sheets[s].arrays.values() {
            if a.intersects(&out) && !out.contains_rect(a) {
                return Err("You can't change part of an array.".into());
            }
        }
        let formats = matches!(o.what, What::All | What::Formats | What::NoBorders | What::AllWidths);
        if !formats {
            for m in &self.sheets[s].merges {
                if m.intersects(&out) && !out.contains_rect(m) {
                    return Err("We can't do that to a merged cell.".into());
                }
            }
        }
        self.begin(label, s);
        let mut memo: HashMap<u32, u32> = HashMap::new();
        let mut map_style = |me: &mut Book, xf: u32| -> u32 {
            match other_styles {
                None => xf,
                Some(st) => *memo.entry(xf).or_insert_with(|| me.styles.derive(0, &import_change(st, xf))),
            }
        };
        let mut no_border: HashMap<u32, u32> = HashMap::new();
        for ti in 0..th {
            for tj in 0..tw {
                for i in 0..b.h {
                    for j in 0..b.w {
                        let (di, dj) = if o.transpose { (j, i) } else { (i, j) };
                        let r = dst.r1 + ti * bh + di;
                        let c = dst.c1 + tj * bw + dj;
                        let src_cell = b.cells.get(&(i, j));
                        if o.skip_blanks && src_cell.map(|x| x.is_blank()).unwrap_or(true) && !matches!(o.what, What::Formats | What::Widths) {
                            continue;
                        }
                        if o.what == What::Widths {
                            continue;
                        }
                        let cur = self.sheets[s].get(r, c).cloned();
                        let cur_xf = cur.as_ref().map(|x| x.s).unwrap_or_else(|| self.sheets[s].style_at(r, c));
                        let src_xf = map_style(self, b.styles[i as usize][j as usize]);
                        // the new contents
                        let mut cell = match o.what {
                            What::Formats | What::Widths => cur.clone().unwrap_or_default(),
                            _ => {
                                let mut n = src_cell.cloned().unwrap_or_default();
                                if o.link {
                                    let refr = formula::cell_text(b.src.r1 + i, b.src.c1 + j, false, false);
                                    let t = match src_sheet {
                                        Some(name) => format!("{}!{}", crate::cellref::quote_sheet(name), refr),
                                        None => refr,
                                    };
                                    n = Cell { f: Some(Rc::new(crate::book::make_formula(&t, FKind::Normal))), ..Default::default() };
                                } else if let Some(f) = &n.f {
                                    if matches!(o.what, What::Values | What::ValuesNumfmt) {
                                        n.f = None;
                                        if let Value::Arr(_) = n.v {
                                            n.v = Value::Empty;
                                        }
                                    } else {
                                        let dr = r as i64 - (b.src.r1 + i) as i64;
                                        let dc = c as i64 - (b.src.c1 + j) as i64;
                                        let mut nf = (**f).clone();
                                        nf.text = formula::adjust(&f.text, &Adjust::Shift(dr, dc));
                                        nf.expr = formula::parse(&nf.text).ok();
                                        if let FKind::Array(_) = nf.kind {
                                            nf.kind = FKind::Dynamic;
                                        }
                                        n.f = Some(Rc::new(nf));
                                    }
                                }
                                n.extra = None;
                                n
                            }
                        };
                        // an operation with what is there
                        if o.op != Op::None && !matches!(o.what, What::Formats | What::Widths) {
                            let old = cur.clone().unwrap_or_default();
                            let srcv = match &cell.v {
                                Value::Num(x) => Some(*x),
                                Value::Empty => Some(0.0),
                                _ => None,
                            };
                            if let Some(x) = srcv {
                                if cell.f.is_none() {
                                    if let Some(f) = &old.f {
                                        let sym = match o.op { Op::Add => "+", Op::Sub => "-", Op::Mul => "*", Op::Div => "/", Op::None => "" };
                                        let t = format!("({}){}{}", f.text, sym, crate::numfmt::num_to_text(x));
                                        cell = Cell { f: Some(Rc::new(crate::book::make_formula(&t, FKind::Dynamic))), ..old.clone() };
                                    } else {
                                        match &old.v {
                                            Value::Num(a) => cell = Cell { v: arith(o.op, *a, x), ..old.clone() },
                                            Value::Empty => cell = Cell { v: arith(o.op, 0.0, x), ..old.clone() },
                                            _ => cell = old.clone(),
                                        }
                                    }
                                }
                            } else {
                                cell = old.clone();
                            }
                        }
                        // the style
                        cell.s = match o.what {
                            What::All | What::Formats | What::AllWidths => src_xf,
                            What::NoBorders => *no_border.entry(src_xf).or_insert_with(|| {
                                self.styles.derive(src_xf, &Change { border: ["left", "right", "top", "bottom", "diagonal"].iter().map(|x| (x.to_string(), None)).collect(), diag: Some((false, false)), ..Default::default() })
                            }),
                            What::FormulasNumfmt | What::ValuesNumfmt => {
                                let code = self.styles.xf_numfmt(src_xf);
                                self.styles.derive(cur_xf, &Change { numfmt: Some(code), ..Default::default() })
                            }
                            _ => cur_xf,
                        };
                        if matches!(o.what, What::Formats) {
                            // contents stay
                        }
                        let blank = cell.is_blank() && cell.rich.is_none();
                        let keep = !(blank && cell.s == self.inherited_xf(s, r, c));
                        let newc = if keep { Some(cell) } else { None };
                        let same = match (&cur, &newc) {
                            (None, None) => true,
                            _ => false,
                        };
                        if !same {
                            self.record(Delta::Cell(s, r, c, newc));
                        }
                    }
                }
            }
        }
        // merges travel with formats
        if formats && !o.transpose {
            let mut merges: Vec<Rect> = self.sheets[s].merges.iter().filter(|m| !m.intersects(&out)).copied().collect();
            for ti in 0..th {
                for tj in 0..tw {
                    for m in &b.merges {
                        merges.push(Rect::new(dst.r1 + ti * bh + m.r1, dst.c1 + tj * bw + m.c1, dst.r1 + ti * bh + m.r2, dst.c1 + tj * bw + m.c2));
                    }
                }
            }
            if merges != self.sheets[s].merges {
                self.record(Delta::Merges(s, merges));
            }
        }
        // column widths
        if matches!(o.what, What::Widths | What::AllWidths) && !o.transpose {
            let mut tmp = Sheet::new(0, "");
            tmp.cols = self.sheets[s].cols.clone();
            for tj in 0..tw {
                for (j, w) in b.widths.iter().enumerate() {
                    let c = dst.c1 + tj * bw + j as u32;
                    if c < MAX_COLS {
                        let w = *w;
                        tmp.map_cols(c, c, |span| {
                            span.width = w;
                            span.custom = w.is_some();
                        });
                    }
                }
            }
            if tmp.cols != self.sheets[s].cols {
                self.record(Delta::Cols(s, tmp.cols));
            }
        }
        self.commit();
        Ok(out)
    }

    /// Cut and paste on one sheet: the cells move, and every reference to them follows.
    pub fn move_cells(&mut self, s: usize, src: Rect, r: u32, c: u32) -> Result<Rect, String> {
        let dst = Rect::new(r, c, r + src.rows() - 1, c + src.cols() - 1);
        if dst.r2 >= MAX_ROWS || dst.c2 >= MAX_COLS {
            return Err("The information cannot be pasted because the paste area is outside the sheet.".into());
        }
        if dst == src {
            return Ok(dst);
        }
        for a in self.sheets[s].arrays.values() {
            if (a.intersects(&src) && !src.contains_rect(a)) || (a.intersects(&dst) && !dst.contains_rect(a) && !src.contains_rect(a)) {
                return Err("You can't change part of an array.".into());
            }
        }
        let (dr, dc) = (r as i64 - src.r1 as i64, c as i64 - src.c1 as i64);
        let name = self.sheets[s].name.clone();
        self.begin("Paste", s);
        let how = Adjust::Move { sheet: &name, here: &name, src, dr, dc };
        // references everywhere (the moving cells' own included) follow the cells
        self.rewrite_formulas(&how, None);
        let mut names = self.names.clone();
        let mut changed = false;
        for n in names.iter_mut() {
            let here = n.scope.and_then(|i| self.sheets.get(i)).map(|x| x.name.clone()).unwrap_or_default();
            let t = formula::adjust(&n.text, &crate::book::rebase(&how, &here));
            if t != n.text {
                n.text = t;
                changed = true;
            }
        }
        if changed {
            self.record(Delta::Names(names));
        }
        let block = snapshot(&self.sheets[s], src);
        // the source empties (cells the destination covers are written below)
        for (rr, cc) in self.sheets[s].positions_in(src) {
            if !dst.contains(rr, cc) {
                let inherited = self.inherited_xf(s, rr, cc);
                self.record(Delta::Cell(s, rr, cc, if inherited != 0 { Some(Cell::default()) } else { None }));
            }
        }
        for i in 0..block.h {
            for j in 0..block.w {
                let (rr, cc) = (r + i, c + j);
                let mut cell = block.cells.get(&(i, j)).cloned().unwrap_or_default();
                cell.s = block.styles[i as usize][j as usize];
                if let Some(f) = &cell.f {
                    if let FKind::Array(rc) = f.kind {
                        let mut nf = (**f).clone();
                        nf.kind = FKind::Array(Rect::new((rc.r1 as i64 + dr) as u32, (rc.c1 as i64 + dc) as u32, (rc.r2 as i64 + dr) as u32, (rc.c2 as i64 + dc) as u32));
                        cell.f = Some(Rc::new(nf));
                    }
                }
                let blank = cell.is_blank() && cell.rich.is_none();
                let keep = !(blank && cell.s == self.inherited_xf(s, rr, cc));
                self.record(Delta::Cell(s, rr, cc, if keep { Some(cell) } else { None }));
            }
        }
        let mut merges: Vec<Rect> = self.sheets[s].merges.iter().filter(|m| !src.contains_rect(m) && !m.intersects(&dst)).copied().collect();
        for m in &block.merges {
            merges.push(Rect::new(r + m.r1, c + m.c1, r + m.r2, c + m.c2));
        }
        if merges != self.sheets[s].merges {
            self.record(Delta::Merges(s, merges));
        }
        self.commit();
        Ok(dst)
    }

    /// A copied block as a small workbook (for the clipboard and other windows).
    pub fn copy_to_bytes(&self, s: usize, rc: Rect) -> Result<Vec<u8>, String> {
        let rc = self.clip_rect(s, rc);
        let mut b = Book::new_empty();
        b.styles = Styles::parse(Some(&self.styles.to_xml()))?;
        b.date1904 = self.date1904;
        let sh = &self.sheets[s];
        let block = snapshot(sh, rc);
        let mut t = Sheet::new(1, &sh.name);
        for ((i, j), cell) in block.cells.iter() {
            let mut cell = cell.clone();
            if let Some(f) = &cell.f {
                // relative references stay relative to the block's corner
                let mut nf = (**f).clone();
                nf.text = formula::adjust(&f.text, &Adjust::Shift(-(rc.r1 as i64), -(rc.c1 as i64)));
                if let FKind::Array(_) = nf.kind {
                    nf.kind = FKind::Dynamic;
                }
                nf.expr = formula::parse(&nf.text).ok();
                cell.f = Some(Rc::new(nf));
            }
            t.put(*i, *j, Some(cell));
        }
        for (i, row) in block.styles.iter().enumerate() {
            for (j, xf) in row.iter().enumerate() {
                if *xf != 0 && !t.cells.contains_key(&(i as u32, j as u32)) {
                    t.put(i as u32, j as u32, Some(Cell { s: *xf, ..Default::default() }));
                }
            }
        }
        t.merges = block.merges.clone();
        for (j, w) in block.widths.iter().enumerate() {
            if let Some(w) = w {
                t.map_cols(j as u32, j as u32, |span| {
                    span.width = Some(*w);
                    span.custom = true;
                });
            }
        }
        let mut vals = Sheet::new(2, "__values");
        for ((i, j), cell) in block.cells.iter() {
            if cell.f.is_some() {
                vals.put(*i, *j, Some(Cell { v: cell.v.clone(), ..Default::default() }));
            }
        }
        b.sheets = vec![t, vals];
        // the size of what was copied (trailing blank cells included)
        b.names.push(crate::book::Name::new("__clip", None, &Rect::new(0, 0, rc.rows() - 1, rc.cols() - 1).a1()));
        b.to_bytes()
    }

    /// The copied area of a clipboard workbook.
    pub fn clip_area(&self) -> Rect {
        self.names.iter().find(|n| n.name == "__clip").and_then(|n| crate::cellref::parse_range(&n.text))
            .unwrap_or_else(|| {
                let (r, c) = self.sheets[0].used_extent().unwrap_or((0, 0));
                Rect::new(0, 0, r, c)
            })
    }
}
