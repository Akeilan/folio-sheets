//! Page setup, page breaks and printing: the settings the worksheet keeps (page margins,
//! page setup, print options, header and footer, manual breaks, print area and titles),
//! the pages a sheet prints on, and drawing one page.

use crate::book::{Book, Delta};
use crate::cellref::{col_name, parse_range, quote_sheet, Rect, MAX_COLS, MAX_ROWS};
use crate::sheet::Piece;
use crate::xml::{self, Element};

/// Paper sizes by their file code: (code, name, width, height in points).
pub const PAPERS: [(u32, &str, f64, f64); 12] = [
    (1, "Letter", 612.0, 792.0),
    (5, "Legal", 612.0, 1008.0),
    (7, "Executive", 522.0, 756.0),
    (3, "Tabloid", 792.0, 1224.0),
    (9, "A4", 595.28, 841.89),
    (8, "A3", 841.89, 1190.55),
    (11, "A5", 419.53, 595.28),
    (13, "B5 (JIS)", 515.91, 728.5),
    (20, "Envelope #10", 297.0, 684.0),
    (27, "Envelope DL", 311.81, 623.62),
    (28, "Envelope C5", 459.21, 649.13),
    (37, "Envelope Monarch", 279.0, 540.0),
];

#[derive(Clone, Debug, PartialEq)]
pub struct PageSetup {
    pub paper: u32,
    pub landscape: bool,
    /// percent
    pub scale: u32,
    /// fit to (width, height) pages; 0 = automatic
    pub fit: Option<(u32, u32)>,
    /// left, right, top, bottom, header, footer (inches)
    pub margins: [f64; 6],
    pub center_h: bool,
    pub center_v: bool,
    pub gridlines: bool,
    pub headings: bool,
    pub first_page: Option<u32>,
    pub over_then_down: bool,
    pub header: String,
    pub footer: String,
    /// breaks above these rows / left of these columns (0-based)
    pub row_breaks: Vec<u32>,
    pub col_breaks: Vec<u32>,
    pub print_area: Vec<Rect>,
    pub title_rows: Option<(u32, u32)>,
    pub title_cols: Option<(u32, u32)>,
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            paper: 1,
            landscape: false,
            scale: 100,
            fit: None,
            margins: [0.7, 0.7, 0.75, 0.75, 0.3, 0.3],
            center_h: false,
            center_v: false,
            gridlines: false,
            headings: false,
            first_page: None,
            over_then_down: false,
            header: String::new(),
            footer: String::new(),
            row_breaks: vec![],
            col_breaks: vec![],
            print_area: vec![],
            title_rows: None,
            title_cols: None,
        }
    }
}

impl PageSetup {
    /// Paper (width, height) in points, turned for landscape.
    pub fn paper_size(&self) -> (f64, f64) {
        let (w, h) = PAPERS.iter().find(|p| p.0 == self.paper).map(|p| (p.2, p.3)).unwrap_or((612.0, 792.0));
        if self.landscape { (h, w) } else { (w, h) }
    }
}

fn flag(e: &Element, name: &str) -> bool {
    matches!(e.attr(name), Some("1") | Some("true"))
}

/// One page: the cells it shows and its number.
#[derive(Clone, Debug)]
pub struct Page {
    pub rows: (u32, u32),
    pub cols: (u32, u32),
    pub number: u32,
}

impl Book {
    fn piece_el(&self, s: usize, tag: &str) -> Option<Element> {
        self.sheets[s].pieces.iter().find_map(|p| match p {
            Piece::Raw(t, x) if t == tag => xml::parse(x).ok().map(|x| x.1),
            _ => None,
        })
    }

