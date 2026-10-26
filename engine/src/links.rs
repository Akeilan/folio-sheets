//! Hyperlinks: the sheet's <hyperlinks> element, kept as a piece. A link to a web page or
//! file is held in the piece with its address in a private `_target` attribute while the
//! book is open; saving turns it back into a relationship (r:id), reading the other way.

use crate::book::{Book, Delta};
use crate::cellref::{parse_range, Rect};
use crate::cf::subtract;
use crate::sheet::Piece;
use crate::xml::{self, Element, Node};

/// The attribute that holds an outside address while the book is open.
pub const TARGET: &str = "_target";
pub const REL_HYPERLINK: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub rect: Rect,
    /// a web address, file or mailto: (outside the book)
    pub target: String,
    /// a place in the book: Sheet2!A1, a defined name
    pub location: String,
    pub display: String,
    pub tooltip: String,
}

impl Link {
    fn to_element(&self, prefix: &str) -> Element {
        let mut e = Element::new(&format!("{}hyperlink", prefix));
        e.set_attr("ref", &self.rect.a1());
        if !self.target.is_empty() {
            e.set_attr(TARGET, &self.target);
        }
        if !self.location.is_empty() {
            e.set_attr("location", &self.location);
        }
        if !self.tooltip.is_empty() {
            e.set_attr("tooltip", &self.tooltip);
        }
        if !self.display.is_empty() {
            e.set_attr("display", &self.display);
        }
        e
    }

    fn from_element(e: &Element) -> Option<Link> {
        let rect = parse_range(e.attr("ref")?)?;
        Some(Link {
            rect,
            target: e.attr_exact(TARGET).unwrap_or("").to_string(),
            location: e.attr("location").unwrap_or("").to_string(),
            display: e.attr("display").unwrap_or("").to_string(),
            tooltip: e.attr("tooltip").unwrap_or("").to_string(),
        })
    }
}

/// The element-name prefix the sheet uses ("" or "x:").
fn prefix_of(root_open: &str) -> String {
    let n = root_open.trim_start_matches('<');
    let n = &n[..n.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(n.len())];
    match n.find(':') {
        Some(k) => n[..=k].to_string(),
        None => String::new(),
    }
}

/// While reading: each hyperlink's r:id becomes the address it names.
pub fn resolve_on_load(xml_text: &str, rels: &[(String, String, String, bool)]) -> String {
    let (_, mut e) = match xml::parse(xml_text) {
        Ok(x) => x,
        Err(_) => return xml_text.to_string(),
    };
    let mut changed = false;
    for h in e.elems_mut() {
        if h.local() != "hyperlink" {
            continue;
        }
        let key = h.attrs.iter().find(|(k, _)| k.contains(':') && xml::local(k) == "id").map(|(k, v)| (k.clone(), v.clone()));
        if let Some((k, rid)) = key {
            if let Some(rel) = rels.iter().find(|r| r.0 == rid) {
                h.remove_attr(&k);
                h.set_attr(TARGET, &rel.2);
                changed = true;
            }
        }
    }
    if changed {
        e.to_string()
    } else {
        xml_text.to_string()
    }
}

/// While saving: the addresses go back to relationships. `new_rel(target)` adds one and
/// gives its id.
pub fn write_out(xml_text: &str, mut new_rel: impl FnMut(&str) -> String) -> String {
    let (_, mut e) = match xml::parse(xml_text) {
        Ok(x) => x,
        Err(_) => return xml_text.to_string(),
    };
    let mut any = false;
    for h in e.elems_mut() {
        if let Some(t) = h.attr_exact(TARGET).map(|s| s.to_string()) {
            h.remove_attr(TARGET);
            let id = new_rel(&t);
            // r:id first, as it is written in files
            h.attrs.insert(1.min(h.attrs.len()), ("r:id".into(), id));
            any = true;
        }
    }
    if any && e.attr_exact("xmlns:r").is_none() {
        e.set_attr("xmlns:r", REL_NS);
    }
    e.to_string()
}

/// Whether a piece holds addresses that need relationships.
pub fn has_targets(sh: &crate::sheet::Sheet) -> bool {
    sh.pieces.iter().any(|p| matches!(p, Piece::Raw(t, x) if t == "hyperlinks" && x.contains(TARGET)))
}

impl Book {
    /// Where a =HYPERLINK(location, ...) formula in the cell points (its first argument).
    pub fn formula_link(&self, s: usize, r: u32, c: u32) -> Option<String> {
        let t = self.input(s, r, c);
        let body = t.strip_prefix('=')?;
        if !body.trim_start().to_ascii_uppercase().starts_with("HYPERLINK(") {
            return None;
        }
        let ex = crate::formula::parse(body).ok()?;
        let arg = match &ex {
            crate::formula::Expr::Func(name, args) if name.eq_ignore_ascii_case("HYPERLINK") && !args.is_empty() => args[0].clone(),
            _ => return None,
        };
        let mut ctx = crate::cf::ViewCtx { b: self };
        let mut ev = crate::calc::Eval::new(&mut ctx, s, r, c, true);
        match ev.eval_top(&arg) {
            crate::value::Value::Str(t) if !t.is_empty() => Some(t.to_string()),
            _ => None,
        }
    }

