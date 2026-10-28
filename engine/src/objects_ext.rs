//! Objects on the sheet: names, alternative text, hiding (the Selection Pane), turning and
//! flipping (Rotate), and groups (Group, Ungroup).
use crate::book::{Book, Delta};
use crate::drawing::{elem_xfrm, main_elem, main_elem_mut, xfrm_of, Anchor, Obj, EMU};
use crate::xml::{Element, Node};

const DECORATIVE_URI: &str = "{C183D7F6-B498-43B3-948B-1728B52AA6E4}";
const NS_ADEC: &str = "http://schemas.microsoft.com/office/drawing/2017/decorative";

fn find<'a>(n: &'a [Node], local: &str) -> Option<&'a Element> {
    for x in n {
        if let Node::Elem(e) = x {
            if e.local() == local {
                return Some(e);
            }
            if matches!(e.local(), "sp" | "pic" | "cxnSp" | "graphicFrame" | "grpSp" | "contentPart" | "nvSpPr" | "nvPicPr"
                | "nvCxnSpPr" | "nvGraphicFramePr" | "nvGrpSpPr" | "nvContentPartPr") {
                if let Some(r) = find(&e.children, local) {
                    return Some(r);
                }
            }
        }
    }
    None
}

fn find_mut<'a>(n: &'a mut [Node], local: &str) -> Option<&'a mut Element> {
    for x in n.iter_mut() {
        if let Node::Elem(e) = x {
            if e.local() == local {
                return Some(e);
            }
            if matches!(e.local(), "sp" | "pic" | "cxnSp" | "graphicFrame" | "grpSp" | "contentPart" | "nvSpPr" | "nvPicPr"
                | "nvCxnSpPr" | "nvGraphicFramePr" | "nvGrpSpPr" | "nvContentPartPr") {
                if let Some(r) = find_mut(&mut e.children, local) {
                    return Some(r);
                }
            }
        }
    }
    None
}

fn deep_mut<'a>(n: &'a mut [Node], local: &str) -> Option<&'a mut Element> {
    for x in n.iter_mut() {
        if let Node::Elem(e) = x {
            if e.local() == local {
                return Some(e);
            }
            if let Some(r) = deep_mut(&mut e.children, local) {
                return Some(r);
            }
        }
    }
    None
}

fn cnvpr(o: &Obj) -> Option<&Element> {
    if let Some(r) = &o.raw {
        return find(&r.children, "cNvPr").or_else(|| r.elems().find_map(|c| find(&c.children, "cNvPr")));
    }
    find(&o.body, "cNvPr")
}

pub fn is_hidden(o: &Obj) -> bool {
    cnvpr(o).and_then(|e| e.attr("hidden")).map(|v| v == "1" || v == "true").unwrap_or(false)
}

/// (rotation in degrees, flipped horizontally, flipped vertically) from the object's xfrm.
pub fn turn_of(o: &Obj) -> (f64, bool, bool) {
    match main_elem(&o.body).and_then(xfrm_of) {
        Some(x) => (x.attr("rot").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 60000.0,
                    x.attr("flipH") == Some("1"), x.attr("flipV") == Some("1")),
        None => (0.0, false, false),
    }
}

/// A group's child space (chOff, chExt) and its turn.
pub fn group_frame(o: &Obj) -> (i64, i64, i64, i64, f64, bool, bool) {
    let (rot, fh, fv) = turn_of(o);
    let x = main_elem(&o.body).and_then(xfrm_of);
    let n = |c: &str, a: &str| x.and_then(|x| x.child(c)).and_then(|v| v.attr(a)).and_then(|v| v.parse::<i64>().ok());
    let (ox, oy, ocx, ocy) = main_elem(&o.body).and_then(elem_xfrm).unwrap_or((0, 0, 1, 1));
    (n("chOff", "x").unwrap_or(ox), n("chOff", "y").unwrap_or(oy), n("chExt", "cx").unwrap_or(ocx), n("chExt", "cy").unwrap_or(ocy), rot, fh, fv)
}

fn set_turn(o: &mut Obj, rot: Option<f64>, fh: Option<bool>, fv: Option<bool>) -> bool {
    let (x, y, cx, cy) = main_elem(&o.body).and_then(elem_xfrm).unwrap_or((0, 0, 0, 0));
    let e = match main_elem_mut(&mut o.body) {
        Some(e) if e.local() != "graphicFrame" && e.local() != "contentPart" => e,
        _ => return false,
    };
    if xfrm_of(e).is_none() {
        let (w, h) = match o.anchor {
            Anchor::One { cx, cy, .. } | Anchor::Abs { cx, cy, .. } => (cx, cy),
            _ => (cx, cy),
        };
        crate::drawing::set_elem_xfrm(e, x, y, w, h);
    }
    let holder = if e.local() == "grpSp" { e.child_mut("grpSpPr") } else { e.child_mut("spPr") };
    let xf = match holder.and_then(|h| h.child_mut("xfrm")) {
        Some(x) => x,
        None => return false,
    };
    if let Some(r) = rot {
        let r = ((r % 360.0) + 360.0) % 360.0;
        if r.abs() < 1e-9 {
            xf.remove_attr("rot");
        } else {
            xf.set_attr("rot", &((r * 60000.0).round() as i64).to_string());
        }
    }
    for (name, v) in [("flipH", fh), ("flipV", fv)] {
        match v {
            Some(true) => xf.set_attr(name, "1"),
            Some(false) => xf.remove_attr(name),
            None => {}
        }
    }
    true
}