    pub fn page_setup(&self, s: usize) -> PageSetup {
        let mut p = PageSetup::default();
        if let Some(m) = self.piece_el(s, "pageMargins") {
            for (k, n) in ["left", "right", "top", "bottom", "header", "footer"].iter().enumerate() {
                if let Some(v) = m.attr(n).and_then(|v| v.parse().ok()) {
                    p.margins[k] = v;
                }
            }
        }
        if let Some(ps) = self.piece_el(s, "pageSetup") {
            p.paper = ps.attr("paperSize").and_then(|v| v.parse().ok()).unwrap_or(1);
            p.landscape = ps.attr("orientation") == Some("landscape");
            p.scale = ps.attr("scale").and_then(|v| v.parse().ok()).unwrap_or(100);
            if flag(&ps, "useFirstPageNumber") {
                p.first_page = ps.attr("firstPageNumber").and_then(|v| v.parse().ok());
            }
            p.over_then_down = ps.attr("pageOrder") == Some("overThenDown");
            if self.sheets[s].fit_to_page {
                p.fit = Some((ps.attr("fitToWidth").and_then(|v| v.parse().ok()).unwrap_or(1), ps.attr("fitToHeight").and_then(|v| v.parse().ok()).unwrap_or(1)));
            }
        } else if self.sheets[s].fit_to_page {
            p.fit = Some((1, 1));
        }
        if let Some(po) = self.piece_el(s, "printOptions") {
            p.center_h = flag(&po, "horizontalCentered");
            p.center_v = flag(&po, "verticalCentered");
            p.gridlines = flag(&po, "gridLines");
            p.headings = flag(&po, "headings");
        }
        if let Some(hf) = self.piece_el(s, "headerFooter") {
            p.header = hf.child("oddHeader").map(|x| x.text()).unwrap_or_default();
            p.footer = hf.child("oddFooter").map(|x| x.text()).unwrap_or_default();
        }
        for (tag, out) in [("rowBreaks", &mut p.row_breaks), ("colBreaks", &mut p.col_breaks)] {
            if let Some(b) = self.piece_el(s, tag) {
                for brk in b.elems() {
                    if let Some(id) = brk.attr("id").and_then(|v| v.parse().ok()) {
                        out.push(id);
                    }
                }
            }
        }
        // the print area and titles are names of the sheet
        for n in &self.names {
            if n.scope != Some(s) {
                continue;
            }
            let lower = n.name.to_ascii_lowercase();
            if lower == "_xlnm.print_area" {
                p.print_area = n.text.split(',').filter_map(|a| {
                    let (_, _, rest) = crate::formula::split_sheet(a.trim());
                    parse_range(&rest.replace('$', ""))
                }).collect();
            } else if lower == "_xlnm.print_titles" {
                for a in n.text.split(',') {
                    let (_, _, rest) = crate::formula::split_sheet(a.trim());
                    if let Some(r) = parse_range(&rest.replace('$', "")) {
                        if r.c1 == 0 && r.c2 == MAX_COLS - 1 {
                            p.title_rows = Some((r.r1, r.r2));
                        } else if r.r1 == 0 && r.r2 == MAX_ROWS - 1 {
                            p.title_cols = Some((r.c1, r.c2));
                        }
                    }
                }
            }
        }
        p
    }

