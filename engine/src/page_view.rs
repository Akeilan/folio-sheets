//! View > Page Layout: the sheet shown as the pages it prints on, with margins, header and
//! footer areas and (View > Ruler) rulers. Cells are edited on the pages as in Normal view:
//! each page's cells are drawn, hit and measured by the normal grid code, moved into place.
use crate::book::Book;
use crate::cellref::{MAX_COLS, MAX_ROWS};
use crate::ffi::*;
use crate::view::{Geometry, HitPart, ViewParams};

pub const RULER: f64 = 18.0;

#[derive(Clone, Debug)]
pub struct PlPage {
    pub band_r: usize,
    pub band_c: usize,
    pub rows: (u32, u32),
    pub cols: (u32, u32),
    /// the paper on screen
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// where the page's first cell is drawn
    pub cx: f64,
    pub cy: f64,
}

struct Metrics {
    page_w: f64,
    page_h: f64,
    /// margins on screen: left, right, top, bottom, header, footer
    m: [f64; 6],
    /// cell zoom (screen zoom times print scale)
    cz: f64,
    gap: f64,
    off_x: f64,
    off_y: f64,
    avail_w: f64,
    avail_h: f64,
}

impl Book {
    fn pl_metrics(&self, s: usize, p: &ViewParams) -> Metrics {
        let ps = self.page_setup(s);
        let sc = self.print_scale(s, &ps);
        let (pw, ph) = ps.paper_size();
        let z = p.zoom;
        let k = z / 0.75;
        let mm = ps.margins;
        let (aw, ah) = self.printable(&ps);
        Metrics {
            page_w: pw * k,
            page_h: ph * k,
            m: [mm[0] * 72.0 * k, mm[1] * 72.0 * k, mm[2] * 72.0 * k, mm[3] * 72.0 * k, mm[4] * 72.0 * k, mm[5] * 72.0 * k],
            cz: z * sc,
            gap: (14.0 * z).max(6.0),
            off_x: if p.ruler { RULER } else { 0.0 },
            off_y: if p.ruler { RULER } else { 0.0 },
            // cell space per page, in pixels at 100 %
            avail_w: (aw / 0.75 / sc).max(10.0),
            avail_h: (ah / 0.75 / sc).max(10.0),
        }
    }

    /// Row (vertical) or column bands, one per page, from the first line up to the band
    /// holding `upto` and then `more` bands after it.
    fn pl_bands(&self, s: usize, rows: bool, avail: f64, upto: u32, more: usize) -> Vec<(u32, u32)> {
        let ps = self.page_setup(s);
        let brks = if rows { ps.row_breaks } else { ps.col_breaks };
        let max = if rows { MAX_ROWS } else { MAX_COLS };
        let size = |k: u32| if rows { self.row_px(s, k) } else { self.col_px(s, k) };
        let mut out = Vec::new();
        let mut start = 0u32;
        let mut acc = 0.0;
        let mut k = 0u32;
        let mut after = 0usize;
        while k < max {
            let w = size(k);
            if k > start && (brks.contains(&k) || acc + w > avail) {
                out.push((start, k - 1));
                if k > upto {
                    after += 1;
                    if after > more {
                        return out;
                    }
                }
                start = k;
                acc = 0.0;
                continue;
            }
            acc += w;
            k += 1;
        }
        out.push((start, max - 1));
        out
    }

    fn band_of(bands: &[(u32, u32)], k: u32) -> usize {
        bands.iter().position(|b| k >= b.0 && k <= b.1).unwrap_or(bands.len().saturating_sub(1))
    }

    /// The scroll position: the top-left line of the window as a distance into the pages.
    fn pl_origin(&self, s: usize, p: &ViewParams, mt: &Metrics, rb: &[(u32, u32)], cb: &[(u32, u32)]) -> (f64, f64) {
        let bi = Self::band_of(rb, p.top_row);
        let bj = Self::band_of(cb, p.left_col);
        let y = bi as f64 * (mt.page_h + mt.gap) + if p.top_row == rb[bi].0 { 0.0 } else {
            mt.m[2] + (self.row_y(s, p.top_row) - self.row_y(s, rb[bi].0)) * mt.cz
        };
        let x = bj as f64 * (mt.page_w + mt.gap) + if p.left_col == cb[bj].0 { 0.0 } else {
            mt.m[0] + (self.col_x(s, p.left_col) - self.col_x(s, cb[bj].0)) * mt.cz
        };
        (x, y)
    }

