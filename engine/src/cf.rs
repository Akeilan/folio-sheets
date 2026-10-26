//! Conditional formatting: the rules as the file keeps them (worksheet
//! conditionalFormatting elements, plus the 2010 extensions), what they do to a cell, and
//! the edits the commands make.
//!
//! The worksheet keeps each conditionalFormatting element as written, so inserting and
//! deleting cells adjusts them like the other kept elements; the model here is read from
//! them (and cached).

use crate::book::{Book, Delta};
use crate::calc::{self, Ctx, Eval};
use crate::cellref::{parse_range, Rect, MAX_COLS, MAX_ROWS};
use crate::formula::{self, Expr, RefKind};
use crate::sheet::Piece;
use crate::styles::Color;
use crate::value::{Array, Value, XlErr};
use crate::xml::{self, Element, Node};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub struct Cfvo {
    /// min, max, num, percent, percentile, formula, autoMin, autoMax
    pub typ: String,
    pub val: String,
    pub gte: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub color: u32,
    pub min_len: f64,
    pub max_len: f64,
    pub gradient: bool,
    pub border: Option<u32>,
    pub neg_color: Option<u32>,
    pub neg_border: Option<u32>,
    pub axis: bool,
    pub axis_color: u32,
    pub rtl: bool,
}

#[derive(Clone, Debug)]
pub struct CfRule {
    pub typ: String,
    pub priority: i64,
    pub stop: bool,
    pub dxf: Option<u32>,
    pub operator: String,
    /// formula texts (relative to the top-left cell of the block's first range) and parsed
    pub formulas: Vec<(String, Option<Rc<Expr>>)>,
    pub text: String,
    pub time_period: String,
    pub rank: u32,
    pub percent: bool,
    pub bottom: bool,
    pub above: bool,
    pub equal: bool,
    pub std_dev: u32,
    pub cfvos: Vec<Cfvo>,
    pub colors: Vec<u32>,
    pub bar: Option<Bar>,
    pub icon_set: String,
    pub reverse: bool,
    pub show_value: bool,
    /// per-threshold icons (custom sets): (set, index); empty: the set's own
    pub icons: Vec<(String, u32)>,
    pub ext_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CfBlock {
    pub ranges: Vec<Rect>,
    pub rules: Vec<CfRule>,
    /// index of its piece in the sheet (None: from the 2010 extension list)
    pub piece: Option<usize>,
}

impl CfBlock {
    pub fn contains(&self, r: u32, c: u32) -> bool {
        self.ranges.iter().any(|x| x.contains(r, c))
    }
    pub fn origin(&self) -> (u32, u32) {
        self.ranges.first().map(|x| (x.r1, x.c1)).unwrap_or((0, 0))
    }
}

/// A differential format (what a rule changes).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dxf {
    pub color: Option<u32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub strike: Option<bool>,
    pub underline: Option<u8>,
    pub fill: Option<u32>,
    pub numfmt: Option<String>,
    /// left, right, top, bottom
    pub borders: [Option<(String, u32)>; 4],
}

/// What conditional formatting does to one cell.
#[derive(Clone, Debug, Default)]
pub struct CfOut {
    pub dxf: Dxf,
    pub scale: Option<u32>,
    /// (start, end) as fractions of the cell's width, colour, bar settings, negative
    pub bar: Option<(f64, f64, Bar, bool, Option<f64>)>,
    pub icon: Option<(String, u32)>,
    pub hide_value: bool,
}

#[derive(Debug, Default)]
pub struct Stats {
    pub nums: Vec<f64>,
    pub mean: f64,
    pub sd: f64,
    pub counts: HashMap<String, usize>,
}

// ---------------------------------------------------------------------- reading

fn color_el(e: &Element, styles: &crate::styles::Styles) -> u32 {
    styles.rgb(&Color::from_el(e), 0)
}

fn flag(e: &Element, name: &str, default: bool) -> bool {
    match e.attr(name) {
        Some(v) => v == "1" || v.eq_ignore_ascii_case("true"),
        None => default,
    }
}

fn read_cfvo(e: &Element) -> Cfvo {
    let mut val = e.attr("val").unwrap_or("").to_string();
    // 2010: the value is an xm:f child
    if let Some(f) = e.elems().find(|x| x.local() == "f") {
        val = f.text();
    }
    Cfvo { typ: e.attr("type").unwrap_or("num").to_string(), val, gte: flag(e, "gte", true) }
}