    /// The sheet's links, in file order.
    pub fn links(&self, s: usize) -> Vec<Link> {
        let mut out = Vec::new();
        for p in &self.sheets[s].pieces {
            if let Piece::Raw(t, x) = p {
                if t == "hyperlinks" {
                    if let Ok((_, e)) = xml::parse(x) {
                        out.extend(e.elems().filter(|h| h.local() == "hyperlink").filter_map(Link::from_element));
                    }
                }
            }
        }
        out
    }

    /// The link on a cell (or on the merged block it is in).
    pub fn link_at(&self, s: usize, r: u32, c: u32) -> Option<Link> {
        if s >= self.sheets.len() || !self.sheets[s].pieces.iter().any(|p| matches!(p, Piece::Raw(t, _) if t == "hyperlinks")) {
            return None;
        }
        let (r, c) = match self.sheets[s].merge_at(r, c) {
            Some(m) => (m.r1, m.c1),
            None => (r, c),
        };
        self.links(s).into_iter().find(|l| l.rect.contains(r, c))
    }

    /// The pieces with the links over `cut` taken off those cells (a link over a wider
    /// range keeps the rest), and `add` added.
    fn link_pieces(&self, s: usize, cut: &[Rect], add: &[Link]) -> Vec<Piece> {
        let sh = &self.sheets[s];
        let pfx = prefix_of(&sh.root_open);
        let mut out = Vec::new();
        let mut keep: Vec<Link> = Vec::new();
        let mut at = None;
        for p in sh.pieces.iter() {
            match p {
                Piece::Raw(t, x) if t == "hyperlinks" => {
                    if let Ok((_, e)) = xml::parse(x) {
                        for h in e.elems().filter(|h| h.local() == "hyperlink") {
                            if let Some(l) = Link::from_element(h) {
                                let mut left = vec![l.rect];
                                for c in cut {
                                    left = left.into_iter().flat_map(|a| subtract(a, c)).collect();
                                }
                                for rc in left {
                                    keep.push(Link { rect: rc, ..l.clone() });
                                }
                            }
                        }
                    }
                    if at.is_none() {
                        at = Some(out.len());
                    }
                }
                other => out.push(other.clone()),
            }
        }
        keep.extend(add.iter().cloned());
        if !keep.is_empty() {
            let mut e = Element::new(&format!("{}hyperlinks", pfx));
            for l in &keep {
                e.children.push(Node::Elem(l.to_element(&pfx)));
            }
            let piece = Piece::Raw("hyperlinks".into(), e.to_string());
            match at {
                Some(i) => out.insert(i, piece),
                None => out.push(piece),
            }
        }
        out
    }

    fn record_link_pieces(&mut self, s: usize, pieces: Vec<Piece>, label: &str) {
        if pieces == self.sheets[s].pieces {
            return;
        }
        self.begin(label, s);
        self.record(Delta::Pieces(s, pieces));
        self.commit();
    }

    /// Put a link on a range (it replaces the links there).
    pub fn set_link(&mut self, s: usize, rect: Rect, link: &Link) -> Result<(), String> {
        if link.target.trim().is_empty() && link.location.trim().is_empty() {
            return Err("Type an address or pick a place in this document.".into());
        }
        let rect = self.sheets[s].clamp_rect(rect);
        let l = Link { rect, ..link.clone() };
        let pieces = self.link_pieces(s, &[rect], &[l]);
        self.record_link_pieces(s, pieces, "Insert Hyperlink");
        Ok(())
    }

    /// Take the links off the ranges (their cells keep their look).
    pub fn clear_links(&mut self, s: usize, ranges: &[Rect]) -> usize {
        let ranges: Vec<Rect> = ranges.iter().map(|r| self.sheets[s].clamp_rect(*r)).collect();
        let n = self.links(s).iter().filter(|l| ranges.iter().any(|r| r.intersects(&l.rect))).count();
        let pieces = self.link_pieces(s, &ranges, &[]);
        self.record_link_pieces(s, pieces, "Clear Hyperlinks");
        n
    }

    /// Copy the links of `src` onto `dst`, repeating `src` (paste).
    pub fn copy_links(&mut self, s: usize, src: Rect, dst_s: usize, dst: Rect) {
        let links = self.links(s);
        let (h, w) = (src.r2 - src.r1 + 1, src.c2 - src.c1 + 1);
        let mut add = Vec::new();
        for l in links.iter() {
            if let Some(part) = l.rect.intersect(&src) {
                let mut r0 = dst.r1;
                while r0 <= dst.r2 {
                    let mut c0 = dst.c1;
                    while c0 <= dst.c2 {
                        let dr = r0 as i64 - src.r1 as i64;
                        let dc = c0 as i64 - src.c1 as i64;
                        let moved = Rect::new((part.r1 as i64 + dr) as u32, (part.c1 as i64 + dc) as u32, (part.r2 as i64 + dr) as u32, (part.c2 as i64 + dc) as u32);
                        if let Some(x) = moved.intersect(&dst) {
                            add.push(Link { rect: x, ..l.clone() });
                        }
                        c0 += w;
                    }
                    r0 += h;
                }
            }
        }
        let has_any = self.sheets[dst_s].pieces.iter().any(|p| matches!(p, Piece::Raw(t, _) if t == "hyperlinks"));
        if add.is_empty() && !has_any {
            return;
        }
        let pieces = self.link_pieces(dst_s, &[dst], &add);
        if pieces != self.sheets[dst_s].pieces {
            self.record(Delta::Pieces(dst_s, pieces));
        }
    }
}
