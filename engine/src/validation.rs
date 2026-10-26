//! Data validation: the rules a sheet keeps in its <dataValidations> element (and the 2010
//! extension list), checking typed entries against them, the in-cell drop-down list, circling
//! invalid data, and the Data Validation dialog's edits. Unchanged rules round-trip exactly.

use std::rc::Rc;

use crate::book::{Book, Delta};
use crate::calc::{Ctx, Eval};
use crate::cellref::{parse_range, Rect};
use crate::cf::{shift_expr, subtract, ViewCtx};
use crate::formula::{self, Expr};
use crate::input::{interpret, Typed};
use crate::sheet::Piece;
use crate::value::Value;
use crate::xml::{self, Element, Node};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DvRule {
    /// none, whole, decimal, list, date, time, textLength, custom
    pub kind: String,
    /// between, notBetween, equal, notEqual, greaterThan, lessThan, greaterThanOrEqual, lessThanOrEqual
    pub op: String,
    pub allow_blank: bool,
    /// the in-cell drop-down arrow (the file's showDropDown means the opposite)
    pub dropdown: bool,
    pub show_input: bool,
    pub show_error: bool,
    /// stop, warning, information
    pub style: String,
    pub error_title: String,
    pub error: String,
    pub prompt_title: String,
    pub prompt: String,
    pub f1: String,
    pub f2: String,
    pub ranges: Vec<Rect>,
    /// read from the 2010 extension list (lists drawn from other sheets)
    pub x14: bool,
}

impl DvRule {
    pub fn contains(&self, r: u32, c: u32) -> bool {
        self.ranges.iter().any(|x| x.contains(r, c))
    }
    pub fn origin(&self) -> (u32, u32) {
        self.ranges.iter().map(|x| (x.r1, x.c1)).min().unwrap_or((0, 0))
    }
    /// The settings the dialog edits (everything but where it applies).
    pub fn same_settings(&self, o: &DvRule) -> bool {
        let mut a = self.clone();
        let mut b = o.clone();
        a.ranges.clear();
        b.ranges.clear();
        a.x14 = false;
        b.x14 = false;
        a == b
    }
}

fn flag(e: &Element, name: &str, default: bool) -> bool {
    match e.attr(name) {
        Some(v) => v == "1" || v.eq_ignore_ascii_case("true"),
        None => default,
    }
}

fn sqref(s: &str) -> Vec<Rect> {
    s.split_whitespace().filter_map(|t| parse_range(&t.replace('$', ""))).collect()
}

fn read_rule(e: &Element, x14: bool) -> DvRule {
    let formula = |name: &str| -> String {
        match e.elems().find(|c| c.local() == name) {
            Some(f) if x14 => f.elems().find(|g| g.local() == "f").map(|g| g.text()).unwrap_or_else(|| f.text()),
            Some(f) => f.text(),
            None => String::new(),
        }
    };
    let ranges = if x14 {
        sqref(&e.elems().find(|c| c.local() == "sqref").map(|c| c.text()).unwrap_or_default())
    } else {
        sqref(e.attr("sqref").unwrap_or(""))
    };
    DvRule {
        kind: e.attr("type").unwrap_or("none").to_string(),
        op: e.attr("operator").unwrap_or("between").to_string(),
        allow_blank: flag(e, "allowBlank", false),
        dropdown: !flag(e, "showDropDown", false),
        show_input: flag(e, "showInputMessage", false),
        show_error: flag(e, "showErrorMessage", false),
        style: e.attr("errorStyle").unwrap_or("stop").to_string(),
        error_title: e.attr("errorTitle").map(xml::unescape_ooxml).unwrap_or_default(),
        error: e.attr("error").map(xml::unescape_ooxml).unwrap_or_default(),
        prompt_title: e.attr("promptTitle").map(xml::unescape_ooxml).unwrap_or_default(),
        prompt: e.attr("prompt").map(xml::unescape_ooxml).unwrap_or_default(),
        f1: formula("formula1"),
        f2: formula("formula2"),
        ranges,
        x14,
    }
}