pub struct ObjProps {
    pub name: String,
    pub descr: String,
    pub decorative: bool,
    pub hidden: bool,
    pub rot: f64,
    pub flip_h: bool,
    pub flip_v: bool,
    pub group: bool,
    pub members: usize,
}

fn decorative(e: &Element) -> bool {
    e.child("extLst").map(|l| l.elems().any(|x| x.attr("uri") == Some(DECORATIVE_URI)
        && x.elems().any(|d| d.local() == "decorative" && d.attr("val") == Some("1")))).unwrap_or(false)
}

impl Book {
    pub fn object_props(&self, s: usize, i: usize) -> Option<ObjProps> {
        let o = self.sheets.get(s)?.objects.get(i)?;
        let c = cnvpr(o);
        let (rot, fh, fv) = turn_of(o);
        Some(ObjProps {
            name: o.name.clone(),
            descr: c.and_then(|e| e.attr("descr")).unwrap_or("").to_string(),
            decorative: c.map(decorative).unwrap_or(false),
            hidden: is_hidden(o),
            rot,
            flip_h: fh,
            flip_v: fv,
            group: !o.members.is_empty(),
            members: o.members.len(),
        })
    }

    /// Change objects' name, description, decorative mark, hidden state, turn or flip (one
    /// undo step). `rot_by` adds to the turn; flips toggle.
    #[allow(clippy::too_many_arguments)]
    pub fn set_object_props(&mut self, s: usize, which: &[usize], name: Option<&str>, descr: Option<&str>, deco: Option<bool>,
                            hidden: Option<bool>, rot: Option<f64>, rot_by: Option<f64>, flip_h: bool, flip_v: bool,
                            label: &str) -> Result<(), String> {
        let sh = self.sheets.get(s).ok_or("no such sheet")?;
        if let Some(n) = name {
            if n.trim().is_empty() {
                return Err("The name can't be empty.".into());
            }
        }
        let mut objs = sh.objects.clone();
        let mut any = false;
        for &i in which {
            let o = match objs.get_mut(i) {
                Some(o) => o,
                None => continue,
            };
            if rot.is_some() || rot_by.is_some() || flip_h || flip_v {
                if o.chart.is_some() || o.slicer.is_some() {
                    continue;
                }
                let (r0, fh0, fv0) = turn_of(o);
                let r = rot.or(rot_by.map(|d| r0 + d));
                let fh = if flip_h { Some(!fh0) } else { None };
                let fv = if flip_v { Some(!fv0) } else { None };
                if set_turn(o, r, fh, fv) {
                    any = true;
                }
            }
            if name.is_some() || descr.is_some() || deco.is_some() || hidden.is_some() {
                let body: &mut Vec<Node> = &mut o.body;
                let c = match &mut o.raw {
                    Some(raw) => deep_mut(&mut raw.children, "cNvPr"),
                    None => find_mut(body, "cNvPr"),
                };
                let c = match c {
                    Some(c) => c,
                    None => continue,
                };
                if let Some(n) = name {
                    c.set_attr("name", n.trim());
                }
                if let Some(d) = descr {
                    if d.is_empty() {
                        c.remove_attr("descr");
                    } else {
                        c.set_attr("descr", d);
                    }
                }
                if let Some(h) = hidden {
                    if h {
                        c.set_attr("hidden", "1");
                    } else {
                        c.remove_attr("hidden");
                    }
                }
                if let Some(dd) = deco {
                    let pfx = match c.name.find(':') {
                        Some(k) => c.name[..=k].to_string(),
                        None => String::new(),
                    };
                    if let Some(l) = c.child_mut("extLst") {
                        l.children.retain(|n| !matches!(n, Node::Elem(e) if e.attr("uri") == Some(DECORATIVE_URI)));
                    }
                    if dd {
                        let mut ext = Element::with_attrs("a:ext", &[("uri", DECORATIVE_URI)]);
                        ext.push(Element::with_attrs("adec:decorative", &[("xmlns:adec", NS_ADEC), ("val", "1")]));
                        if c.child("extLst").is_none() {
                            c.push(Element::new("a:extLst"));
                        }
                        c.child_mut("extLst").unwrap().push(ext);
                    } else if c.child("extLst").map(|l| l.elems().next().is_none()).unwrap_or(false) {
                        c.remove_children("extLst");
                    }
                    let _ = pfx;
                }
                if let Some(n) = name {
                    o.name = n.trim().to_string();
                }
                any = true;
            }
        }
        if !any {
            return Ok(());
        }
        self.begin(label, s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(())
    }

    /// Group: the objects become one, in the place of the frontmost. Returns its index.
    pub fn group_objects(&mut self, s: usize, which: &[usize]) -> Result<usize, String> {
        let sh = self.sheets.get(s).ok_or("no such sheet")?;
        let mut idx: Vec<usize> = which.iter().copied().filter(|&i| i < sh.objects.len()).collect();
        idx.sort();
        idx.dedup();
        if idx.len() < 2 {
            return Err("Select two or more objects to group.".into());
        }
        for &i in &idx {
            let o = &sh.objects[i];
            if o.slicer.is_some() || o.ink.is_some() || o.raw.is_some() || main_elem(&o.body).is_none() {
                return Err("This object can't be grouped.".into());
            }
        }
        // the union of their frames, in sheet pixels
        let rects: Vec<(f64, f64, f64, f64)> = idx.iter().map(|&i| self.obj_rect(s, &sh.objects[i])).collect();
        let x1 = rects.iter().map(|r| r.0).fold(f64::MAX, f64::min);
        let y1 = rects.iter().map(|r| r.1).fold(f64::MAX, f64::min);
        let x2 = rects.iter().map(|r| r.0 + r.2).fold(f64::MIN, f64::max);
        let y2 = rects.iter().map(|r| r.1 + r.3).fold(f64::MIN, f64::max);
        let e = |v: f64| (v * EMU).round() as i64;
        let mut members = Vec::new();
        for (k, &i) in idx.iter().enumerate() {
            let mut m = sh.objects[i].clone();
            let r = rects[k];
            m.anchor = Anchor::Abs { x: e(r.0), y: e(r.1), cx: e(r.2), cy: e(r.3) };
            m.body.retain(|n| matches!(n, Node::Elem(el) if matches!(el.local(), "sp" | "pic" | "cxnSp" | "graphicFrame" | "grpSp")));
            members.push(m);
        }
        let (id, name) = self.next_object_name(s, "Group");
        let gx = format!("<x><xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id=\"{id}\" name=\"{name}\"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr><xdr:grpSpPr><a:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/><a:chOff x=\"{x}\" y=\"{y}\"/><a:chExt cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm></xdr:grpSpPr></xdr:grpSp><xdr:clientData/></x>",
                         id = id, name = crate::xml::escape_attr(&name), x = e(x1), y = e(y1), cx = e(x2 - x1), cy = e(y2 - y1));
        let body = crate::xml::parse(&gx).map(|(_, el)| el.children).map_err(|e| e.to_string())?;
        let like = Anchor::Two { from: Default::default(), to: Default::default(), edit_as: None };
        let anchor = self.anchor_at(s, &like, x1, y1, x2 - x1, y2 - y1);
        let g = Obj { anchor, body, chart: None, name, part: None, picture: None, pic_part: None, ink: None, ink_part: None, raw: None,
                      slicer: None, members };
        let mut objs: Vec<Obj> = Vec::new();
        let at = *idx.last().unwrap() + 1 - idx.len();
        for (k, o) in sh.objects.iter().enumerate() {
            if !idx.contains(&k) {
                objs.push(o.clone());
            }
        }
        objs.insert(at, g);
        self.begin("Group", s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok(at)
    }

    /// Ungroup: a group's members become objects again. Returns their indexes.
    pub fn ungroup_object(&mut self, s: usize, i: usize) -> Result<Vec<usize>, String> {
        let sh = self.sheets.get(s).ok_or("no such sheet")?;
        let g = sh.objects.get(i).ok_or("no such object")?;
        if g.members.is_empty() {
            return Err("Select a group to ungroup.".into());
        }
        let (gx, gy, gw, gh) = self.obj_rect(s, g);
        let (cx0, cy0, ccx, ccy, _rot, _fh, _fv) = group_frame(g);
        let sx = gw / ccx.max(1) as f64;
        let sy = gh / ccy.max(1) as f64;
        let mut out = Vec::new();
        for m in &g.members {
            let mut o = m.clone();
            if let Anchor::Abs { x, y, cx, cy } = m.anchor {
                let (px, py, pw, ph) = (gx + (x - cx0) as f64 * sx, gy + (y - cy0) as f64 * sy, cx as f64 * sx, cy as f64 * sy);
                let like = Anchor::Two { from: Default::default(), to: Default::default(), edit_as: None };
                o.anchor = self.anchor_at(s, &like, px, py, pw, ph);
                let e = |v: f64| (v * EMU).round() as i64;
                if let Some(el) = main_elem_mut(&mut o.body) {
                    crate::drawing::set_elem_xfrm(el, e(px), e(py), e(pw), e(ph));
                }
                if !o.members.is_empty() {
                    // a nested group keeps its own child space; its frame is where it shows now
                }
            }
            if let Ok((_, cd)) = crate::xml::parse("<xdr:clientData/>") {
                o.body.push(Node::Elem(cd));
            }
            out.push(o);
        }
        let mut objs = sh.objects.clone();
        objs.remove(i);
        let n = out.len();
        for (k, o) in out.into_iter().enumerate() {
            objs.insert(i + k, o);
        }
        self.begin("Ungroup", s);
        self.record(Delta::Objects(s, objs));
        self.commit();
        Ok((i..i + n).collect())
    }
}