    /// Write page settings back to the sheet (one undo step).
    pub fn set_page_setup(&mut self, s: usize, p: &PageSetup, label: &str) -> Result<(), String> {
        let sh = &self.sheets[s];
        let pfx = {
            let n = sh.root_open.trim_start_matches('<');
            let n = &n[..n.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(n.len())];
            n.find(':').map(|k| n[..=k].to_string()).unwrap_or_default()
        };
        let num = |x: f64| crate::xlsx::num_text((x * 10000.0).round() / 10000.0);
        let mut pieces: Vec<Piece> = sh.pieces.iter().filter(|x| !matches!(x, Piece::Raw(t, _) if matches!(t.as_str(), "pageMargins" | "printOptions" | "headerFooter" | "rowBreaks" | "colBreaks"))).cloned().collect();
        // page setup keeps what it had (printer settings) beside what we set
        let old_ps = self.piece_el(s, "pageSetup");
        pieces.retain(|x| !matches!(x, Piece::Raw(t, _) if t == "pageSetup"));
        let mut po = format!("<{}printOptions", pfx);
        if p.center_h { po.push_str(" horizontalCentered=\"1\"") }
        if p.center_v { po.push_str(" verticalCentered=\"1\"") }
        if p.headings { po.push_str(" headings=\"1\"") }
        if p.gridlines { po.push_str(" gridLines=\"1\"") }
        po.push_str("/>");
        if po != format!("<{}printOptions/>", pfx) {
            pieces.push(Piece::Raw("printOptions".into(), po));
        }
        let m = &p.margins;
        pieces.push(Piece::Raw("pageMargins".into(), format!("<{}pageMargins left=\"{}\" right=\"{}\" top=\"{}\" bottom=\"{}\" header=\"{}\" footer=\"{}\"/>",
            pfx, num(m[0]), num(m[1]), num(m[2]), num(m[3]), num(m[4]), num(m[5]))));
        let mut ps = old_ps.unwrap_or_else(|| Element::new(&format!("{}pageSetup", pfx)));
        ps.set_attr("paperSize", &p.paper.to_string());
        ps.set_attr("orientation", if p.landscape { "landscape" } else { "portrait" });
        if p.scale != 100 { ps.set_attr("scale", &p.scale.to_string()) } else { ps.remove_attr("scale") }
        match p.fit {
            Some((w, h)) => {
                ps.set_attr("fitToWidth", &w.to_string());
                ps.set_attr("fitToHeight", &h.to_string());
            }
            None => {
                ps.remove_attr("fitToWidth");
                ps.remove_attr("fitToHeight");
            }
        }
        match p.first_page {
            Some(n) => {
                ps.set_attr("firstPageNumber", &n.to_string());
                ps.set_attr("useFirstPageNumber", "1");
            }
            None => {
                ps.remove_attr("firstPageNumber");
                ps.remove_attr("useFirstPageNumber");
            }
        }
        if p.over_then_down { ps.set_attr("pageOrder", "overThenDown") } else { ps.remove_attr("pageOrder") }
        pieces.push(Piece::Raw("pageSetup".into(), ps.to_string()));
        if !p.header.is_empty() || !p.footer.is_empty() {
            let mut hf = format!("<{}headerFooter>", pfx);
            if !p.header.is_empty() {
                hf.push_str(&format!("<{p}oddHeader>{}</{p}oddHeader>", xml::escape_text(&p.header), p = pfx));
            }
            if !p.footer.is_empty() {
                hf.push_str(&format!("<{p}oddFooter>{}</{p}oddFooter>", xml::escape_text(&p.footer), p = pfx));
            }
            hf.push_str(&format!("</{}headerFooter>", pfx));
            pieces.push(Piece::Raw("headerFooter".into(), hf));
        }
        let brks = |tag: &str, list: &[u32], max: u32| -> Option<String> {
            if list.is_empty() {
                return None;
            }
            let mut v: Vec<u32> = list.to_vec();
            v.sort_unstable();
            v.dedup();
            let mut x = format!("<{}{} count=\"{}\" manualBreakCount=\"{}\">", pfx, tag, v.len(), v.len());
            for id in v {
                x.push_str(&format!("<{}brk id=\"{}\" max=\"{}\" man=\"1\"/>", pfx, id, max));
            }
            x.push_str(&format!("</{}{}>", pfx, tag));
            Some(x)
        };
        if let Some(x) = brks("rowBreaks", &p.row_breaks, MAX_COLS - 1) {
            pieces.push(Piece::Raw("rowBreaks".into(), x));
        }
        if let Some(x) = brks("colBreaks", &p.col_breaks, MAX_ROWS - 1) {
            pieces.push(Piece::Raw("colBreaks".into(), x));
        }
        // names
        let q = quote_sheet(&sh.name);
        let abs = |r: &Rect| format!("{}!${}${}:${}${}", q, col_name(r.c1), r.r1 + 1, col_name(r.c2), r.r2 + 1);
        let mut names = self.names.clone();
        names.retain(|n| !(n.scope == Some(s) && matches!(n.name.to_ascii_lowercase().as_str(), "_xlnm.print_area" | "_xlnm.print_titles")));
        if !p.print_area.is_empty() {
            let t = p.print_area.iter().map(abs).collect::<Vec<_>>().join(",");
            names.push(crate::book::Name::new("_xlnm.Print_Area", Some(s), &t));
        }
        let mut titles = Vec::new();
        if let Some((c1, c2)) = p.title_cols {
            titles.push(format!("{}!${}:${}", q, col_name(c1), col_name(c2)));
        }
        if let Some((r1, r2)) = p.title_rows {
            titles.push(format!("{}!${}:${}", q, r1 + 1, r2 + 1));
        }
        if !titles.is_empty() {
            names.push(crate::book::Name::new("_xlnm.Print_Titles", Some(s), &titles.join(",")));
        }
        let fit = p.fit.is_some();
        self.begin(label, s);
        self.record(Delta::Pieces(s, pieces));
        let key = |v: &[crate::book::Name]| v.iter().map(|n| (n.name.clone(), n.scope, n.text.clone())).collect::<Vec<_>>();
        if key(&names) != key(&self.names) {
            self.record(Delta::Names(names));
        }
        if fit != self.sheets[s].fit_to_page {
            self.record(Delta::Fit(s, fit));
        }
        self.commit();
        Ok(())
    }