fn fingerprint(pieces: &[Piece]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in pieces {
        if let Piece::Raw(t, x) = p {
            if t == "dataValidations" || t == "extLst" {
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

/// The workbook with one cell holding the value being typed (custom rules test the new value).
struct TypedCtx<'a> {
    base: ViewCtx<'a>,
    at: (usize, u32, u32),
    v: Value,
}

impl<'a> Ctx for TypedCtx<'a> {
    fn value(&mut self, sheet: usize, r: u32, c: u32) -> Value {
        if (sheet, r, c) == self.at {
            return self.v.clone();
        }
        self.base.value(sheet, r, c)
    }
    fn sheet_index(&self, name: &str) -> Option<usize> {
        self.base.sheet_index(name)
    }
    fn sheet_name(&self, i: usize) -> String {
        self.base.sheet_name(i)
    }
    fn sheet_count(&self) -> usize {
        self.base.sheet_count()
    }
    fn cells_in(&mut self, sheet: usize, rc: Rect) -> Vec<(u32, u32, Value)> {
        let mut out = self.base.cells_in(sheet, rc);
        let (s, r, c) = self.at;
        if s == sheet && rc.contains(r, c) {
            out.retain(|x| (x.0, x.1) != (r, c));
            if !matches!(self.v, Value::Empty) {
                out.push((r, c, self.v.clone()));
                out.sort_by_key(|x| (x.0, x.1));
            }
        }
        out
    }
    fn extent(&self, sheet: usize) -> (u32, u32) {
        let (r0, c0) = self.base.extent(sheet);
        if sheet == self.at.0 {
            (r0.max(self.at.1), c0.max(self.at.2))
        } else {
            (r0, c0)
        }
    }
    fn name(&self, name: &str, scope: usize) -> Option<(Rc<Expr>, Option<usize>)> {
        self.base.name(name, scope)
    }
    fn date1904(&self) -> bool {
        self.base.date1904()
    }
    fn formula_text(&self, sheet: usize, r: u32, c: u32) -> Option<String> {
        self.base.formula_text(sheet, r, c)
    }
    fn number_format(&self, sheet: usize, r: u32, c: u32) -> String {
        self.base.number_format(sheet, r, c)
    }
    fn row_hidden(&self, sheet: usize, r: u32) -> bool {
        self.base.row_hidden(sheet, r)
    }
    fn cell_info(&self, sheet: usize, r: u32, c: u32, what: &str) -> Value {
        self.base.cell_info(sheet, r, c, what)
    }
    fn now(&self) -> f64 {
        self.base.now()
    }
    fn rand(&mut self) -> f64 {
        0.5
    }
    fn is_subtotal(&self, sheet: usize, r: u32, c: u32) -> bool {
        self.base.is_subtotal(sheet, r, c)
    }
    fn table_ref(&self, text: &str, sheet: usize, r: u32, c: u32) -> Option<(usize, Rect)> {
        self.base.table_ref(text, sheet, r, c)
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Num(n) => *n != 0.0,
        Value::Arr(a) => truthy(a.get(0, 0)),
        _ => false,
    }
}

fn compare(v: f64, op: &str, a: f64, b: Option<f64>) -> bool {
    let b = b.unwrap_or(a);
    match op {
        "notBetween" => v < a.min(b) || v > a.max(b),
        "equal" => v == a,
        "notEqual" => v != a,
        "greaterThan" => v > a,
        "lessThan" => v < a,
        "greaterThanOrEqual" => v >= a,
        "lessThanOrEqual" => v <= a,
        _ => v >= a.min(b) && v <= a.max(b),
    }
}

/// A rule as the Data Validation dialog gives it.
#[derive(Clone, Debug, Default)]
pub struct DvSpec {
    pub kind: String,
    pub op: String,
    pub f1: String,
    pub f2: String,
    pub allow_blank: bool,
    pub dropdown: bool,
    pub show_input: bool,
    pub prompt_title: String,
    pub prompt: String,
    pub show_error: bool,
    pub style: String,
    pub error_title: String,
    pub error: String,
}

impl Book {
    /// The sheet's validation rules, in file order (the first that covers a cell applies).
    pub fn dv_rules(&self, s: usize) -> Rc<Vec<DvRule>> {
        let sh = &self.sheets[s];
        let fp = fingerprint(&sh.pieces);
        if let Some((k, v)) = self.dv_cache.borrow().get(&s) {
            if *k == fp {
                return v.clone();
            }
        }
        let mut rules = Vec::new();
        for p in sh.pieces.iter() {
            if let Piece::Raw(t, x) = p {
                let e = match xml::parse(x) {
                    Ok((_, e)) => e,
                    Err(_) => continue,
                };
                if t == "dataValidations" {
                    for d in e.elems().filter(|x| x.local() == "dataValidation") {
                        let r = read_rule(d, false);
                        if !r.ranges.is_empty() {
                            rules.push(r);
                        }
                    }
                } else if t == "extLst" {
                    for ex in e.elems() {
                        for dvs in ex.elems().filter(|x| x.local() == "dataValidations") {
                            for d in dvs.elems().filter(|x| x.local() == "dataValidation") {
                                let r = read_rule(d, true);
                                if !r.ranges.is_empty() {
                                    rules.push(r);
                                }
                            }
                        }
                    }
                }
            }
        }
        let rc = Rc::new(rules);
        self.dv_cache.borrow_mut().insert(s, (fp, rc.clone()));
        rc
    }

    /// The rule that applies to a cell.
    pub fn dv_at(&self, s: usize, r: u32, c: u32) -> Option<DvRule> {
        self.dv_rules(s).iter().find(|x| x.contains(r, c) && x.kind != "none").cloned()
    }

    fn dv_eval(&self, s: usize, rule: &DvRule, text: &str, r: u32, c: u32, typed: Option<&Value>) -> Value {
        let t = text.trim().trim_start_matches('=');
        if t.is_empty() {
            return Value::Empty;
        }
        let ex = match formula::parse(t) {
            Ok(e) => e,
            Err(_) => return Value::Err(crate::value::XlErr::Value),
        };
        let (r0, c0) = rule.origin();
        let shifted = shift_expr(&ex, r as i64 - r0 as i64, c as i64 - c0 as i64);
        let v = match typed {
            Some(v) => {
                let mut ctx = TypedCtx { base: ViewCtx { b: self }, at: (s, r, c), v: v.clone() };
                let mut ev = Eval::new(&mut ctx, s, r, c, true);
                ev.eval_top(&shifted)
            }
            None => {
                let mut ctx = ViewCtx { b: self };
                let mut ev = Eval::new(&mut ctx, s, r, c, true);
                ev.eval_top(&shifted)
            }
        };
        match v {
            Value::Arr(a) if typed.is_none() => Value::Arr(a),
            Value::Arr(a) => a.get(0, 0).clone(),
            v => v,
        }
    }

    fn dv_num(&self, s: usize, rule: &DvRule, text: &str, r: u32, c: u32) -> Option<f64> {
        if text.trim().is_empty() {
            return None;
        }
        if let Ok(n) = text.trim().parse::<f64>() {
            return Some(n);
        }
        match self.dv_eval(s, rule, text, r, c, None) {
            Value::Num(n) => Some(n),
            Value::Arr(a) => match a.get(0, 0) {
                Value::Num(n) => Some(*n),
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether the active cell shows a list drop-down arrow.
    pub fn dv_dropdown_at(&self, s: usize, r: u32, c: u32) -> bool {
        if s >= self.sheets.len() {
            return false;
        }
        if !self.sheets[s].pieces.iter().any(|p| matches!(p, Piece::Raw(t, x) if t == "dataValidations" || (t == "extLst" && x.contains("dataValidation")))) {
            return false;
        }
        matches!(self.dv_at(s, r, c), Some(x) if x.kind == "list" && x.dropdown)
    }

    /// The choices of a list rule, as they show.
    pub fn dv_list(&self, s: usize, r: u32, c: u32) -> Vec<String> {
        let rule = match self.dv_at(s, r, c) {
            Some(x) if x.kind == "list" => x,
            _ => return vec![],
        };
        self.dv_items(s, &rule, r, c)
    }

    fn dv_items(&self, s: usize, rule: &DvRule, r: u32, c: u32) -> Vec<String> {
        let f = rule.f1.trim();
        if let Some(inner) = f.strip_prefix('"').and_then(|x| x.strip_suffix('"')) {
            return inner.replace("\"\"", "\"").split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
        }
        let t = f.trim_start_matches('=');
        let ex = match formula::parse(t) {
            Ok(e) => e,
            Err(_) => return vec![],
        };
        let (r0, c0) = rule.origin();
        let shifted = shift_expr(&ex, r as i64 - r0 as i64, c as i64 - c0 as i64);
        let mut areas = Vec::new();
        crate::recalc::collect_areas(&shifted, &self.sheets, &self.names, s, &mut areas, 0);
        let mut out = Vec::new();
        if !areas.is_empty() {
            for (si, rc) in areas {
                if si >= self.sheets.len() {
                    continue;
                }
                let rc = self.sheets[si].clamp_rect(rc);
                if rc.count() > 32768 {
                    continue;
                }
                for rr in rc.r1..=rc.r2 {
                    for cc in rc.c1..=rc.c2 {
                        let t = self.display(si, rr, cc);
                        if !t.is_empty() {
                            out.push(t);
                        }
                    }
                }
            }
            return out;
        }
        match self.dv_eval(s, rule, f, r, c, None) {
            Value::Arr(a) => {
                for i in 0..a.rows {
                    for j in 0..a.cols {
                        let v = a.get(i, j);
                        if !matches!(v, Value::Empty) {
                            out.push(v.to_string());
                        }
                    }
                }
            }
            Value::Empty => {}
            v => out.push(v.to_string()),
        }
        out
    }

    /// Whether a value passes a rule at a cell.
    fn dv_passes(&self, s: usize, rule: &DvRule, r: u32, c: u32, v: &Value, typed_text: &str) -> bool {
        let blank = match v {
            Value::Empty => true,
            Value::Str(t) => t.is_empty(),
            _ => false,
        };
        if blank {
            return true;
        }
        match rule.kind.as_str() {
            "whole" | "decimal" | "date" | "time" => {
                let n = match v {
                    Value::Num(n) => *n,
                    _ => return false,
                };
                if rule.kind == "whole" && n.fract() != 0.0 {
                    return false;
                }
                let a = match self.dv_num(s, rule, &rule.f1, r, c) {
                    Some(a) => a,
                    None => return false,
                };
                let two = matches!(rule.op.as_str(), "between" | "notBetween");
                let b = if two { self.dv_num(s, rule, &rule.f2, r, c) } else { None };
                if two && b.is_none() {
                    return false;
                }
                compare(n, &rule.op, a, b)
            }
            "textLength" => {
                let len = match v {
                    Value::Str(t) => t.chars().count(),
                    _ => typed_text.trim_start_matches('\'').chars().count(),
                } as f64;
                let a = match self.dv_num(s, rule, &rule.f1, r, c) {
                    Some(a) => a,
                    None => return false,
                };
                let b = if matches!(rule.op.as_str(), "between" | "notBetween") { self.dv_num(s, rule, &rule.f2, r, c) } else { None };
                compare(len, &rule.op, a, b)
            }
            "list" => {
                let items = self.dv_items(s, rule, r, c);
                match v {
                    Value::Num(n) => items.iter().any(|it| {
                        it.parse::<f64>().map(|x| x == *n).unwrap_or(false)
                            || crate::input::parse_number_text(it, self.date1904).map(|(x, _)| x == *n).unwrap_or(false)
                            || it.trim() == typed_text.trim()
                    }),
                    Value::Bool(b) => items.iter().any(|it| it.eq_ignore_ascii_case(if *b { "TRUE" } else { "FALSE" })),
                    other => {
                        let t = other.to_string();
                        items.iter().any(|it| it.to_lowercase() == t.to_lowercase())
                    }
                }
            }
            "custom" => truthy(&self.dv_eval(s, rule, &rule.f1, r, c, Some(v))),
            _ => true,
        }
    }

    /// Check a typed entry: the rule it breaks, if it breaks one whose alert is on.
    pub fn dv_check(&self, s: usize, r: u32, c: u32, text: &str) -> Option<DvRule> {
        let rule = self.dv_at(s, r, c)?;
        if !rule.show_error {
            return None;
        }
        let code = {
            let xf = self.sheets[s].get(r, c).map(|x| x.s).unwrap_or_else(|| self.sheets[s].style_at(r, c));
            self.styles.xf_numfmt(xf)
        };
        let v = match interpret(text, self.date1904, code == "@") {
            Typed::Empty | Typed::Formula(_) => return None,
            Typed::Value(v, _) => v,
            Typed::QuotedText(t) => Value::string(t),
        };
        if self.dv_passes(s, &rule, r, c, &v, text) {
            None
        } else {
            Some(rule)
        }
    }

    /// Cells holding values their rule doesn't allow (Circle Invalid Data), at most `limit`.
    pub fn dv_invalid(&self, s: usize, limit: usize) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        let sh = &self.sheets[s];
        for rule in self.dv_rules(s).iter() {
            if rule.kind == "none" {
                continue;
            }
            for rc in &rule.ranges {
                let rc = sh.clamp_rect(*rc);
                for (r, c) in sh.positions_in(rc) {
                    if self.dv_at(s, r, c).as_ref() != Some(rule) {
                        continue;
                    }
                    let v = sh.value(r, c);
                    if matches!(v, Value::Empty) {
                        continue;
                    }
                    let t = self.display(s, r, c);
                    if !self.dv_passes(s, rule, r, c, &v, &t) && !out.contains(&(r, c)) {
                        out.push((r, c));
                        if out.len() >= limit {
                            return out;
                        }
                    }
                }
            }
        }
        out
    }

    /// The pieces with the ranges taken out of every rule (rules left covering nothing go).
    fn dv_pieces_without(&self, s: usize, cut: &[Rect]) -> Vec<Piece> {
        let cut_rect = |rs: Vec<Rect>| -> Vec<Rect> {
            let mut left = rs;
            for c in cut {
                left = left.into_iter().flat_map(|a| subtract(a, c)).collect();
            }
            left
        };
        let mut out = Vec::new();
        for p in self.sheets[s].pieces.iter() {
            match p {
                Piece::Raw(t, x) if t == "dataValidations" => {
                    let mut e = match xml::parse(x) {
                        Ok((_, e)) => e,
                        Err(_) => {
                            out.push(p.clone());
                            continue;
                        }
                    };
                    let mut kids = Vec::new();
                    let mut n = 0;
                    for k in e.children.drain(..) {
                        match k {
                            Node::Elem(mut d) if d.local() == "dataValidation" => {
                                let left = cut_rect(sqref(d.attr("sqref").unwrap_or("")));
                                if !left.is_empty() {
                                    let sq: Vec<String> = left.iter().map(|r| r.a1()).collect();
                                    d.set_attr("sqref", &sq.join(" "));
                                    kids.push(Node::Elem(d));
                                    n += 1;
                                }
                            }
                            other => kids.push(other),
                        }
                    }
                    e.children = kids;
                    if n > 0 {
                        e.set_attr("count", &n.to_string());
                        out.push(Piece::Raw(t.clone(), e.to_string()));
                    }
                }
                Piece::Raw(t, x) if t == "extLst" && x.contains("dataValidation") => {
                    let mut e = match xml::parse(x) {
                        Ok((_, e)) => e,
                        Err(_) => {
                            out.push(p.clone());
                            continue;
                        }
                    };
                    for ex in e.elems_mut() {
                        for dvs in ex.elems_mut().filter(|x| x.local() == "dataValidations") {
                            let mut kids = Vec::new();
                            let mut n = 0;
                            for k in dvs.children.drain(..) {
                                match k {
                                    Node::Elem(mut d) if d.local() == "dataValidation" => {
                                        let sq_el = d.elems().find(|c| c.local() == "sqref").map(|c| c.text()).unwrap_or_default();
                                        let left = cut_rect(sqref(&sq_el));
                                        if !left.is_empty() {
                                            let sq: Vec<String> = left.iter().map(|r| r.a1()).collect();
                                            if let Some(se) = d.elems_mut().find(|c| c.local() == "sqref") {
                                                se.children = vec![Node::Text(sq.join(" "))];
                                            }
                                            kids.push(Node::Elem(d));
                                            n += 1;
                                        }
                                    }
                                    other => kids.push(other),
                                }
                            }
                            dvs.children = kids;
                            dvs.set_attr("count", &n.to_string());
                        }
                    }
                    // an extension left with no validations goes; the list too when it is empty
                    e.children.retain(|k| match k {
                        Node::Elem(ex) => !ex.elems().any(|d| d.local() == "dataValidations" && !d.elems().any(|x| x.local() == "dataValidation")),
                        _ => true,
                    });
                    if e.elems().next().is_some() {
                        out.push(Piece::Raw(t.clone(), e.to_string()));
                    }
                }
                other => out.push(other.clone()),
            }
        }
        out
    }

    fn dv_rule_xml(&self, sp: &DvSpec, ranges: &[Rect]) -> String {
        let mut x = String::from("<dataValidation");
        if sp.kind != "none" {
            x.push_str(&format!(" type=\"{}\"", xml::escape_attr(&sp.kind)));
        }
        if sp.style != "stop" && !sp.style.is_empty() {
            x.push_str(&format!(" errorStyle=\"{}\"", xml::escape_attr(&sp.style)));
        }
        let two_or_more = matches!(sp.kind.as_str(), "whole" | "decimal" | "date" | "time" | "textLength");
        if two_or_more && sp.op != "between" && !sp.op.is_empty() {
            x.push_str(&format!(" operator=\"{}\"", xml::escape_attr(&sp.op)));
        }
        if sp.allow_blank {
            x.push_str(" allowBlank=\"1\"");
        }
        if sp.kind == "list" && !sp.dropdown {
            x.push_str(" showDropDown=\"1\"");
        }
        if sp.show_input {
            x.push_str(" showInputMessage=\"1\"");
        }
        if sp.show_error {
            x.push_str(" showErrorMessage=\"1\"");
        }
        for (k, v) in [("errorTitle", &sp.error_title), ("error", &sp.error), ("promptTitle", &sp.prompt_title), ("prompt", &sp.prompt)] {
            if !v.is_empty() {
                x.push_str(&format!(" {}=\"{}\"", k, xml::escape_attr(v)));
            }
        }
        let sq: Vec<String> = ranges.iter().map(|r| r.a1()).collect();
        x.push_str(&format!(" sqref=\"{}\">", sq.join(" ")));
        let f = |t: &str| t.trim().trim_start_matches('=').to_string();
        if sp.kind != "none" && !sp.f1.trim().is_empty() {
            x.push_str(&format!("<formula1>{}</formula1>", xml::escape_text(&f(&sp.f1))));
        }
        if matches!(sp.op.as_str(), "between" | "notBetween") && two_or_more && !sp.f2.trim().is_empty() {
            x.push_str(&format!("<formula2>{}</formula2>", xml::escape_text(&f(&sp.f2))));
        }
        x.push_str("</dataValidation>");
        x
    }

    /// Check a dialog's rule before it is applied.
    pub fn dv_validate_spec(&self, s: usize, sp: &DvSpec, at: (u32, u32)) -> Result<(), String> {
        let needs_two = matches!(sp.kind.as_str(), "whole" | "decimal" | "date" | "time" | "textLength") && matches!(sp.op.as_str(), "between" | "notBetween");
        let check = |t: &str, what: &str| -> Result<(), String> {
            let t = t.trim();
            if t.is_empty() {
                return Err(format!("You must enter a value in the {} box.", what));
            }
            let body = t.trim_start_matches('=');
            if t.starts_with('=') && formula::check(body).is_some() {
                return Err("There's a problem with this formula.".into());
            }
            if !t.starts_with('=') && sp.kind != "custom" && crate::input::parse_number_text(t, self.date1904).is_none() {
                return Err(match sp.kind.as_str() {
                    "date" => format!("The {} you entered is not a valid date.", what),
                    "time" => format!("The {} you entered is not a valid time.", what),
                    _ => format!("The {} you entered must be a number.", what),
                });
            }
            Ok(())
        };
        match sp.kind.as_str() {
            "none" => Ok(()),
            "list" => {
                let t = sp.f1.trim();
                if t.is_empty() {
                    return Err("You must enter a value in the Source box.".into());
                }
                if t.starts_with('=') {
                    let rule = DvRule { kind: "list".into(), f1: t.trim_start_matches('=').to_string(), ranges: vec![Rect::new(at.0, at.1, at.0, at.1)], ..Default::default() };
                    if formula::check(t.trim_start_matches('=')).is_some() || self.dv_items(s, &rule, at.0, at.1).is_empty() && formula::parse(t.trim_start_matches('=')).is_err() {
                        return Err("The Source currently evaluates to an error.".into());
                    }
                }
                Ok(())
            }
            "custom" => check(&sp.f1, "Formula"),
            _ => {
                let one = if needs_two { "Minimum" } else { "Value" };
                check(&sp.f1, one)?;
                if needs_two {
                    check(&sp.f2, "Maximum")?;
                }
                Ok(())
            }
        }
    }

    /// Apply a rule to the ranges (replacing what they had); kind "none" clears them.
    pub fn dv_set(&mut self, s: usize, ranges: &[Rect], sp: &DvSpec) -> Result<(), String> {
        if ranges.is_empty() {
            return Err("Select the cells first.".into());
        }
        let ranges: Vec<Rect> = ranges.iter().map(|r| self.sheets[s].clamp_rect(*r)).collect();
        self.dv_validate_spec(s, sp, (ranges[0].r1, ranges[0].c1))?;
        let mut pieces = self.dv_pieces_without(s, &ranges);
        // a list whose source is a typed list keeps it quoted, as the file wants it
        let mut sp = sp.clone();
        if sp.kind == "list" && !sp.f1.trim().starts_with('=') && !sp.f1.trim().starts_with('"') {
            sp.f1 = format!("\"{}\"", sp.f1.trim().replace('"', "\"\""));
        }
        // typed bounds (12, 1/5/2026, 9:30, 50%) are stored as their numbers
        if matches!(sp.kind.as_str(), "whole" | "decimal" | "date" | "time" | "textLength") {
            for f in [&mut sp.f1, &mut sp.f2] {
                let t = f.trim().to_string();
                if !t.is_empty() && !t.starts_with('=') {
                    if let Some((n, _)) = crate::input::parse_number_text(&t, self.date1904) {
                        *f = crate::numfmt::num_to_text(n);
                    }
                }
            }
        }
        // "Any value" with an input message is still kept, for the message
        let keep = sp.kind != "none" || (sp.show_input && !(sp.prompt.is_empty() && sp.prompt_title.is_empty()));
        if keep {
            let rule = self.dv_rule_xml(&sp, &ranges);
            match pieces.iter().position(|p| matches!(p, Piece::Raw(t, _) if t == "dataValidations")) {
                Some(i) => {
                    if let Piece::Raw(t, x) = &pieces[i] {
                        if let Ok((_, mut e)) = xml::parse(x) {
                            let rx = self.with_prefix(s, &rule);
                            if let Ok((_, d)) = xml::parse(&rx) {
                                e.push(d);
                            }
                            let n = e.elems().filter(|x| x.local() == "dataValidation").count();
                            e.set_attr("count", &n.to_string());
                            pieces[i] = Piece::Raw(t.clone(), e.to_string());
                        }
                    }
                }
                None => {
                    let x = self.with_prefix(s, &format!("<dataValidations count=\"1\">{}</dataValidations>", rule));
                    pieces.push(Piece::Raw("dataValidations".into(), x));
                }
            }
        }
        if pieces == self.sheets[s].pieces {
            return Ok(());
        }
        self.begin("Data Validation", s);
        self.record(Delta::Pieces(s, pieces));
        self.commit();
        Ok(())
    }

    /// Clear All: no validation on the ranges.
    pub fn dv_clear(&mut self, s: usize, ranges: &[Rect]) -> Result<(), String> {
        let ranges: Vec<Rect> = ranges.iter().map(|r| self.sheets[s].clamp_rect(*r)).collect();
        let pieces = self.dv_pieces_without(s, &ranges);
        if pieces == self.sheets[s].pieces {
            return Ok(());
        }
        self.begin("Clear Validation", s);
        self.record(Delta::Pieces(s, pieces));
        self.commit();
        Ok(())
    }

    /// What the dialog shows for a selection: the active cell's rule, whether other cells of
    /// the selection have different rules, and every cell with the same settings.
    /// `mixed`: "" (one rule or none), "erase" (different rules meet), "extend" (some cells lack the active cell's rule).
    pub fn dv_for_selection(&self, s: usize, active: (u32, u32), sel: &[Rect]) -> (Option<DvRule>, &'static str, Vec<Rect>) {
        let rule = self.dv_at(s, active.0, active.1);
        let rules = self.dv_rules(s);
        let mut mixed = "";
        for x in rules.iter() {
            if x.kind == "none" {
                continue;
            }
            let touches = sel.iter().any(|a| x.ranges.iter().any(|b| a.intersects(b)));
            if touches && rule.as_ref().map(|r| !r.same_settings(x)).unwrap_or(true) {
                mixed = "erase";
            }
        }
        if rule.is_some() && mixed.is_empty() {
            // cells of the selection without a rule mix too
            let covered = |r: u32, c: u32| rules.iter().any(|x| x.kind != "none" && x.contains(r, c));
            'outer: for a in sel {
                let a = self.sheets[s].clamp_rect(*a);
                if a.count() > 100_000 {
                    continue;
                }
                for r in a.r1..=a.r2 {
                    for c in a.c1..=a.c2 {
                        if !covered(r, c) {
                            mixed = "extend";
                            break 'outer;
                        }
                    }
                }
            }
        }
        let same: Vec<Rect> = match &rule {
            Some(r) => rules.iter().filter(|x| x.same_settings(r)).flat_map(|x| x.ranges.clone()).collect(),
            None => vec![],
        };
        (rule, mixed, same)
    }

    /// Copy the validation of `src` to `dst` (paste, fill), cell for cell, repeating `src`.
    pub fn copy_validation(&mut self, s: usize, src: Rect, dst_s: usize, dst: Rect) {
        let rules = self.dv_rules(s);
        let mut adds: Vec<(DvRule, Vec<Rect>)> = Vec::new();
        let (h, w) = (src.r2 - src.r1 + 1, src.c2 - src.c1 + 1);
        for rule in rules.iter() {
            let mut out = Vec::new();
            for rc in &rule.ranges {
                if let Some(part) = rc.intersect(&src) {
                    let mut r0 = dst.r1;
                    while r0 <= dst.r2 {
                        let mut c0 = dst.c1;
                        while c0 <= dst.c2 {
                            let dr = r0 as i64 - src.r1 as i64;
                            let dc = c0 as i64 - src.c1 as i64;
                            let moved = Rect::new((part.r1 as i64 + dr) as u32, (part.c1 as i64 + dc) as u32, (part.r2 as i64 + dr) as u32, (part.c2 as i64 + dc) as u32);
                            if let Some(x) = moved.intersect(&dst) {
                                out.push(x);
                            }
                            c0 += w;
                        }
                        r0 += h;
                    }
                }
            }
            if !out.is_empty() {
                adds.push((rule.clone(), out));
            }
        }
        let mut pieces = self.dv_pieces_without(dst_s, &[dst]);
        let mut extra = String::new();
        for (rule, rs) in &adds {
            // the formulas mean the same at the new place as at the old (relative parts move)
            let (r0, c0) = rule.origin();
            let (n0, m0) = rs.iter().map(|x| (x.r1, x.c1)).min().unwrap_or((r0, c0));
            let (dr, dc) = (dst.r1 as i64 - src.r1 as i64, dst.c1 as i64 - src.c1 as i64);
            let (sr, sc) = (n0 as i64 - dr - r0 as i64, m0 as i64 - dc - c0 as i64);
            let shift = |f: &str| -> String {
                let t = f.trim();
                if t.starts_with('"') || t.is_empty() {
                    return f.to_string();
                }
                // the rule's formula at the new origin's source cell, then moved with the paste
                crate::formula::adjust(&crate::formula::adjust(t.trim_start_matches('='), &crate::formula::Adjust::Shift(sr, sc)), &crate::formula::Adjust::Shift(dr, dc))
            };
            let sp = DvSpec {
                kind: rule.kind.clone(),
                op: rule.op.clone(),
                f1: shift(&rule.f1),
                f2: shift(&rule.f2),
                allow_blank: rule.allow_blank,
                dropdown: rule.dropdown,
                show_input: rule.show_input,
                prompt_title: rule.prompt_title.clone(),
                prompt: rule.prompt.clone(),
                show_error: rule.show_error,
                style: rule.style.clone(),
                error_title: rule.error_title.clone(),
                error: rule.error.clone(),
            };
            extra.push_str(&self.dv_rule_xml(&sp, rs));
        }
        if !extra.is_empty() {
            match pieces.iter().position(|p| matches!(p, Piece::Raw(t, _) if t == "dataValidations")) {
                Some(i) => {
                    if let Piece::Raw(t, x) = &pieces[i] {
                        if let Ok((_, mut e)) = xml::parse(x) {
                            if let Ok((_, wrap)) = xml::parse(&self.with_prefix(dst_s, &format!("<w>{}</w>", extra))) {
                                for k in wrap.children {
                                    if let Node::Elem(d) = k {
                                        e.push(d);
                                    }
                                }
                            }
                            let n = e.elems().filter(|x| x.local() == "dataValidation").count();
                            e.set_attr("count", &n.to_string());
                            pieces[i] = Piece::Raw(t.clone(), e.to_string());
                        }
                    }
                }
                None => {
                    let n = adds.len();
                    pieces.push(Piece::Raw("dataValidations".into(), self.with_prefix(dst_s, &format!("<dataValidations count=\"{}\">{}</dataValidations>", n, extra))));
                }
            }
        }
        if pieces != self.sheets[dst_s].pieces {
            self.record(Delta::Pieces(dst_s, pieces));
        }
    }
}