    fn pl_page(&self, mt: &Metrics, org: (f64, f64), rb: &[(u32, u32)], cb: &[(u32, u32)], i: usize, j: usize) -> PlPage {
        let x = mt.off_x + mt.gap + j as f64 * (mt.page_w + mt.gap) - org.0;
        let y = mt.off_y + mt.gap + i as f64 * (mt.page_h + mt.gap) - org.1;
        PlPage { band_r: i, band_c: j, rows: rb[i], cols: cb[j], x, y, w: mt.page_w, h: mt.page_h, cx: x + mt.m[0], cy: y + mt.m[2] }
    }

    /// The pages on screen.
    pub fn pl_pages(&self, s: usize, p: &ViewParams) -> Vec<PlPage> {
        let mt = self.pl_metrics(s, p);
        let more_r = (p.height / (mt.page_h + mt.gap)).ceil() as usize + 2;
        let more_c = (p.width / (mt.page_w + mt.gap)).ceil() as usize + 2;
        let rb = self.pl_bands(s, true, mt.avail_h, p.top_row, more_r);
        let cb = self.pl_bands(s, false, mt.avail_w, p.left_col, more_c);
        let org = self.pl_origin(s, p, &mt, &rb, &cb);
        let mut out = Vec::new();
        for i in Self::band_of(&rb, p.top_row)..rb.len() {
            for j in Self::band_of(&cb, p.left_col)..cb.len() {
                let pg = self.pl_page(&mt, org, &rb, &cb, i, j);
                if pg.y > p.height || pg.x > p.width {
                    continue;
                }
                if pg.y + pg.h < 0.0 || pg.x + pg.w < 0.0 {
                    continue;
                }
                out.push(pg);
            }
        }
        out
    }

    /// The view parameters that draw (and hit) one page's cells at its origin.
    pub fn pl_block(&self, s: usize, p: &ViewParams, pg: &PlPage) -> ViewParams {
        let mt = self.pl_metrics(s, p);
        let mut vp = p.clone();
        vp.page_layout = false;
        vp.ruler = false;
        vp.zoom = mt.cz;
        vp.top_row = pg.rows.0;
        vp.left_col = pg.cols.0;
        vp.freeze_rows = 0;
        vp.freeze_cols = 0;
        vp.pane_top = 0;
        vp.pane_left = 0;
        vp.split = false;
        vp.show_headers = false;
        vp.page_breaks = false;
        vp.width = (self.col_x(s, pg.cols.1 + 1) - self.col_x(s, pg.cols.0)) * mt.cz;
        vp.height = (self.row_y(s, pg.rows.1 + 1) - self.row_y(s, pg.rows.0)) * mt.cz;
        if let Some(r) = vp.obj_preview {
            vp.obj_preview = Some((r.0 - pg.cx, r.1 - pg.cy, r.2, r.3));
        }
        vp
    }

    /// The page holding a cell (its bands worked out as far as needed).
    pub fn pl_page_of(&self, s: usize, p: &ViewParams, r: u32, c: u32) -> PlPage {
        let mt = self.pl_metrics(s, p);
        let rb = self.pl_bands(s, true, mt.avail_h, p.top_row.max(r), 1);
        let cb = self.pl_bands(s, false, mt.avail_w, p.left_col.max(c), 1);
        let org = self.pl_origin(s, p, &mt, &rb, &cb);
        self.pl_page(&mt, org, &rb, &cb, Self::band_of(&rb, r), Self::band_of(&cb, c))
    }

    pub fn pl_cell_rect(&self, s: usize, r: u32, c: u32, p: &ViewParams) -> Option<(f64, f64, f64, f64)> {
        let pg = self.pl_page_of(s, p, r, c);
        let vp = self.pl_block(s, p, &pg);
        let (x, y, w, h) = self.cell_rect(s, r, c, &vp)?;
        Some((x + pg.cx, y + pg.cy, w, h))
    }