    /// What prints when there is no print area: every cell with something in it or a
    /// look of its own, and the objects.
    pub fn print_extent(&self, s: usize) -> Option<Rect> {
        let sh = &self.sheets[s];
        let mut out: Option<Rect> = None;
        let mut add = |r: u32, c: u32| {
            out = Some(match out {
                None => Rect::cell(r, c),
                Some(o) => Rect::new(o.r1.min(r), o.c1.min(c), o.r2.max(r), o.c2.max(c)),
            });
        };
        for ((r, c), cell) in sh.cells.iter() {
            if !cell.is_blank() || (cell.s != 0 && (self.fill_color(self.styles.xf(cell.s)).is_some() || self.styles.xf(cell.s).border != 0)) {
                add(*r, *c);
            }
        }
        for (r, c) in sh.ghosts.keys() {
            add(*r, *c);
        }
        for o in &sh.objects {
            let (x, y, w, h) = self.obj_rect(s, o);
            add(self.row_at(s, y), self.col_at(s, x));
            add(self.row_at(s, y + h), self.col_at(s, x + w));
        }
        out
    }

    /// The scale a sheet prints at (fit-to-pages worked out).
    pub fn print_scale(&self, s: usize, p: &PageSetup) -> f64 {
        let (fw, fh) = match p.fit {
            None => return (p.scale.clamp(10, 400)) as f64 / 100.0,
            Some(x) => x,
        };
        let (pw, ph) = self.printable(p);
        let areas = self.print_areas(s, p);
        let mut sc: f64 = 1.0;
        for a in &areas {
            let (tw, th) = self.title_size(s, p);
            let w: f64 = (a.c1..=a.c2).map(|c| self.col_px(s, c)).sum::<f64>() * 0.75;
            let h: f64 = (a.r1..=a.r2).map(|r| self.row_px(s, r)).sum::<f64>() * 0.75;
            if fw > 0 && w > 0.0 {
                sc = sc.min(fw as f64 * (pw - tw * 0.75) / w);
            }
            if fh > 0 && h > 0.0 {
                sc = sc.min(fh as f64 * (ph - th * 0.75) / h);
            }
        }
        // a little room for rounding, as the original leaves
        (sc * 0.999).clamp(0.1, 4.0)
    }

    /// Printable size on the page in points.
    pub fn printable(&self, p: &PageSetup) -> (f64, f64) {
        let (w, h) = p.paper_size();
        let m = &p.margins;
        ((w - (m[0] + m[1]) * 72.0).max(36.0), (h - (m[2] + m[3]) * 72.0).max(36.0))
    }

    fn print_areas(&self, s: usize, p: &PageSetup) -> Vec<Rect> {
        if !p.print_area.is_empty() {
            return p.print_area.iter().map(|r| self.sheets[s].clamp_rect(*r)).collect();
        }
        self.print_extent(s).into_iter().collect()
    }