fn read_rule(e: &Element, styles: &crate::styles::Styles) -> CfRule {
    let typ = e.attr("type").unwrap_or("").to_string();
    let mut formulas = Vec::new();
    for f in e.elems().filter(|x| x.local() == "formula" || x.local() == "f") {
        let t = formula::from_file_text(&f.text());
        let ex = formula::parse(&t).ok().map(Rc::new);
        formulas.push((t, ex));
    }
    let mut rule = CfRule {
        typ: typ.clone(),
        priority: e.attr("priority").and_then(|v| v.parse().ok()).unwrap_or(1),
        stop: flag(e, "stopIfTrue", false),
        dxf: e.attr("dxfId").and_then(|v| v.parse().ok()),
        operator: e.attr("operator").unwrap_or("").to_string(),
        formulas,
        text: e.attr("text").unwrap_or("").to_string(),
        time_period: e.attr("timePeriod").unwrap_or("").to_string(),
        rank: e.attr("rank").and_then(|v| v.parse().ok()).unwrap_or(10),
        percent: flag(e, "percent", false),
        bottom: flag(e, "bottom", false),
        above: flag(e, "aboveAverage", true),
        equal: flag(e, "equalAverage", false),
        std_dev: e.attr("stdDev").and_then(|v| v.parse().ok()).unwrap_or(0),
        cfvos: vec![],
        colors: vec![],
        bar: None,
        icon_set: String::new(),
        reverse: false,
        show_value: true,
        icons: vec![],
        ext_id: None,
    };
    for c in e.elems() {
        match c.local() {
            "colorScale" => {
                rule.cfvos = c.elems().filter(|x| x.local() == "cfvo").map(read_cfvo).collect();
                rule.colors = c.elems().filter(|x| x.local() == "color").map(|x| color_el(x, styles)).collect();
            }
            "dataBar" => {
                rule.cfvos = c.elems().filter(|x| x.local() == "cfvo").map(read_cfvo).collect();
                let col = c.elems().find(|x| x.local() == "color").map(|x| color_el(x, styles)).unwrap_or(0x638EC6);
                rule.show_value = flag(c, "showValue", true);
                rule.bar = Some(Bar {
                    color: col,
                    min_len: c.attr("minLength").and_then(|v| v.parse().ok()).unwrap_or(10.0),
                    max_len: c.attr("maxLength").and_then(|v| v.parse().ok()).unwrap_or(90.0),
                    gradient: true,
                    border: None,
                    neg_color: None,
                    neg_border: None,
                    axis: false,
                    axis_color: 0,
                    rtl: false,
                });
            }
            "iconSet" => {
                rule.cfvos = c.elems().filter(|x| x.local() == "cfvo").map(read_cfvo).collect();
                rule.icon_set = c.attr("iconSet").unwrap_or("3TrafficLights1").to_string();
                rule.reverse = flag(c, "reverse", false);
                rule.show_value = flag(c, "showValue", true);
                rule.icons = c.elems().filter(|x| x.local() == "cfIcon")
                    .map(|x| (x.attr("iconSet").unwrap_or("").to_string(), x.attr("iconId").and_then(|v| v.parse().ok()).unwrap_or(0))).collect();
            }
            "extLst" => {
                // the id of the 2010 rule that goes with this one
                for ext in c.elems() {
                    for id in ext.elems() {
                        if id.local() == "id" {
                            rule.ext_id = Some(id.text());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    rule
}

/// The 2010 data bar settings onto a rule.
fn apply_ext_bar(rule: &mut CfRule, x: &Element, styles: &crate::styles::Styles) {
    let db = match x.elems().find(|e| e.local() == "dataBar") {
        Some(d) => d,
        None => return,
    };
    let bar = match &mut rule.bar {
        Some(b) => b,
        None => return,
    };
    bar.min_len = db.attr("minLength").and_then(|v| v.parse().ok()).unwrap_or(0.0);
    bar.max_len = db.attr("maxLength").and_then(|v| v.parse().ok()).unwrap_or(100.0);
    bar.gradient = flag(db, "gradient", true);
    bar.rtl = db.attr("direction") == Some("rightToLeft");
    let same_neg = flag(db, "negativeBarColorSameAsPositive", false);
    let same_neg_border = flag(db, "negativeBarBorderColorSameAsPositive", true);
    bar.axis = db.attr("axisPosition") != Some("none");
    for c in db.elems() {
        match c.local() {
            "borderColor" => {
                if flag(db, "border", false) {
                    bar.border = Some(color_el(c, styles));
                }
            }
            "negativeFillColor" => {
                if !same_neg {
                    bar.neg_color = Some(color_el(c, styles));
                }
            }
            "negativeBorderColor" => {
                if !same_neg_border {
                    bar.neg_border = Some(color_el(c, styles));
                }
            }
            "axisColor" => bar.axis_color = color_el(c, styles),
            _ => {}
        }
    }
    if bar.neg_color.is_none() && !same_neg {
        bar.neg_color = Some(0xFF0000);
    }
    let cfvos: Vec<Cfvo> = db.elems().filter(|x| x.local() == "cfvo").map(read_cfvo).collect();
    if cfvos.len() == 2 {
        rule.cfvos = cfvos;
    }
}

fn parse_sqref(s: &str) -> Vec<Rect> {
    s.split_whitespace().filter_map(|t| parse_range(&t.replace('$', ""))).collect()
}

fn fingerprint(pieces: &[Piece]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in pieces {
        if let Piece::Raw(t, x) = p {
            if t == "conditionalFormatting" || t == "extLst" {
                for b in x.as_bytes() {
                    h ^= *b as u64;
                    h = h.wrapping_mul(0x100000001b3);
                }
                h ^= 0xff;
            }
        }
    }
    h
}

impl Book {
    /// The sheet's rules, read from its kept elements.
    pub fn cf_blocks(&self, s: usize) -> Rc<Vec<CfBlock>> {
        let sh = &self.sheets[s];
        let fp = fingerprint(&sh.pieces);
        if let Some((k, v)) = self.cf_cache.borrow().get(&s) {
            if *k == fp {
                return v.clone();
            }
        }
        let mut blocks: Vec<CfBlock> = Vec::new();
        let mut ext: HashMap<String, Element> = HashMap::new();
        let mut ext_blocks: Vec<(Vec<Rect>, Vec<Element>)> = Vec::new();
        for p in sh.pieces.iter() {
            if let Piece::Raw(t, x) = p {
                if t == "extLst" {
                    if let Ok((_, e)) = xml::parse(x) {
                        for ex in e.elems() {
                            for cfs in ex.elems().filter(|x| x.local() == "conditionalFormattings") {
                                for cf in cfs.elems().filter(|x| x.local() == "conditionalFormatting") {
                                    let sq = cf.elems().find(|x| x.local() == "sqref").map(|x| x.text()).unwrap_or_default();
                                    let mut rules = Vec::new();
                                    for r in cf.elems().filter(|x| x.local() == "cfRule") {
                                        if let Some(id) = r.attr("id") {
                                            ext.insert(id.to_string(), r.clone());
                                        }
                                        rules.push(r.clone());
                                    }
                                    ext_blocks.push((parse_sqref(&sq), rules));
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut used_ext: Vec<String> = Vec::new();
        for (i, p) in sh.pieces.iter().enumerate() {
            if let Piece::Raw(t, x) = p {
                if t != "conditionalFormatting" {
                    continue;
                }
                let e = match xml::parse(x) {
                    Ok((_, e)) => e,
                    Err(_) => continue,
                };
                let ranges = parse_sqref(e.attr("sqref").unwrap_or(""));
                let mut rules = Vec::new();
                for r in e.elems().filter(|x| x.local() == "cfRule") {
                    let mut rule = read_rule(r, &self.styles);
                    if let Some(id) = rule.ext_id.clone() {
                        if let Some(x) = ext.get(&id) {
                            apply_ext_bar(&mut rule, x, &self.styles);
                            if rule.typ == "iconSet" {
                                if let Some(is) = x.elems().find(|e| e.local() == "iconSet") {
                                    let r2 = read_rule(x, &self.styles);
                                    let _ = is;
                                    rule.icons = r2.icons;
                                    rule.icon_set = r2.icon_set;
                                    rule.cfvos = r2.cfvos;
                                }
                            }
                            used_ext.push(id.clone());
                        }
                    }
                    rules.push(rule);
                }
                if !ranges.is_empty() {
                    blocks.push(CfBlock { ranges, rules, piece: Some(i) });
                }
            }
        }
        // rules only the extension list has (2010 icon sets, ...)
        for (ranges, rules) in ext_blocks {
            let mut out = Vec::new();
            for r in rules {
                if r.attr("id").map(|id| used_ext.iter().any(|u| u == id)).unwrap_or(false) {
                    continue;
                }
                let mut rule = read_rule(&r, &self.styles);
                if rule.typ == "dataBar" {
                    apply_ext_bar(&mut rule, &r, &self.styles);
                }
                // their dxfs are inline: not modelled
                out.push(rule);
            }
            if !out.is_empty() && !ranges.is_empty() {
                blocks.push(CfBlock { ranges, rules: out, piece: None });
            }
        }
        let rc = Rc::new(blocks);
        self.cf_cache.borrow_mut().insert(s, (fp, rc.clone()));
        rc
    }

    /// A dxf from the styles part.
    pub fn dxf(&self, i: u32) -> Dxf {
        let list = match self.styles.root.child("dxfs") {
            Some(d) => d,
            None => return Dxf::default(),
        };
        let e = match list.elems().nth(i as usize) {
            Some(e) => e,
            None => return Dxf::default(),
        };
        let mut d = Dxf::default();
        if let Some(f) = e.child("font") {
            let on = |n: &str| f.child(n).map(|x| !matches!(x.attr("val"), Some("0") | Some("false")));
            d.bold = on("b");
            d.italic = on("i");
            d.strike = on("strike");
            d.underline = f.child("u").map(|x| match x.attr("val").unwrap_or("single") {
                "double" => 2,
                "none" => 0,
                _ => 1,
            });
            d.color = f.child("color").map(|c| color_el(c, &self.styles));
        }
        if let Some(fill) = e.child("fill") {
            if let Some(pf) = fill.child("patternFill") {
                let pt = pf.attr("patternType").unwrap_or("solid");
                if pt != "none" {
                    // dxf fills keep the colour in bgColor
                    let c = pf.child("bgColor").or_else(|| pf.child("fgColor"));
                    d.fill = c.map(|c| color_el(c, &self.styles));
                }
            } else if let Some(g) = fill.child("gradientFill") {
                d.fill = g.elems().next().and_then(|s| s.child("color")).map(|c| color_el(c, &self.styles));
            }
        }
        if let Some(n) = e.child("numFmt") {
            d.numfmt = n.attr("formatCode").map(|s| s.to_string());
        }
        if let Some(b) = e.child("border") {
            for (k, side) in ["left", "right", "top", "bottom"].iter().enumerate() {
                if let Some(x) = b.child(side) {
                    if let Some(st) = x.attr("style") {
                        let col = x.child("color").map(|c| color_el(c, &self.styles)).unwrap_or(0);
                        d.borders[k] = Some((st.to_string(), col));
                    }
                }
            }
        }
        d
    }

    fn cf_stats(&self, s: usize, bi: usize, ri: usize, block: &CfBlock) -> Rc<Stats> {
        let key = (s, bi, ri);
        let ver = (self.edits, self.calc_count as u64);
        if let Some((v, st)) = self.cf_stats.borrow().get(&key) {
            if *v == ver {
                return st.clone();
            }
        }
        let sh = &self.sheets[s];
        let mut st = Stats::default();
        for rc in &block.ranges {
            let rc = sh.clamp_rect(*rc);
            for (r, c) in self.positions_with_values(s, rc) {
                let v = sh.value(r, c);
                match &v {
                    Value::Num(n) => st.nums.push(*n),
                    Value::Empty => continue,
                    _ => {}
                }
                *st.counts.entry(key_of(&v)).or_insert(0) += 1;
            }
        }
        st.nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = st.nums.len() as f64;
        if n > 0.0 {
            st.mean = st.nums.iter().sum::<f64>() / n;
            if n > 1.0 {
                st.sd = (st.nums.iter().map(|x| (x - st.mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
            }
        }
        let rc = Rc::new(st);
        self.cf_stats.borrow_mut().insert(key, (ver, rc.clone()));
        rc
    }

    /// Cells with something in them (values and spilled results) inside rc.
    fn positions_with_values(&self, s: usize, rc: Rect) -> Vec<(u32, u32)> {
        let sh = &self.sheets[s];
        let mut out: Vec<(u32, u32)> = sh.positions_in(rc).into_iter().filter(|(r, c)| !sh.cells[&(*r, *c)].is_blank()).collect();
        for (r, c) in sh.ghosts.keys() {
            if rc.contains(*r, *c) && !sh.cells.contains_key(&(*r, *c)) {
                out.push((*r, *c));
            }
        }
        out
    }

    /// Evaluate a rule's formula k for cell (r, c) of the block.
    fn cf_formula(&self, s: usize, block: &CfBlock, rule: &CfRule, k: usize, r: u32, c: u32) -> Value {
        let ex = match rule.formulas.get(k).and_then(|f| f.1.as_ref()) {
            Some(e) => e,
            None => return Value::Err(XlErr::Name),
        };
        let (r0, c0) = block.origin();
        let shifted = shift_expr(ex, r as i64 - r0 as i64, c as i64 - c0 as i64);
        let mut ctx = ViewCtx { b: self };
        let mut ev = Eval::new(&mut ctx, s, r, c, true);
        let v = ev.eval_top(&shifted);
        match v {
            Value::Arr(a) => a.get(0, 0).clone(),
            v => v,
        }
    }

    fn cfvo_value(&self, s: usize, block: &CfBlock, st: &Stats, v: &Cfvo, is_max: bool) -> Option<f64> {
        let nums = &st.nums;
        let (lo, hi) = (*nums.first()?, *nums.last()?);
        Some(match v.typ.as_str() {
            "min" => lo,
            "max" => hi,
            "autoMin" => lo.min(0.0),
            "autoMax" => hi.max(0.0),
            "num" => v.val.parse().ok().or_else(|| self.cfvo_formula(s, block, &v.val))?,
            "percent" => {
                let p: f64 = v.val.parse().ok().or_else(|| self.cfvo_formula(s, block, &v.val))?;
                lo + (hi - lo) * p / 100.0
            }
            "percentile" => {
                let p: f64 = v.val.parse().ok().or_else(|| self.cfvo_formula(s, block, &v.val))?;
                percentile(nums, p / 100.0)
            }
            "formula" => self.cfvo_formula(s, block, &v.val)?,
            _ => if is_max { hi } else { lo },
        })
    }

    fn cfvo_formula(&self, s: usize, block: &CfBlock, text: &str) -> Option<f64> {
        let t = text.trim_start_matches('=');
        let ex = formula::parse(t).ok()?;
        let (r0, c0) = block.origin();
        let mut ctx = ViewCtx { b: self };
        let mut ev = Eval::new(&mut ctx, s, r0, c0, true);
        match ev.eval_top(&ex) {
            Value::Num(n) => Some(n),
            _ => None,
        }
    }

    /// What the rules do to cell (r, c).
    pub fn cf_at(&self, s: usize, r: u32, c: u32, blocks: &[CfBlock]) -> Option<CfOut> {
        // the rules that apply here, by priority
        let mut list: Vec<(i64, usize, usize)> = Vec::new();
        for (bi, b) in blocks.iter().enumerate() {
            if b.contains(r, c) {
                for (ri, rule) in b.rules.iter().enumerate() {
                    list.push((rule.priority, bi, ri));
                }
            }
        }
        if list.is_empty() {
            return None;
        }
        list.sort();
        let v = self.sheets[s].value(r, c);
        let mut out = CfOut::default();
        let mut any = false;
        let mut have_scale = false;
        let mut have_bar = false;
        let mut have_icon = false;
        for (_, bi, ri) in list {
            let block = &blocks[bi];
            let rule = &block.rules[ri];
            let hit = match rule.typ.as_str() {
                "colorScale" => {
                    if !have_scale {
                        if let Some(col) = self.scale_color(s, bi, ri, block, rule, &v) {
                            out.scale = Some(col);
                            have_scale = true;
                            any = true;
                        }
                    }
                    continue;
                }
                "dataBar" => {
                    if !have_bar {
                        if let Some(b) = self.bar_of(s, bi, ri, block, rule, &v) {
                            out.bar = Some(b);
                            out.hide_value |= !rule.show_value;
                            have_bar = true;
                            any = true;
                        }
                    }
                    continue;
                }
                "iconSet" => {
                    if !have_icon {
                        if let Some(ic) = self.icon_of(s, bi, ri, block, rule, &v) {
                            out.icon = Some(ic);
                            out.hide_value |= !rule.show_value;
                            have_icon = true;
                            any = true;
                        }
                    }
                    continue;
                }
                _ => self.rule_hits(s, bi, ri, block, rule, r, c, &v),
            };
            if !hit {
                continue;
            }
            if let Some(d) = rule.dxf {
                let d = self.dxf(d);
                let o = &mut out.dxf;
                if o.color.is_none() { o.color = d.color }
                if o.bold.is_none() { o.bold = d.bold }
                if o.italic.is_none() { o.italic = d.italic }
                if o.strike.is_none() { o.strike = d.strike }
                if o.underline.is_none() { o.underline = d.underline }
                if o.fill.is_none() { o.fill = d.fill }
                if o.numfmt.is_none() { o.numfmt = d.numfmt }
                for k in 0..4 {
                    if o.borders[k].is_none() {
                        o.borders[k] = d.borders[k].clone();
                    }
                }
            }
            any = true;
            if rule.stop {
                break;
            }
        }
        if any { Some(out) } else { None }
    }

    #[allow(clippy::too_many_arguments)]
    fn rule_hits(&self, s: usize, bi: usize, ri: usize, block: &CfBlock, rule: &CfRule, r: u32, c: u32, v: &Value) -> bool {
        let truthy = |x: &Value| -> bool {
            match x {
                Value::Bool(b) => *b,
                Value::Num(n) => *n != 0.0,
                _ => false,
            }
        };
        match rule.typ.as_str() {
            "cellIs" => {
                let a = self.cf_formula(s, block, rule, 0, r, c);
                let cmp = |op: formula::BinOp, x: &Value, y: &Value| truthy(&calc::scalar_op(op, x, y, self.date1904));
                let val = if matches!(v, Value::Empty) { Value::num(0.0) } else { v.clone() };
                if matches!(val, Value::Err(_)) {
                    return false;
                }
                use formula::BinOp::*;
                match rule.operator.as_str() {
                    "lessThan" => cmp(Lt, &val, &a),
                    "lessThanOrEqual" => cmp(Le, &val, &a),
                    "equal" => cmp(Eq, &val, &a),
                    "notEqual" => cmp(Ne, &val, &a),
                    "greaterThanOrEqual" => cmp(Ge, &val, &a),
                    "greaterThan" => cmp(Gt, &val, &a),
                    "between" | "notBetween" => {
                        let b = self.cf_formula(s, block, rule, 1, r, c);
                        let (lo, hi) = if calc::compare(&a, &b) == std::cmp::Ordering::Greater { (b, a) } else { (a, b) };
                        let inside = cmp(Ge, &val, &lo) && cmp(Le, &val, &hi);
                        if rule.operator == "between" { inside } else { !inside }
                    }
                    _ => false,
                }
            }
            "top10" => {
                let n = match v {
                    Value::Num(n) => *n,
                    _ => return false,
                };
                let st = self.cf_stats(s, bi, ri, block);
                let len = st.nums.len();
                if len == 0 {
                    return false;
                }
                let k = if rule.percent { ((len as f64 * rule.rank as f64 / 100.0).floor() as usize).max(1) } else { rule.rank as usize };
                let k = k.clamp(1, len);
                if rule.bottom { n <= st.nums[k - 1] } else { n >= st.nums[len - k] }
            }
            "aboveAverage" => {
                let n = match v {
                    Value::Num(n) => *n,
                    _ => return false,
                };
                let st = self.cf_stats(s, bi, ri, block);
                let m = st.mean;
                let d = rule.std_dev as f64 * st.sd;
                if rule.std_dev > 0 {
                    if rule.above { n >= m + d } else { n <= m - d }
                } else if rule.above {
                    if rule.equal { n >= m } else { n > m }
                } else if rule.equal {
                    n <= m
                } else {
                    n < m
                }
            }
            "duplicateValues" | "uniqueValues" => {
                if matches!(v, Value::Empty) {
                    return false;
                }
                let st = self.cf_stats(s, bi, ri, block);
                let n = st.counts.get(&key_of(v)).copied().unwrap_or(0);
                if rule.typ == "duplicateValues" { n > 1 } else { n == 1 }
            }
            _ => {
                if !rule.formulas.is_empty() {
                    return truthy(&self.cf_formula(s, block, rule, 0, r, c));
                }
                // rules written without their formula
                let t = match v {
                    Value::Empty => String::new(),
                    v => v.to_string(),
                };
                let tl = t.to_lowercase();
                let q = rule.text.to_lowercase();
                match rule.typ.as_str() {
                    "containsText" => tl.contains(&q),
                    "notContainsText" => !tl.contains(&q),
                    "beginsWith" => tl.starts_with(&q),
                    "endsWith" => tl.ends_with(&q),
                    "containsBlanks" => t.trim().is_empty(),
                    "notContainsBlanks" => !t.trim().is_empty(),
                    "containsErrors" => matches!(v, Value::Err(_)),
                    "notContainsErrors" => !matches!(v, Value::Err(_)),
                    _ => false,
                }
            }
        }
    }

    fn scale_color(&self, s: usize, bi: usize, ri: usize, block: &CfBlock, rule: &CfRule, v: &Value) -> Option<u32> {
        let n = match v {
            Value::Num(n) => *n,
            _ => return None,
        };
        if rule.cfvos.len() < 2 || rule.colors.len() < rule.cfvos.len() {
            return None;
        }
        let st = self.cf_stats(s, bi, ri, block);
        let k = rule.cfvos.len();
        let pts: Vec<f64> = rule.cfvos.iter().enumerate().map(|(i, x)| self.cfvo_value(s, block, &st, x, i == k - 1)).collect::<Option<Vec<_>>>()?;
        if n <= pts[0] {
            return Some(rule.colors[0]);
        }
        if n >= pts[k - 1] {
            return Some(rule.colors[k - 1]);
        }
        for i in 0..k - 1 {
            if n >= pts[i] && n <= pts[i + 1] {
                let t = if pts[i + 1] > pts[i] { (n - pts[i]) / (pts[i + 1] - pts[i]) } else { 0.0 };
                return Some(mix(rule.colors[i], rule.colors[i + 1], t));
            }
        }
        None
    }

    fn bar_of(&self, s: usize, bi: usize, ri: usize, block: &CfBlock, rule: &CfRule, v: &Value) -> Option<(f64, f64, Bar, bool, Option<f64>)> {
        let n = match v {
            Value::Num(n) => *n,
            _ => return None,
        };
        let bar = rule.bar.clone()?;
        let st = self.cf_stats(s, bi, ri, block);
        let lo = self.cfvo_value(s, block, &st, rule.cfvos.first()?, false)?;
        let hi = self.cfvo_value(s, block, &st, rule.cfvos.get(1)?, true)?;
        let (minl, maxl) = (bar.min_len / 100.0, bar.max_len / 100.0);
        // 2010 bars with negative values grow both ways from an axis
        if bar.neg_color.is_some() || bar.axis && lo < 0.0 {
            let lo = lo.min(0.0);
            let hi = hi.max(0.0);
            let span = (hi - lo).max(1e-300);
            let axis = -lo / span;
            let f = (n.clamp(lo, hi) - 0.0) / span;
            let (a, b) = if n >= 0.0 { (axis, axis + f.max(0.0)) } else { (axis + f, axis) };
            return Some((a, b, bar, n < 0.0, if lo < 0.0 { Some(axis) } else { None }));
        }
        let t = if hi > lo { ((n - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 1.0 };
        let len = minl + (maxl - minl) * t;
        Some((0.0, len, bar, false, None))
    }

    fn icon_of(&self, s: usize, bi: usize, ri: usize, block: &CfBlock, rule: &CfRule, v: &Value) -> Option<(String, u32)> {
        let n = match v {
            Value::Num(n) => *n,
            _ => return None,
        };
        let st = self.cf_stats(s, bi, ri, block);
        let k = rule.cfvos.len();
        if k == 0 {
            return None;
        }
        // the highest threshold the value reaches
        let mut idx = 0;
        for i in 1..k {
            let th = self.cfvo_value(s, block, &st, &rule.cfvos[i], false)?;
            let ok = if rule.cfvos[i].gte { n >= th } else { n > th };
            if ok {
                idx = i;
            }
        }
        if let Some(ic) = rule.icons.get(idx) {
            if ic.0 == "NoIcons" {
                return None;
            }
            return Some(ic.clone());
        }
        let i = if rule.reverse { k - 1 - idx } else { idx };
        Some((rule.icon_set.clone(), i as u32))
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let p = p.clamp(0.0, 1.0);
    let x = p * (sorted.len() - 1) as f64;
    let i = x.floor() as usize;
    let f = x - i as f64;
    if i + 1 < sorted.len() { sorted[i] + (sorted[i + 1] - sorted[i]) * f } else { sorted[i] }
}

fn key_of(v: &Value) -> String {
    match v {
        Value::Str(s) => format!("s{}", s.to_lowercase()),
        Value::Num(n) => format!("n{}", n),
        Value::Bool(b) => format!("b{}", b),
        Value::Err(e) => format!("e{:?}", e),
        _ => String::new(),
    }
}

pub fn mix(a: u32, b: u32, t: f64) -> u32 {
    let ch = |s: u32| -> u32 {
        let x = ((a >> s) & 255) as f64;
        let y = ((b >> s) & 255) as f64;
        (x + (y - x) * t).round().clamp(0.0, 255.0) as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// An expression with its relative references moved by (dr, dc).
pub fn shift_expr(e: &Expr, dr: i64, dc: i64) -> Expr {
    if dr == 0 && dc == 0 {
        return e.clone();
    }
    let w = |v: u32, d: i64, max: u32| ((v as i64 + d).rem_euclid(max as i64)) as u32;
    let s = |x: &Expr| shift_expr(x, dr, dc);
    match e {
        Expr::Ref(r) => {
            let mut r = (**r).clone();
            if r.kind != RefKind::Err {
                if r.kind != RefKind::Cols {
                    if !r.ra1 { r.r1 = w(r.r1, dr, MAX_ROWS) }
                    if !r.ra2 { r.r2 = w(r.r2, dr, MAX_ROWS) }
                }
                if r.kind != RefKind::Rows {
                    if !r.ca1 { r.c1 = w(r.c1, dc, MAX_COLS) }
                    if !r.ca2 { r.c2 = w(r.c2, dc, MAX_COLS) }
                }
            }
            Expr::Ref(Box::new(r))
        }
        Expr::Array(rows) => Expr::Array(rows.iter().map(|row| row.iter().map(s).collect()).collect()),
        Expr::Func(n, args) => Expr::Func(n.clone(), args.iter().map(s).collect()),
        Expr::Neg(x) => Expr::Neg(Box::new(s(x))),
        Expr::Plus(x) => Expr::Plus(Box::new(s(x))),
        Expr::Percent(x) => Expr::Percent(Box::new(s(x))),
        Expr::Bin(op, a, b) => Expr::Bin(*op, Box::new(s(a)), Box::new(s(b))),
        Expr::Range(a, b) => Expr::Range(Box::new(s(a)), Box::new(s(b))),
        Expr::Isect(a, b) => Expr::Isect(Box::new(s(a)), Box::new(s(b))),
        Expr::Union(v) => Expr::Union(v.iter().map(s).collect()),
        Expr::Spill(x) => Expr::Spill(Box::new(s(x))),
        Expr::At(x) => Expr::At(Box::new(s(x))),
        Expr::Call(f, args) => Expr::Call(Box::new(s(f)), args.iter().map(s).collect()),
        other => other.clone(),
    }
}

/// The workbook seen read-only, for evaluating rules while drawing (the values are
/// already calculated).
pub struct ViewCtx<'a> {
    pub b: &'a Book,
}

impl<'a> Ctx for ViewCtx<'a> {
    fn value(&mut self, sheet: usize, r: u32, c: u32) -> Value {
        match self.b.sheets.get(sheet) {
            Some(sh) => sh.value(r, c),
            None => Value::Err(XlErr::Ref),
        }
    }
    fn sheet_index(&self, name: &str) -> Option<usize> {
        self.b.sheet_index(name)
    }
    fn sheet_name(&self, i: usize) -> String {
        self.b.sheets.get(i).map(|s| s.name.clone()).unwrap_or_default()
    }
    fn sheet_count(&self) -> usize {
        self.b.sheets.len()
    }
    fn cells_in(&mut self, sheet: usize, rc: Rect) -> Vec<(u32, u32, Value)> {
        let sh = match self.b.sheets.get(sheet) {
            Some(s) => s,
            None => return vec![],
        };
        let mut out: Vec<(u32, u32, Value)> = sh.positions_in(rc).into_iter().filter_map(|(r, c)| {
            let cell = &sh.cells[&(r, c)];
            if cell.is_blank() { None } else { Some((r, c, cell.v.clone())) }
        }).collect();
        let mut extra = false;
        for ((r, c), v) in sh.ghosts.iter() {
            if rc.contains(*r, *c) && !sh.cells.get(&(*r, *c)).map(|x| !x.is_blank()).unwrap_or(false) {
                out.push((*r, *c, v.clone()));
                extra = true;
            }
        }
        if extra {
            out.sort_by_key(|(r, c, _)| (*r, *c));
        }
        out
    }
    fn extent(&self, sheet: usize) -> (u32, u32) {
        if sheet >= self.b.sheets.len() {
            return (0, 0);
        }
        self.b.data_extent_cached(sheet)
    }
    fn name(&self, name: &str, scope: usize) -> Option<(Rc<Expr>, Option<usize>)> {
        <Book as Ctx>::name(self.b, name, scope)
    }
    fn date1904(&self) -> bool {
        self.b.date1904
    }
    fn formula_text(&self, sheet: usize, r: u32, c: u32) -> Option<String> {
        <Book as Ctx>::formula_text(self.b, sheet, r, c)
    }
    fn number_format(&self, sheet: usize, r: u32, c: u32) -> String {
        Book::number_format(self.b, sheet, r, c)
    }
    fn row_hidden(&self, sheet: usize, r: u32) -> bool {
        self.b.sheets.get(sheet).map(|s| s.row_hidden(r)).unwrap_or(false)
    }
    fn cell_info(&self, sheet: usize, r: u32, c: u32, what: &str) -> Value {
        <Book as Ctx>::cell_info(self.b, sheet, r, c, what)
    }
    fn now(&self) -> f64 {
        <Book as Ctx>::now(self.b)
    }
    fn rand(&mut self) -> f64 {
        0.5
    }
    fn is_subtotal(&self, sheet: usize, r: u32, c: u32) -> bool {
        <Book as Ctx>::is_subtotal(self.b, sheet, r, c)
    }
    fn table_ref(&self, text: &str, sheet: usize, r: u32, c: u32) -> Option<(usize, Rect)> {
        crate::tables::resolve(&self.b.sheets, text, sheet, Some((r, c)))
    }
}

#[allow(dead_code)]
fn unused(_: Array) {}

// ---------------------------------------------------------------------- editing

/// A new rule, as the commands describe it.
#[derive(Clone, Debug, Default)]
pub struct RuleSpec {
    /// cellIs, expression, containsText, notContainsText, beginsWith, endsWith,
    /// containsBlanks, notContainsBlanks, containsErrors, notContainsErrors, timePeriod,
    /// top10, aboveAverage, duplicateValues, uniqueValues, colorScale, dataBar, iconSet
    pub typ: String,
    pub operator: String,
    /// as typed: "=A1>5", "10", "=$B$1" (relative to the top-left cell)
    pub values: Vec<String>,
    pub text: String,
    pub time_period: String,
    pub rank: u32,
    pub percent: bool,
    pub bottom: bool,
    pub above: bool,
    pub equal: bool,
    pub std_dev: u32,
    /// (type, value, colour) for colour scales; (type, value) for bars and icons
    pub points: Vec<(String, String, u32)>,
    pub bar_color: u32,
    pub gradient: bool,
    pub icon_set: String,
    pub reverse: bool,
    pub show_value: bool,
    pub stop: bool,
    /// the format: font colour, fill, bold, italic, border colour, underline, strike, number format
    pub font_color: Option<u32>,
    pub fill: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub border: Option<u32>,
    pub numfmt: Option<String>,
}

fn argb(c: u32) -> String {
    format!("FF{:06X}", c & 0xFFFFFF)
}

/// A typed value as a rule formula: "=A1" stays a formula, numbers stay, text is quoted.
pub fn value_formula(t: &str) -> String {
    let t = t.trim();
    if let Some(f) = t.strip_prefix('=') {
        return f.trim().to_string();
    }
    if t.parse::<f64>().is_ok() {
        return t.to_string();
    }
    format!("\"{}\"", t.replace('"', "\"\""))
}

fn time_formula(period: &str, cell: &str) -> String {
    match period {
        "yesterday" => format!("FLOOR({c},1)=TODAY()-1", c = cell),
        "today" => format!("FLOOR({c},1)=TODAY()", c = cell),
        "tomorrow" => format!("FLOOR({c},1)=TODAY()+1", c = cell),
        "last7Days" => format!("AND(TODAY()-FLOOR({c},1)<=6,FLOOR({c},1)<=TODAY())", c = cell),
        "lastWeek" => format!("AND(TODAY()-ROUNDDOWN({c},0)>=(WEEKDAY(TODAY())),TODAY()-ROUNDDOWN({c},0)<(WEEKDAY(TODAY())+7))", c = cell),
        "thisWeek" => format!("AND(TODAY()-ROUNDDOWN({c},0)<=WEEKDAY(TODAY())-1,ROUNDDOWN({c},0)-TODAY()<=7-WEEKDAY(TODAY()))", c = cell),
        "nextWeek" => format!("AND(ROUNDDOWN({c},0)-TODAY()>(7-WEEKDAY(TODAY())),ROUNDDOWN({c},0)-TODAY()<(15-WEEKDAY(TODAY())))", c = cell),
        "lastMonth" => format!("AND(MONTH({c})=MONTH(EDATE(TODAY(),0-1)),YEAR({c})=YEAR(EDATE(TODAY(),0-1)))", c = cell),
        "thisMonth" => format!("AND(MONTH({c})=MONTH(TODAY()),YEAR({c})=YEAR(TODAY()))", c = cell),
        _ => format!("AND(MONTH({c})=MONTH(EDATE(TODAY(),0+1)),YEAR({c})=YEAR(EDATE(TODAY(),0+1)))", c = cell),
    }
}

impl Book {
    /// Add a dxf to the styles part; its index.
    pub fn add_dxf(&mut self, sp: &RuleSpec) -> u32 {
        let mut d = String::from("<dxf>");
        if sp.font_color.is_some() || sp.bold || sp.italic || sp.underline || sp.strike {
            d.push_str("<font>");
            if sp.bold { d.push_str("<b/>") }
            if sp.italic { d.push_str("<i/>") }
            if sp.strike { d.push_str("<strike/>") }
            if sp.underline { d.push_str("<u/>") }
            if let Some(c) = sp.font_color {
                d.push_str(&format!("<color rgb=\"{}\"/>", argb(c)));
            }
            d.push_str("</font>");
        }
        if let Some(n) = &sp.numfmt {
            d.push_str(&format!("<numFmt numFmtId=\"{}\" formatCode=\"{}\"/>", 200 + self.styles.root.child("dxfs").map(|x| x.elems().count()).unwrap_or(0), xml::escape_attr(n)));
        }
        if let Some(c) = sp.fill {
            d.push_str(&format!("<fill><patternFill><bgColor rgb=\"{}\"/></patternFill></fill>", argb(c)));
        }
        if let Some(c) = sp.border {
            d.push_str("<border>");
            for side in ["left", "right", "top", "bottom"] {
                d.push_str(&format!("<{s} style=\"thin\"><color rgb=\"{c}\"/></{s}>", s = side, c = argb(c)));
            }
            d.push_str("</border>");
        }
        d.push_str("</dxf>");
        let el = xml::parse(&d).map(|(_, e)| e).unwrap_or_else(|_| Element::new("dxf"));
        let root = &mut self.styles.root;
        if root.child("dxfs").is_none() {
            root.insert_ordered(Element::with_attrs("dxfs", &[("count", "0")]), &crate::styles::STYLE_ORDER);
        }
        let list = root.child_mut("dxfs").unwrap();
        // the same format again uses the same dxf
        let s = el.to_string();
        if let Some(i) = list.elems().position(|e| e.to_string() == s) {
            return i as u32;
        }
        let tag = if list.name.contains(':') { format!("{}:dxf", list.name.split(':').next().unwrap()) } else { "dxf".to_string() };
        let mut el = el;
        el.name = tag;
        list.push(el);
        let n = list.elems().count();
        list.set_attr("count", &n.to_string());
        self.styles.dirty = true;
        (n - 1) as u32
    }

    /// Add a rule to the ranges; it gets the top priority.
    pub fn add_cf_rule(&mut self, s: usize, ranges: &[Rect], sp: &RuleSpec) -> Result<(), String> {
        if ranges.is_empty() {
            return Err("Select the cells first.".into());
        }
        let ranges: Vec<Rect> = ranges.iter().map(|r| self.sheets[s].clamp_rect(*r)).collect();
        let (r0, c0) = (ranges[0].r1, ranges[0].c1);
        let cell = crate::cellref::a1(r0, c0);
        let mut x = format!("<cfRule type=\"{}\"", xml::escape_attr(&sp.typ));
        let needs_dxf = !matches!(sp.typ.as_str(), "colorScale" | "dataBar" | "iconSet");
        if needs_dxf {
            let d = self.add_dxf(sp);
            x.push_str(&format!(" dxfId=\"{}\"", d));
        }
        x.push_str(" priority=\"1\"");
        if sp.stop {
            x.push_str(" stopIfTrue=\"1\"");
        }
        let mut formulas: Vec<String> = Vec::new();
        let q = sp.text.replace('"', "\"\"");
        match sp.typ.as_str() {
            "cellIs" => {
                x.push_str(&format!(" operator=\"{}\"", sp.operator));
                for v in &sp.values {
                    formulas.push(value_formula(v));
                }
                if formulas.is_empty() {
                    return Err("Enter a value for the rule.".into());
                }
            }
            "expression" => {
                let f = sp.values.first().map(|v| v.trim().trim_start_matches('=').to_string()).unwrap_or_default();
                if f.is_empty() || formula::check(&f).is_some() {
                    return Err("There's a problem with this formula.".into());
                }
                formulas.push(f);
            }
            "containsText" => {
                x.push_str(&format!(" operator=\"containsText\" text=\"{}\"", xml::escape_attr(&sp.text)));
                formulas.push(format!("NOT(ISERROR(SEARCH(\"{}\",{})))", q, cell));
            }
            "notContainsText" => {
                x.push_str(&format!(" operator=\"notContains\" text=\"{}\"", xml::escape_attr(&sp.text)));
                formulas.push(format!("ISERROR(SEARCH(\"{}\",{}))", q, cell));
            }
            "beginsWith" => {
                x.push_str(&format!(" operator=\"beginsWith\" text=\"{}\"", xml::escape_attr(&sp.text)));
                formulas.push(format!("LEFT({},LEN(\"{}\"))=\"{}\"", cell, q, q));
            }
            "endsWith" => {
                x.push_str(&format!(" operator=\"endsWith\" text=\"{}\"", xml::escape_attr(&sp.text)));
                formulas.push(format!("RIGHT({},LEN(\"{}\"))=\"{}\"", cell, q, q));
            }
            "containsBlanks" => formulas.push(format!("LEN(TRIM({}))=0", cell)),
            "notContainsBlanks" => formulas.push(format!("LEN(TRIM({}))>0", cell)),
            "containsErrors" => formulas.push(format!("ISERROR({})", cell)),
            "notContainsErrors" => formulas.push(format!("NOT(ISERROR({}))", cell)),
            "timePeriod" => {
                x.push_str(&format!(" timePeriod=\"{}\"", xml::escape_attr(&sp.time_period)));
                formulas.push(time_formula(&sp.time_period, &cell));
            }
            "top10" => {
                x.push_str(&format!(" rank=\"{}\"", sp.rank.max(1)));
                if sp.percent { x.push_str(" percent=\"1\"") }
                if sp.bottom { x.push_str(" bottom=\"1\"") }
            }
            "aboveAverage" => {
                if !sp.above { x.push_str(" aboveAverage=\"0\"") }
                if sp.equal { x.push_str(" equalAverage=\"1\"") }
                if sp.std_dev > 0 { x.push_str(&format!(" stdDev=\"{}\"", sp.std_dev)) }
            }
            "duplicateValues" | "uniqueValues" | "colorScale" | "dataBar" | "iconSet" => {}
            other => return Err(format!("unknown rule type {}", other)),
        }
        x.push('>');
        for f in &formulas {
            x.push_str(&format!("<formula>{}</formula>", xml::escape_text(&formula::to_file_text(f))));
        }
        let cfvo = |t: &str, v: &str, gte: bool| -> String {
            let mut s = format!("<cfvo type=\"{}\"", t);
            if !matches!(t, "min" | "max") {
                s.push_str(&format!(" val=\"{}\"", xml::escape_attr(v.trim_start_matches('='))));
            }
            if !gte {
                s.push_str(" gte=\"0\"");
            }
            s.push_str("/>");
            s
        };
        match sp.typ.as_str() {
            "colorScale" => {
                x.push_str("<colorScale>");
                for (t, v, _) in &sp.points {
                    x.push_str(&cfvo(t, v, true));
                }
                for (_, _, c) in &sp.points {
                    x.push_str(&format!("<color rgb=\"{}\"/>", argb(*c)));
                }
                x.push_str("</colorScale>");
            }
            "dataBar" => {
                x.push_str("<dataBar>");
                x.push_str(&cfvo("min", "", true));
                x.push_str(&cfvo("max", "", true));
                x.push_str(&format!("<color rgb=\"{}\"/></dataBar>", argb(sp.bar_color)));
            }
            "iconSet" => {
                x.push_str(&format!("<iconSet iconSet=\"{}\"", xml::escape_attr(&sp.icon_set)));
                if sp.reverse { x.push_str(" reverse=\"1\"") }
                if !sp.show_value { x.push_str(" showValue=\"0\"") }
                x.push('>');
                let n = icon_count(&sp.icon_set);
                if sp.points.len() == n {
                    for (t, v, gte) in &sp.points {
                        x.push_str(&cfvo(t, v, *gte != 0));
                    }
                } else {
                    for k in 0..n {
                        x.push_str(&cfvo("percent", &format!("{}", (k * 100 / n)), true));
                    }
                }
                x.push_str("</iconSet>");
            }
            _ => {}
        }
        x.push_str("</cfRule>");
        let sq: Vec<String> = ranges.iter().map(|r| r.a1()).collect();
        let piece = format!("<conditionalFormatting sqref=\"{}\">{}</conditionalFormatting>", sq.join(" "), x);
        // the others move down one
        let mut pieces = self.bump_priorities(s, 1);
        // after the last conditional format, or where the schema puts it
        let at = pieces.iter().rposition(|p| matches!(p, Piece::Raw(t, _) if t == "conditionalFormatting")).map(|i| i + 1).unwrap_or(pieces.len());
        pieces.insert(at, Piece::Raw("conditionalFormatting".into(), self.with_prefix(s, &piece)));
        self.begin("Conditional Formatting", s);
        self.record(Delta::Pieces(s, pieces));
        self.commit();
        Ok(())
    }

    /// The worksheet's element prefix ("x:" in prefixed files).
    pub(crate) fn with_prefix(&self, s: usize, xml_text: &str) -> String {
        let ro = self.sheets[s].root_open.trim_start_matches('<');
        let n = &ro[..ro.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(ro.len())];
        match n.find(':') {
            Some(k) => {
                let p = &n[..=k];
                xml_text.replace("</", "\u{1}").replace('<', &format!("<{}", p)).replace('\u{1}', &format!("</{}", p))
            }
            None => xml_text.to_string(),
        }
    }

    /// The pieces with every rule's priority raised by `by`.
    fn bump_priorities(&self, s: usize, by: i64) -> Vec<Piece> {
        self.map_rules(s, |_, _, e| {
            let p: i64 = e.attr("priority").and_then(|v| v.parse().ok()).unwrap_or(1);
            e.set_attr("priority", &(p + by).to_string());
            true
        })
    }

    /// Rewrite each cfRule element (f returns false to drop it); blocks left with no
    /// rules go.
    fn map_rules(&self, s: usize, mut f: impl FnMut(usize, usize, &mut Element) -> bool) -> Vec<Piece> {
        let mut out = Vec::new();
        for (i, p) in self.sheets[s].pieces.iter().enumerate() {
            match p {
                Piece::Raw(t, x) if t == "conditionalFormatting" => {
                    let mut e = match xml::parse(x) {
                        Ok((_, e)) => e,
                        Err(_) => {
                            out.push(p.clone());
                            continue;
                        }
                    };
                    let mut k = 0;
                    let mut kids = Vec::new();
                    for n in e.children.drain(..) {
                        match n {
                            Node::Elem(mut r) if r.local() == "cfRule" => {
                                if f(i, k, &mut r) {
                                    kids.push(Node::Elem(r));
                                }
                                k += 1;
                            }
                            other => kids.push(other),
                        }
                    }
                    e.children = kids;
                    if e.elems().any(|x| x.local() == "cfRule") {
                        out.push(Piece::Raw(t.clone(), e.to_string()));
                    }
                }
                other => out.push(other.clone()),
            }
        }
        out
    }

    /// Delete one rule (by its block's piece and its place in it).
    pub fn delete_cf_rule(&mut self, s: usize, piece: usize, k: usize) -> Result<(), String> {
        let pieces = self.map_rules(s, |i, j, _| !(i == piece && j == k));
        self.begin("Delete Rule", s);
        self.record(Delta::Pieces(s, pieces));
        self.commit();
        Ok(())
    }

    /// Clear rules from the ranges (None: the whole sheet).
    pub fn clear_cf(&mut self, s: usize, ranges: Option<&[Rect]>) -> Result<(), String> {
        let mut pieces = Vec::new();
        for p in self.sheets[s].pieces.iter() {
            match p {
                Piece::Raw(t, x) if t == "conditionalFormatting" => {
                    let rs = match ranges {
                        None => continue,
                        Some(r) => r,
                    };
                    let mut e = match xml::parse(x) {
                        Ok((_, e)) => e,
                        Err(_) => {
                            pieces.push(p.clone());
                            continue;
                        }
                    };
                    let mut left: Vec<Rect> = parse_sqref(e.attr("sqref").unwrap_or(""));
                    for cut in rs {
                        left = left.into_iter().flat_map(|a| subtract(a, cut)).collect();
                    }
                    if left.is_empty() {
                        continue;
                    }
                    let sq: Vec<String> = left.iter().map(|r| r.a1()).collect();
                    e.set_attr("sqref", &sq.join(" "));
                    pieces.push(Piece::Raw(t.clone(), e.to_string()));
                }
                Piece::Raw(t, x) if t == "extLst" && ranges.is_none() => {
                    // drop the 2010 conditional formats with the others
                    match xml::parse(x) {
                        Ok((_, mut e)) => {
                            e.children.retain(|n| !matches!(n, Node::Elem(ext) if ext.elems().any(|c| c.local() == "conditionalFormattings")));
                            if e.elems().next().is_some() {
                                pieces.push(Piece::Raw(t.clone(), e.to_string()));
                            }
                        }
                        Err(_) => pieces.push(p.clone()),
                    }
                }
                other => pieces.push(other.clone()),
            }
        }
        if pieces == self.sheets[s].pieces {
            return Ok(());
        }
        self.begin("Clear Rules", s);
        self.record(Delta::Pieces(s, pieces));
        self.commit();
        Ok(())
    }

    /// Change a rule: its priority order (up/down), stop-if-true, or where it applies.
    pub fn edit_cf_rule(&mut self, s: usize, piece: usize, k: usize, stop: Option<bool>, applies: Option<&str>, step: i32) -> Result<(), String> {
        let blocks = self.cf_blocks(s);
        let mut order: Vec<(i64, usize, usize)> = Vec::new();
        for b in blocks.iter() {
            if let Some(pi) = b.piece {
                for (j, r) in b.rules.iter().enumerate() {
                    order.push((r.priority, pi, j));
                }
            }
        }
        order.sort();
        let pos = order.iter().position(|x| x.1 == piece && x.2 == k).ok_or("no such rule")?;
        if step != 0 {
            let np = (pos as i64 + step as i64).clamp(0, order.len() as i64 - 1) as usize;
            let item = order.remove(pos);
            order.insert(np, item);
        }
        let prio: HashMap<(usize, usize), i64> = order.iter().enumerate().map(|(n, x)| ((x.1, x.2), n as i64 + 1)).collect();
        let mut pieces = self.map_rules(s, |i, j, e| {
            if let Some(p) = prio.get(&(i, j)) {
                e.set_attr("priority", &p.to_string());
            }
            if i == piece && j == k {
                if let Some(st) = stop {
                    if st { e.set_attr("stopIfTrue", "1") } else { e.remove_attr("stopIfTrue") }
                }
            }
            true
        });
        if let Some(a) = applies {
            let rects = parse_sqref(&a.replace(',', " ").replace('=', ""));
            if rects.is_empty() {
                return Err("The reference isn't valid.".into());
            }
            if let Some(Piece::Raw(_, x)) = pieces.get_mut(piece) {
                if let Ok((_, mut e)) = xml::parse(x) {
                    let sq: Vec<String> = rects.iter().map(|r| r.a1()).collect();
                    e.set_attr("sqref", &sq.join(" "));
                    *x = e.to_string();
                }
            }
        }
        if pieces == self.sheets[s].pieces {
            return Ok(());
        }
        self.begin("Manage Rules", s);
        self.record(Delta::Pieces(s, pieces));
        self.commit();
        Ok(())
    }
}

pub fn icon_count(set: &str) -> usize {
    set.chars().next().and_then(|c| c.to_digit(10)).unwrap_or(3) as usize
}

/// a minus b, as up to four rectangles.
pub fn subtract(a: Rect, b: &Rect) -> Vec<Rect> {
    let i = match a.intersect(b) {
        Some(i) => i,
        None => return vec![a],
    };
    let mut out = Vec::new();
    if i.r1 > a.r1 {
        out.push(Rect::new(a.r1, a.c1, i.r1 - 1, a.c2));
    }
    if i.r2 < a.r2 {
        out.push(Rect::new(i.r2 + 1, a.c1, a.r2, a.c2));
    }
    if i.c1 > a.c1 {
        out.push(Rect::new(i.r1, a.c1, i.r2, i.c1 - 1));
    }
    if i.c2 < a.c2 {
        out.push(Rect::new(i.r1, i.c2 + 1, i.r2, a.c2));
    }
    out
}

/// A rule as the Manage Rules list shows it.
#[derive(Clone, Debug)]
pub struct RuleInfo {
    pub piece: Option<usize>,
    pub index: usize,
    pub priority: i64,
    pub typ: String,
    pub desc: String,
    pub stop: bool,
    pub applies: String,
    pub dxf: Dxf,
}

fn op_text(op: &str) -> &'static str {
    match op {
        "lessThan" => "<",
        "lessThanOrEqual" => "<=",
        "equal" => "=",
        "notEqual" => "<>",
        "greaterThanOrEqual" => ">=",
        "greaterThan" => ">",
        "between" => "between",
        "notBetween" => "not between",
        _ => "?",
    }
}

impl Book {
    /// The rules on a sheet (those touching rc, when given), by priority.
    pub fn cf_rules(&self, s: usize, rc: Option<Rect>) -> Vec<RuleInfo> {
        let blocks = self.cf_blocks(s);
        let mut out = Vec::new();
        for b in blocks.iter() {
            if let Some(rc) = rc {
                if !b.ranges.iter().any(|x| x.intersects(&rc)) {
                    continue;
                }
            }
            let applies = b.ranges.iter().map(|r| {
                let a = r.a1();
                a.split(':').map(|p| {
                    let i = p.find(|c: char| c.is_ascii_digit()).unwrap_or(p.len());
                    format!("${}${}", &p[..i], &p[i..])
                }).collect::<Vec<_>>().join(":")
            }).collect::<Vec<_>>().join(",");
            for (k, r) in b.rules.iter().enumerate() {
                let f = |i: usize| r.formulas.get(i).map(|x| format!("={}", x.0)).unwrap_or_default();
                let desc = match r.typ.as_str() {
                    "cellIs" => {
                        if r.operator.contains("etween") {
                            format!("Cell Value {} {} and {}", op_text(&r.operator), f(0), f(1))
                        } else {
                            format!("Cell Value {} {}", op_text(&r.operator), f(0))
                        }
                    }
                    "expression" => format!("Formula: {}", f(0)),
                    "containsText" => format!("Cell Value contains '{}'", r.text),
                    "notContainsText" => format!("Cell Value does not contain '{}'", r.text),
                    "beginsWith" => format!("Cell Value begins with '{}'", r.text),
                    "endsWith" => format!("Cell Value ends with '{}'", r.text),
                    "containsBlanks" => "Blank values".into(),
                    "notContainsBlanks" => "No blanks".into(),
                    "containsErrors" => "Errors".into(),
                    "notContainsErrors" => "No errors".into(),
                    "timePeriod" => format!("Dates occurring: {}", r.time_period),
                    "top10" => format!("{} {}{}", if r.bottom { "Bottom" } else { "Top" }, r.rank, if r.percent { "%" } else { "" }),
                    "aboveAverage" => {
                        if r.std_dev > 0 {
                            format!("{} std dev {} average", r.std_dev, if r.above { "above" } else { "below" })
                        } else {
                            format!("{} Average", if r.above { if r.equal { "Equal or Above" } else { "Above" } } else if r.equal { "Equal or Below" } else { "Below" })
                        }
                    }
                    "duplicateValues" => "Duplicate Values".into(),
                    "uniqueValues" => "Unique Values".into(),
                    "colorScale" => "Graded Color Scale".into(),
                    "dataBar" => "Data Bar".into(),
                    "iconSet" => "Icon Set".into(),
                    t => t.to_string(),
                };
                out.push(RuleInfo {
                    piece: b.piece,
                    index: k,
                    priority: r.priority,
                    typ: r.typ.clone(),
                    desc,
                    stop: r.stop,
                    applies: format!("={}", applies),
                    dxf: r.dxf.map(|d| self.dxf(d)).unwrap_or_default(),
                });
            }
        }
        out.sort_by_key(|x| x.priority);
        out
    }

    /// Conditional formats of src copied onto dst (same size or tiled), as paste does.
    pub fn copy_cf(&mut self, s: usize, src: Rect, dst_s: usize, dst: Rect) {
        if s != dst_s {
            return;
        }
        let blocks = self.cf_blocks(s);
        let mut add: Vec<String> = Vec::new();
        let dr = dst.r1 as i64 - src.r1 as i64;
        let dc = dst.c1 as i64 - src.c1 as i64;
        for b in blocks.iter() {
            let pi = match b.piece {
                Some(p) => p,
                None => continue,
            };
            let parts: Vec<Rect> = b.ranges.iter().filter_map(|r| r.intersect(&src)).collect();
            if parts.is_empty() {
                continue;
            }
            let raw = match &self.sheets[s].pieces[pi] {
                Piece::Raw(_, x) => x.clone(),
                _ => continue,
            };
            let mut e = match xml::parse(&raw) {
                Ok((_, e)) => e,
                Err(_) => continue,
            };
            // the formulas stay relative to the block's first cell, which moves with the copy
            let (r0, c0) = b.origin();
            let nr0 = parts[0].r1 as i64 + dr;
            let nc0 = parts[0].c1 as i64 + dc;
            let how = formula::Adjust::Shift(nr0 - r0 as i64, nc0 - c0 as i64);
            fn walk(e: &mut Element, how: &formula::Adjust) {
                for n in e.children.iter_mut() {
                    if let Node::Elem(c) = n {
                        if c.local() == "formula" {
                            let t = c.text();
                            c.children = vec![Node::Text(formula::adjust(&t, how))];
                        } else {
                            walk(c, how);
                        }
                    }
                }
            }
            walk(&mut e, &how);
            let mut sq = Vec::new();
            for p in &parts {
                let r1 = p.r1 as i64 + dr;
                let c1 = p.c1 as i64 + dc;
                if r1 < 0 || c1 < 0 {
                    continue;
                }
                let n = Rect::new(r1 as u32, c1 as u32, (p.r2 as i64 + dr) as u32, (p.c2 as i64 + dc) as u32);
                sq.push(n.a1());
            }
            if sq.is_empty() {
                continue;
            }
            e.set_attr("sqref", &sq.join(" "));
            // a copied 2010 rule loses its extension link (it is drawn as a 2007 rule)
            for n in e.children.iter_mut() {
                if let Node::Elem(r) = n {
                    r.children.retain(|x| !matches!(x, Node::Elem(c) if c.local() == "extLst"));
                }
            }
            add.push(e.to_string());
        }
        if add.is_empty() {
            return;
        }
        let mut pieces = self.sheets[s].pieces.clone();
        let at = pieces.iter().rposition(|p| matches!(p, Piece::Raw(t, _) if t == "conditionalFormatting")).map(|i| i + 1).unwrap_or(pieces.len());
        for (k, x) in add.into_iter().enumerate() {
            pieces.insert(at + k, Piece::Raw("conditionalFormatting".into(), x));
        }
        self.record(Delta::Pieces(s, pieces));
    }
}