    pub fn pl_hit(&self, s: usize, x: f64, y: f64, p: &ViewParams) -> Vec<HitPart> {
        if p.ruler && (x < RULER || y < RULER) {
            return vec![HitPart::S("ruler")];
        }
        let mt = self.pl_metrics(s, p);
        for pg in self.pl_pages(s, p) {
            if x < pg.x || x > pg.x + pg.w || y < pg.y || y > pg.y + pg.h {
                continue;
            }
            let vp = self.pl_block(s, p, &pg);
            if x >= pg.cx && x < pg.cx + vp.width && y >= pg.cy && y < pg.cy + vp.height {
                let h = self.hit(s, x - pg.cx, y - pg.cy, &vp);
                if !matches!(h.first(), Some(HitPart::S("out"))) {
                    return h;
                }
            }
            let third = ((x - pg.x) / (pg.w / 3.0)).floor().clamp(0.0, 2.0) as u32;
            if y < pg.cy {
                let _ = &mt;
                return vec![HitPart::S("header"), HitPart::N(third), HitPart::N(pg.band_r as u32), HitPart::N(pg.band_c as u32)];
            }
            if y > pg.y + pg.h - mt.m[3] {
                return vec![HitPart::S("footer"), HitPart::N(third), HitPart::N(pg.band_r as u32), HitPart::N(pg.band_c as u32)];
            }
            // the margin beside the cells: the nearest cell of the page
            let cx = (x - pg.cx).clamp(0.0, (vp.width - 1.0).max(0.0));
            let cy = (y - pg.cy).clamp(0.0, (vp.height - 1.0).max(0.0));
            return self.hit(s, cx, cy, &vp);
        }
        vec![HitPart::S("out")]
    }

    /// Rows and columns on screen (for scrolling and paging), from the first page column and row.
    pub fn pl_geometry(&self, s: usize, p: &ViewParams) -> Geometry {
        let mut g = Geometry { hw: if p.ruler { RULER } else { 0.0 }, hh: if p.ruler { RULER } else { 0.0 }, ..Default::default() };
        let pages = self.pl_pages(s, p);
        let first_c = pages.iter().map(|x| x.band_c).min().unwrap_or(0);
        let first_r = pages.iter().map(|x| x.band_r).min().unwrap_or(0);
        let mt = self.pl_metrics(s, p);
        for pg in pages.iter().filter(|x| x.band_c == first_c) {
            let mut y = pg.cy;
            for r in pg.rows.0..=pg.rows.1 {
                let h = self.row_px(s, r) * mt.cz;
                if h > 0.0 && y + h > 0.0 && y < p.height {
                    g.rows.push((r, y, h));
                }
                y += h;
            }
        }
        for pg in pages.iter().filter(|x| x.band_r == first_r) {
            let mut x = pg.cx;
            for c in pg.cols.0..=pg.cols.1 {
                let w = self.col_px(s, c) * mt.cz;
                if w > 0.0 && x + w > 0.0 && x < p.width {
                    g.cols.push((c, x, w));
                }
                x += w;
            }
        }
        g.row_spans = vec![(g.hh, p.height)];
        g.col_spans = vec![(g.hw, p.width)];
        g
    }

    pub fn pl_object_screen_rect(&self, s: usize, i: usize, p: &ViewParams) -> Option<(f64, f64, f64, f64)> {
        let o = self.sheets.get(s)?.objects.get(i)?;
        let (x, y, _, _) = self.obj_rect(s, o);
        let pg = self.pl_page_of(s, p, self.row_at(s, y), self.col_at(s, x));
        let vp = self.pl_block(s, p, &pg);
        let g = self.geometry(s, &vp);
        let (ox, oy, w, h) = self.obj_screen_rect(s, &vp, &g, o);
        Some((ox + pg.cx, oy + pg.cy, w, h))
    }