    /// Height of the title rows and width of the title columns, in pixels at 100 %.
    fn title_size(&self, s: usize, p: &PageSetup) -> (f64, f64) {
        let tw = p.title_cols.map(|(a, b)| (a..=b).map(|c| self.col_px(s, c)).sum()).unwrap_or(0.0);
        let th = p.title_rows.map(|(a, b)| (a..=b).map(|r| self.row_px(s, r)).sum()).unwrap_or(0.0);
        (tw, th)
    }

    /// The pages a sheet prints on, in order.
    pub fn pages(&self, s: usize) -> Vec<Page> {
        let p = self.page_setup(s);
        let sc = self.print_scale(s, &p);
        let (pw, ph) = self.printable(&p);
        // content space in pixels at 100 %
        let (tw, th) = self.title_size(s, &p);
        let head = if p.headings { 20.0 } else { 0.0 };
        let avail_w = pw / 0.75 / sc - tw - if p.headings { 40.0 } else { 0.0 };
        let avail_h = ph / 0.75 / sc - th - head;
        let mut out = Vec::new();
        let mut n = p.first_page.unwrap_or(1);
        for a in self.print_areas(s, &p) {
            let split = |lo: u32, hi: u32, size: &dyn Fn(u32) -> f64, avail: f64, brks: &[u32]| -> Vec<(u32, u32)> {
                let mut v = Vec::new();
                let mut start = lo;
                let mut acc = 0.0;
                let mut k = lo;
                while k <= hi {
                    let w = size(k);
                    let manual = k > start && brks.contains(&k);
                    if manual || (acc + w > avail && k > start) {
                        v.push((start, k - 1));
                        start = k;
                        acc = 0.0;
                        continue;
                    }
                    acc += w;
                    k += 1;
                }
                v.push((start, hi));
                v
            };
            let rows = split(a.r1, a.r2, &|r| self.row_px(s, r), avail_h.max(10.0), &p.row_breaks);
            let cols = split(a.c1, a.c2, &|c| self.col_px(s, c), avail_w.max(10.0), &p.col_breaks);
            if p.over_then_down {
                for r in &rows {
                    for c in &cols {
                        out.push(Page { rows: *r, cols: *c, number: n });
                        n += 1;
                    }
                }
            } else {
                for c in &cols {
                    for r in &rows {
                        out.push(Page { rows: *r, cols: *c, number: n });
                        n += 1;
                    }
                }
            }
        }
        // pages of hidden lines only don't print
        out.retain(|pg| (pg.rows.0..=pg.rows.1).any(|r| self.row_px(s, r) > 0.0) && (pg.cols.0..=pg.cols.1).any(|c| self.col_px(s, c) > 0.0));
        let first = p.first_page.unwrap_or(1);
        for (k, pg) in out.iter_mut().enumerate() {
            pg.number = first + k as u32;
        }
        out
    }