    pub fn pl_object_hit(&self, s: usize, x: f64, y: f64, p: &ViewParams) -> Option<(usize, &'static str)> {
        let sh = self.sheets.get(s)?;
        for &i in p.sel_objects.iter().rev() {
            if let Some((ox, oy, w, h)) = self.pl_object_screen_rect(s, i, p) {
                for (k, (hx, hy)) in crate::render::handles(ox, oy, w, h).iter().enumerate() {
                    if (x - hx).abs() <= 6.0 && (y - hy).abs() <= 6.0 {
                        return Some((i, crate::render::HANDLE_NAMES[k]));
                    }
                }
            }
        }
        for i in (0..sh.objects.len()).rev() {
            if crate::objects_ext::is_hidden(&sh.objects[i]) {
                continue;
            }
            if let Some((ox, oy, w, h)) = self.pl_object_screen_rect(s, i, p) {
                if x >= ox && x <= ox + w && y >= oy && y <= oy + h {
                    return Some((i, "body"));
                }
            }
        }
        None
    }

    pub unsafe fn pl_render(&self, s: usize, cr: *mut cairo_t, p: &ViewParams, file_name: &str) {
        let mt = self.pl_metrics(s, p);
        let ps = self.page_setup(s);
        let z = p.zoom;
        cairo_save(cr);
        let bg = if p.dark { 0x3A3A3A } else { 0xE6E6E6 };
        let rgb = |c: u32| cairo_set_source_rgb(cr, ((c >> 16) & 255) as f64 / 255.0, ((c >> 8) & 255) as f64 / 255.0, (c & 255) as f64 / 255.0);
        rgb(bg);
        cairo_rectangle(cr, 0.0, 0.0, p.width, p.height);
        cairo_fill(cr);
        let pages = self.pl_pages(s, p);
        let print_pages = self.pages(s);
        let total = print_pages.len().max(1) as u32;
        let sheet = self.sheets[s].name.clone();
        for pg in &pages {
            // the paper and its shadow
            cairo_set_source_rgba(cr, 0.0, 0.0, 0.0, 0.18);
            cairo_rectangle(cr, pg.x + 2.0, pg.y + 2.0, pg.w, pg.h);
            cairo_fill(cr);
            rgb(0xFFFFFF);
            cairo_rectangle(cr, pg.x, pg.y, pg.w, pg.h);
            cairo_fill(cr);
            let vp = self.pl_block(s, p, pg);
            cairo_save(cr);
            cairo_rectangle(cr, pg.cx, pg.cy, vp.width.min(pg.w - mt.m[0] - mt.m[1] + 1.0), vp.height.min(pg.h - mt.m[2] - mt.m[3] + 1.0));
            cairo_clip(cr);
            cairo_translate(cr, pg.cx, pg.cy);
            self.render(s, cr, &vp);
            cairo_restore(cr);
            // header and footer, as printed (in points on the page)
            let number = print_pages.iter().find(|x| x.rows.0 == pg.rows.0 && x.cols.0 == pg.cols.0).map(|x| x.number)
                .unwrap_or(ps.first_page.unwrap_or(1) + pg.band_r as u32 + pg.band_c as u32);
            cairo_save(cr);
            cairo_translate(cr, pg.x, pg.y);
            cairo_scale(cr, z / 0.75, z / 0.75);
            let (pw, ph) = ps.paper_size();
            if ps.header.is_empty() {
                self.pl_faint(cr, "Click to add header", pw / 2.0, ps.margins[4] * 72.0, false);
            } else {
                self.paint_header_footer(cr, &ps.header, ps.margins[4] * 72.0, false, pw, ps.margins, number, total, &sheet, file_name);
            }
            if ps.footer.is_empty() {
                self.pl_faint(cr, "Click to add footer", pw / 2.0, ph - ps.margins[5] * 72.0, true);
            } else {
                self.paint_header_footer(cr, &ps.footer, ph - ps.margins[5] * 72.0, true, pw, ps.margins, number, total, &sheet, file_name);
            }
            cairo_restore(cr);
        }
        if p.ruler {
            self.pl_rulers(cr, p, &mt, &pages);
        }
        cairo_restore(cr);
    }