    /// Draw page k of sheet s on a cairo context whose units are points (the page's
    /// top-left at 0, 0).
    pub unsafe fn render_page(&self, s: usize, k: usize, cr: *mut crate::ffi::cairo_t, file_name: &str) -> Result<(), String> {
        use crate::ffi::*;
        let pages = self.pages(s);
        let pg = pages.get(k).ok_or("no such page")?.clone();
        let p = self.page_setup(s);
        let sc = self.print_scale(s, &p);
        let (paper_w, paper_h) = p.paper_size();
        let m = p.margins;
        let (pw, ph) = self.printable(&p);
        let (tw, th) = self.title_size(s, &p);
        let body_w: f64 = (pg.cols.0..=pg.cols.1).map(|c| self.col_px(s, c)).sum();
        let body_h: f64 = (pg.rows.0..=pg.rows.1).map(|r| self.row_px(s, r)).sum();
        let hw = if p.headings { self.print_header_width(s, pg.rows.1) } else { 0.0 };
        let hh = if p.headings { 20.0 } else { 0.0 };
        // titles count only when the page doesn't already show them
        let trows = p.title_rows.filter(|(a, _)| pg.rows.0 > *a);
        let tcols = p.title_cols.filter(|(a, _)| pg.cols.0 > *a);
        let th = if trows.is_some() { th } else { 0.0 };
        let tw = if tcols.is_some() { tw } else { 0.0 };
        let content_w = (hw + tw + body_w) * 0.75 * sc;
        let content_h = (hh + th + body_h) * 0.75 * sc;
        let mut ox = m[0] * 72.0;
        let mut oy = m[2] * 72.0;
        if p.center_h {
            ox += ((pw - content_w) / 2.0).max(0.0);
        }
        if p.center_v {
            oy += ((ph - content_h) / 2.0).max(0.0);
        }
        cairo_save(cr);
        cairo_set_source_rgb(cr, 1.0, 1.0, 1.0);
        cairo_rectangle(cr, 0.0, 0.0, paper_w, paper_h);
        cairo_fill(cr);
        // one block of cells drawn by the grid renderer
        let block = |x: f64, y: f64, r0: u32, r1: u32, c0: u32, c1: u32, headers: bool| {
            let w: f64 = (c0..=c1).map(|c| self.col_px(s, c)).sum::<f64>() + if headers { hw } else { 0.0 };
            let h: f64 = (r0..=r1).map(|r| self.row_px(s, r)).sum::<f64>() + if headers { hh } else { 0.0 };
            let mut vp = crate::view::ViewParams::default();
            vp.width = w;
            vp.height = h;
            vp.zoom = 1.0;
            vp.top_row = r0;
            vp.left_col = c0;
            vp.show_grid = p.gridlines;
            vp.show_headers = headers;
            vp.sel = vec![];
            vp.active = (MAX_ROWS - 1, MAX_COLS - 1);
            vp.print = true;
            cairo_save(cr);
            cairo_translate(cr, x, y);
            cairo_scale(cr, 0.75 * sc, 0.75 * sc);
            cairo_rectangle(cr, 0.0, 0.0, w, h);
            cairo_clip(cr);
            self.render(s, cr, &vp);
            cairo_restore(cr);
        };
        let (bx, by) = (ox + (tw * 0.75 * sc), oy + (th * 0.75 * sc));
        if let (Some((a, b)), Some((c, d))) = (trows, tcols) {
            block(ox, oy, a, b, c, d, false);
        }
        if let Some((a, b)) = trows {
            block(bx, oy, a, b, pg.cols.0, pg.cols.1, false);
        }
        if let Some((c, d)) = tcols {
            block(ox, by, pg.rows.0, pg.rows.1, c, d, false);
        }
        block(if p.headings { bx } else { bx }, if p.headings { by } else { by }, pg.rows.0, pg.rows.1, pg.cols.0, pg.cols.1, p.headings);
        // header and footer
        let total = pages.len() as u32 + p.first_page.unwrap_or(1) - 1;
        let sheet = self.sheets[s].name.clone();
        self.paint_header_footer(cr, &p.header, m[4] * 72.0, false, paper_w, m, pg.number, total, &sheet, file_name);
        self.paint_header_footer(cr, &p.footer, paper_h - m[5] * 72.0, true, paper_w, m, pg.number, total, &sheet, file_name);
        cairo_restore(cr);
        Ok(())
    }

    fn print_header_width(&self, _s: usize, last_row: u32) -> f64 {
        let digits = (last_row + 1).to_string().len().max(3) as f64;
        digits * 7.0 + 6.0
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) unsafe fn paint_header_footer(&self, cr: *mut crate::ffi::cairo_t, code: &str, y: f64, bottom: bool, paper_w: f64, m: [f64; 6],
                                  page: u32, pages: u32, sheet: &str, file: &str) {
        use crate::ffi::*;
        if code.is_empty() {
            return;
        }
        let parts = header_sections(code, page, pages, sheet, file);
        let layout = pango_cairo_create_layout(cr);
        for (k, sec) in parts.iter().enumerate() {
            if sec.0.is_empty() {
                continue;
            }
            let (text, bold, italic, size) = sec;
            let f = crate::layout::FontSpec { family: self.styles.theme_fonts.1.clone(), size: *size, bold: *bold, italic: *italic, underline: 0, strike: false, vert: 0, color: 0 };
            let d = crate::layout::FontDesc::new(&f, 0.75);
            pango_layout_set_font_description(layout, d.0);
            set_text(layout, text);
            let (mut w, mut h) = (0, 0);
            pango_layout_get_pixel_size(layout, &mut w, &mut h);
            let x = match k {
                0 => m[0] * 72.0,
                1 => (paper_w - w as f64) / 2.0,
                _ => paper_w - m[1] * 72.0 - w as f64,
            };
            let yy = if bottom { y - h as f64 } else { y };
            cairo_set_source_rgb(cr, 0.0, 0.0, 0.0);
            cairo_move_to(cr, x, yy);
            pango_cairo_show_layout(cr, layout);
        }
        g_object_unref(layout as *mut _);
    }
}