    /// "Click to add header": light text in the middle of the header or footer area (points).
    unsafe fn pl_faint(&self, cr: *mut cairo_t, text: &str, cx: f64, y: f64, bottom: bool) {
        let layout = pango_cairo_create_layout(cr);
        let f = crate::layout::FontSpec { family: self.styles.theme_fonts.1.clone(), size: 10.0, bold: false, italic: false, underline: 0, strike: false, vert: 0, color: 0 };
        let d = crate::layout::FontDesc::new(&f, 0.75);
        pango_layout_set_font_description(layout, d.0);
        let c = std::ffi::CString::new(text).unwrap_or_default();
        pango_layout_set_text(layout, c.as_ptr(), -1);
        let (mut w, mut h) = (0, 0);
        pango_layout_get_pixel_size(layout, &mut w, &mut h);
        cairo_set_source_rgb(cr, 0.62, 0.62, 0.62);
        cairo_move_to(cr, cx - w as f64 / 2.0, if bottom { y - h as f64 } else { y });
        pango_cairo_show_layout(cr, layout);
        g_object_unref(layout as *mut _);
    }

    unsafe fn pl_rulers(&self, cr: *mut cairo_t, p: &ViewParams, mt: &Metrics, pages: &[PlPage]) {
        let face = if p.dark { (0.25, 0.25, 0.25) } else { (0.96, 0.96, 0.96) };
        let ink = if p.dark { 0.8 } else { 0.35 };
        cairo_set_source_rgb(cr, face.0, face.1, face.2);
        cairo_rectangle(cr, 0.0, 0.0, p.width, RULER);
        cairo_fill(cr);
        cairo_rectangle(cr, 0.0, 0.0, RULER, p.height);
        cairo_fill(cr);
        let inch = 96.0 * p.zoom;
        cairo_set_source_rgb(cr, ink, ink, ink);
        cairo_set_line_width(cr, 1.0);
        // one ruler per page column / row on screen; the margins shaded
        let mut done_c = Vec::new();
        let mut done_r = Vec::new();
        for pg in pages {
            if !done_c.contains(&pg.band_c) {
                done_c.push(pg.band_c);
                cairo_set_source_rgba(cr, 0.5, 0.5, 0.5, 0.25);
                cairo_rectangle(cr, pg.x, 2.0, mt.m[0], RULER - 4.0);
                cairo_rectangle(cr, pg.x + pg.w - mt.m[1], 2.0, mt.m[1], RULER - 4.0);
                cairo_fill(cr);
                cairo_set_source_rgb(cr, ink, ink, ink);
                let mut k = 0;
                while (k as f64) * inch / 8.0 <= pg.w {
                    let x = pg.x + mt.m[0] + (k as f64) * inch / 8.0;
                    if x > pg.x + pg.w {
                        break;
                    }
                    let len = if k % 8 == 0 { 8.0 } else if k % 4 == 0 { 5.0 } else { 3.0 };
                    cairo_move_to(cr, x.floor() + 0.5, RULER - len);
                    cairo_line_to(cr, x.floor() + 0.5, RULER);
                    k += 1;
                }
                cairo_stroke(cr);
            }
            if !done_r.contains(&pg.band_r) {
                done_r.push(pg.band_r);
                cairo_set_source_rgba(cr, 0.5, 0.5, 0.5, 0.25);
                cairo_rectangle(cr, 2.0, pg.y, RULER - 4.0, mt.m[2]);
                cairo_rectangle(cr, 2.0, pg.y + pg.h - mt.m[3], RULER - 4.0, mt.m[3]);
                cairo_fill(cr);
                cairo_set_source_rgb(cr, ink, ink, ink);
                let mut k = 0;
                loop {
                    let y = pg.y + mt.m[2] + (k as f64) * inch / 8.0;
                    if y > pg.y + pg.h {
                        break;
                    }
                    let len = if k % 8 == 0 { 8.0 } else if k % 4 == 0 { 5.0 } else { 3.0 };
                    cairo_move_to(cr, RULER - len, y.floor() + 0.5);
                    cairo_line_to(cr, RULER, y.floor() + 0.5);
                    k += 1;
                }
                cairo_stroke(cr);
            }
        }
    }
}