/// The left, centre and right text of a header or footer code (&L &C &R, &P &N &D &T
/// &A &F &B &I &"font" &nn), with whether bold/italic and the size.
pub fn header_sections(code: &str, page: u32, pages: u32, sheet: &str, file: &str) -> [(String, bool, bool, f64); 3] {
    let mut out: [(String, bool, bool, f64); 3] = [(String::new(), false, false, 11.0), (String::new(), false, false, 11.0), (String::new(), false, false, 11.0)];
    let mut cur = 1usize;
    let cs: Vec<char> = code.chars().collect();
    let mut i = 0;
    let now = crate::recalc::now_serial(false);
    while i < cs.len() {
        let c = cs[i];
        if c == '&' && i + 1 < cs.len() {
            let n = cs[i + 1];
            i += 2;
            match n {
                'L' => cur = 0,
                'C' => cur = 1,
                'R' => cur = 2,
                'P' => out[cur].0.push_str(&page.to_string()),
                'N' => out[cur].0.push_str(&pages.to_string()),
                'D' => out[cur].0.push_str(&crate::numfmt::format_number(now.floor(), "m/d/yyyy", false)),
                'T' => out[cur].0.push_str(&crate::numfmt::format_number(now, "h:mm AM/PM", false)),
                'A' => out[cur].0.push_str(sheet),
                'F' => out[cur].0.push_str(file),
                'Z' => {}
                'B' => out[cur].1 = !out[cur].1,
                'I' => out[cur].2 = !out[cur].2,
                'U' | 'E' | 'S' | 'X' | 'Y' | 'O' | 'H' | 'G' | 'K' => {}
                '&' => out[cur].0.push('&'),
                '"' => {
                    // &"Font,Style": the style can make it bold or italic
                    let start = i;
                    while i < cs.len() && cs[i] != '"' {
                        i += 1;
                    }
                    let spec: String = cs[start..i].iter().collect();
                    i += 1;
                    let style = spec.split(',').nth(1).unwrap_or("").to_lowercase();
                    out[cur].1 = style.contains("bold");
                    out[cur].2 = style.contains("italic");
                }
                d if d.is_ascii_digit() => {
                    let mut j = i - 1;
                    while j < cs.len() && cs[j].is_ascii_digit() {
                        j += 1;
                    }
                    let t: String = cs[i - 1..j].iter().collect();
                    out[cur].3 = t.parse().unwrap_or(11.0);
                    i = j;
                }
                _ => {}
            }
            continue;
        }
        out[cur].0.push(c);
        i += 1;
    }
    out
}

/// Margin presets: Normal, Wide, Narrow.
pub fn margin_preset(name: &str) -> Option<[f64; 6]> {
    match name {
        "normal" => Some([0.7, 0.7, 0.75, 0.75, 0.3, 0.3]),
        "wide" => Some([1.0, 1.0, 1.0, 1.0, 0.5, 0.5]),
        "narrow" => Some([0.25, 0.25, 0.75, 0.75, 0.3, 0.3]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headers() {
        let h = header_sections("&LBudget&C&BPage &P of &N&R&A", 2, 5, "Sales", "f.xlsx");
        assert_eq!(h[0].0, "Budget");
        assert_eq!(h[1].0, "Page 2 of 5");
        assert!(h[1].1);
        assert_eq!(h[2].0, "Sales");
    }
}
