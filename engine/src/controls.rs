//! Form controls (S3-07): check boxes, option buttons, combo and list boxes, spin buttons, scroll
//! bars, buttons, group boxes and labels over the cells, linked to cells. Kept as the desktop
//! spreadsheet keeps them in its legacy drawing (VML x:ClientData), which every version reads.
use crate::book::{Book, Delta};
use crate::drawing::{Marker, EMU};

/// The kinds, as x:ClientData ObjectType names them.
pub const KINDS: [&str; 9] = ["Button", "Checkbox", "Drop", "GBox", "Label", "List", "Radio", "Scroll", "Spin"];

#[derive(Clone, Debug, PartialEq, Default)]
pub struct FormControl {
    pub kind: String,
    /// top-left and bottom-right, cell + offset in pixels
    pub from: Marker,
    pub to: Marker,
    pub text: String,
    /// the cell it sets ($B$2 or Sheet2!$B$2)
    pub link: Option<String>,
    /// the input range of a list or combo box
    pub range: Option<String>,
    /// check box: 0 unchecked, 1 checked, 2 mixed
    pub checked: u8,
    /// spin / scroll: value, bounds and steps; list/combo: the selected item (1-based)
    pub val: i64,
    pub min: i64,
    pub max: i64,
    pub inc: i64,
    pub page: i64,
    pub horiz: bool,
    pub drop_lines: u32,
    /// the macro a button would run (kept, never run)
    pub macro_name: Option<String>,
    /// the first option button of a group
    pub first_button: bool,
    pub name: String,
}

impl FormControl {
    pub fn new(kind: &str) -> FormControl {
        FormControl {
            kind: kind.to_string(),
            min: 0,
            max: 100,
            inc: 1,
            page: 10,
            drop_lines: 8,
            ..Default::default()
        }
    }
}

fn tag_text<'a>(shape: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<x:{}>", name);
    let i = shape.find(&open)? + open.len();
    let j = shape[i..].find(&format!("</x:{}>", name))? + i;
    Some(&shape[i..j])
}

fn has_tag(shape: &str, name: &str) -> bool {
    shape.contains(&format!("<x:{}/>", name)) || shape.contains(&format!("<x:{}>", name))
}

/// The form controls of a VML drawing, in order.
pub fn read_vml(v: &str) -> Vec<FormControl> {
    let mut out = Vec::new();
    let mut rest = v;
    while let Some(i) = rest.find("<v:shape ") {
        let after = &rest[i..];
        let end = after.find("</v:shape>").map(|e| e + 10).unwrap_or(after.len());
        let shape = &after[..end];
        rest = &after[end..];
        let kind = match shape.find("ObjectType=\"") {
            Some(k) => {
                let t = &shape[k + 12..];
                t[..t.find('"').unwrap_or(0)].to_string()
            }
            None => continue,
        };
        if !KINDS.contains(&kind.as_str()) {
            continue;
        }
        let mut c = FormControl::new(&kind);
        if let Some(a) = tag_text(shape, "Anchor") {
            let n: Vec<i64> = a.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            if n.len() == 8 {
                c.from = Marker { col: n[0] as u32, col_off: n[1], row: n[2] as u32, row_off: n[3] };
                c.to = Marker { col: n[4] as u32, col_off: n[5], row: n[6] as u32, row_off: n[7] };
            }
        }
        // the text: the textbox's characters
        if let Some(tb) = shape.find("<v:textbox") {
            let t = &shape[tb..];
            if let Some(e) = t.find("</v:textbox>") {
                let inner = &t[..e];
                let mut txt = String::new();
                let mut in_tag = false;
                for ch in inner.chars() {
                    match ch {
                        '<' => in_tag = true,
                        '>' => in_tag = false,
                        _ if !in_tag => txt.push(ch),
                        _ => {}
                    }
                }
                c.text = crate::xml::unescape(txt.trim());
            }
        }
        c.link = tag_text(shape, "FmlaLink").map(|s| crate::xml::unescape(s.trim()));
        c.range = tag_text(shape, "FmlaRange").map(|s| crate::xml::unescape(s.trim()));
        c.macro_name = tag_text(shape, "FmlaMacro").map(|s| crate::xml::unescape(s.trim()));
        c.checked = tag_text(shape, "Checked").and_then(|s| s.trim().parse().ok()).unwrap_or(0);
        let num = |n: &str, d: i64| tag_text(shape, n).and_then(|s| s.trim().parse().ok()).unwrap_or(d);
        c.val = num("Val", 0);
        c.min = num("Min", 0);
        c.max = num("Max", 100);
        c.inc = num("Inc", 1);
        c.page = num("Page", 10);
        c.drop_lines = num("DropLines", 8) as u32;
        c.horiz = has_tag(shape, "Horiz");
        c.first_button = has_tag(shape, "FirstButton");
        if let Some(k) = shape.find("o:title=\"") {
            let t = &shape[k + 9..];
            c.name = crate::xml::unescape(&t[..t.find('"').unwrap_or(0)]);
        }
        out.push(c);
    }
    out
}

pub fn is_control_shape(shape: &str) -> bool {
    KINDS.iter().any(|k| shape.contains(&format!("ObjectType=\"{}\"", k)))
}

pub const SHAPETYPE: &str = "<v:shapetype id=\"_x0000_t201\" coordsize=\"21600,21600\" o:spt=\"201\" path=\"m,l,21600r21600,l21600,xe\"><v:stroke joinstyle=\"miter\"/><v:path shadowok=\"f\" o:extrusionok=\"f\" strokeok=\"f\" fillok=\"f\" o:connecttype=\"rect\"/><o:lock v:ext=\"edit\" shapetype=\"t\"/></v:shapetype>";

/// One control as a VML shape.
pub fn write_shape(book: &Book, s: usize, c: &FormControl, id: usize, z: usize) -> String {
    let esc = crate::xml::escape_text;
    let x = book.col_x(s, c.from.col) + c.from.col_off as f64;
    let y = book.row_y(s, c.from.row) + c.from.row_off as f64;
    let x2 = book.col_x(s, c.to.col) + c.to.col_off as f64;
    let y2 = book.row_y(s, c.to.row) + c.to.row_off as f64;
    let pt = |px: f64| px * 72.0 / 96.0;
    let mut data = format!("<x:ClientData ObjectType=\"{}\"><x:Anchor>{}, {}, {}, {}, {}, {}, {}, {}</x:Anchor>",
        c.kind, c.from.col, c.from.col_off, c.from.row, c.from.row_off, c.to.col, c.to.col_off, c.to.row, c.to.row_off);
    if c.kind == "Button" || c.kind == "GBox" || c.kind == "Label" {
        data.push_str("<x:PrintObject>False</x:PrintObject>");
    }
    data.push_str("<x:AutoFill>False</x:AutoFill>");
    if matches!(c.kind.as_str(), "Checkbox" | "Radio" | "Label" | "GBox" | "Button") {
        data.push_str("<x:AutoLine>False</x:AutoLine>");
    }
    if c.kind == "Button" {
        data.push_str("<x:TextHAlign>Center</x:TextHAlign>");
    }
    if matches!(c.kind.as_str(), "Checkbox" | "Radio" | "Button") {
        data.push_str("<x:TextVAlign>Center</x:TextVAlign>");
    }
    if let Some(m) = &c.macro_name {
        data.push_str(&format!("<x:FmlaMacro>{}</x:FmlaMacro>", esc(m)));
    }
    if let Some(l) = &c.link {
        data.push_str(&format!("<x:FmlaLink>{}</x:FmlaLink>", esc(l)));
    }
    if let Some(r) = &c.range {
        data.push_str(&format!("<x:FmlaRange>{}</x:FmlaRange>", esc(r)));
    }
    match c.kind.as_str() {
        "Checkbox" | "Radio" => {
            if c.checked > 0 {
                data.push_str(&format!("<x:Checked>{}</x:Checked>", c.checked));
            }
            if c.kind == "Radio" && c.first_button {
                data.push_str("<x:FirstButton/>");
            }
            data.push_str("<x:NoThreeD/>");
        }
        "Spin" | "Scroll" => {
            data.push_str(&format!("<x:Val>{}</x:Val><x:Min>{}</x:Min><x:Max>{}</x:Max><x:Inc>{}</x:Inc><x:Page>{}</x:Page>",
                c.val, c.min, c.max, c.inc, c.page));
            if c.horiz {
                data.push_str("<x:Horiz/>");
            }
            data.push_str("<x:Dx>16</x:Dx>");
        }
        "Drop" | "List" => {
            if c.val > 0 {
                data.push_str(&format!("<x:Sel>{}</x:Sel>", c.val));
            }
            data.push_str(&format!("<x:Val>0</x:Val><x:Min>0</x:Min><x:Max>0</x:Max><x:Dx>16</x:Dx>"));
            if c.kind == "Drop" {
                data.push_str(&format!("<x:DropStyle>Combo</x:DropStyle><x:DropLines>{}</x:DropLines>", c.drop_lines));
            } else {
                data.push_str("<x:SelType>Single</x:SelType>");
            }
        }
        _ => {}
    }
    data.push_str("</x:ClientData>");
    let text = if matches!(c.kind.as_str(), "Checkbox" | "Radio" | "Button" | "GBox" | "Label") {
        format!("<v:textbox style=\"mso-direction-alt:auto\" o:singleclick=\"f\"><div style=\"text-align:{}\"><font face=\"Segoe UI\" size=\"160\" color=\"#000000\">{}</font></div></v:textbox>",
            if c.kind == "Button" { "center" } else { "left" }, esc(&c.text))
    } else {
        String::new()
    };
    let (fill, stroke) = match c.kind.as_str() {
        "Button" => ("fillcolor=\"buttonFace [67]\" strokecolor=\"windowText [64]\" o:insetmode=\"auto\"", ""),
        _ => ("filled=\"f\" fillcolor=\"window [65]\" stroked=\"f\" strokecolor=\"windowText [64]\" o:insetmode=\"auto\"", ""),
    };
    format!("<v:shape id=\"_x0000_s{}\" type=\"#_x0000_t201\"{} style=\"position:absolute;margin-left:{:.2}pt;margin-top:{:.2}pt;width:{:.2}pt;height:{:.2}pt;z-index:{};mso-wrap-style:tight\" {}{}><v:path shadowok=\"t\" strokeok=\"t\" fillok=\"t\"/><o:lock v:ext=\"edit\" rotation=\"t\"/>{}{}</v:shape>",
        id, if c.name.is_empty() { String::new() } else { format!(" o:title=\"{}\"", crate::xml::escape_attr(&c.name)) },
        pt(x), pt(y), pt((x2 - x).max(1.0)), pt((y2 - y).max(1.0)), z, fill, stroke, text, data)
}

impl Book {
    /// The sheet-pixel box (x, y, w, h) of a control.
    pub fn control_rect(&self, s: usize, c: &FormControl) -> (f64, f64, f64, f64) {
        let x = self.col_x(s, c.from.col) + c.from.col_off as f64;
        let y = self.row_y(s, c.from.row) + c.from.row_off as f64;
        let x2 = self.col_x(s, c.to.col) + c.to.col_off as f64;
        let y2 = self.row_y(s, c.to.row) + c.to.row_off as f64;
        (x, y, (x2 - x).max(1.0), (y2 - y).max(1.0))
    }

    fn px_cell(&self, s: usize, x: f64, y: f64) -> Marker {
        let m = self.px_marker(s, x, y);
        Marker { col: m.col, col_off: (m.col_off as f64 / EMU).round() as i64, row: m.row, row_off: (m.row_off as f64 / EMU).round() as i64 }
    }

    /// Put a control at a sheet-pixel box.
    pub fn place_control(&self, s: usize, c: &mut FormControl, x: f64, y: f64, w: f64, h: f64) {
        c.from = self.px_cell(s, x.max(0.0), y.max(0.0));
        c.to = self.px_cell(s, (x + w).max(1.0), (y + h).max(1.0));
    }

    pub fn set_controls(&mut self, s: usize, list: Vec<FormControl>, label: &str) {
        self.begin(label, s);
        self.record(Delta::Controls(s, list));
        self.commit();
    }

    /// A name of its own: "Check Box 3".
    pub fn control_name(&self, s: usize, kind: &str) -> String {
        let base = match kind {
            "Button" => "Button", "Checkbox" => "Check Box", "Drop" => "Drop Down", "GBox" => "Group Box",
            "Label" => "Label", "List" => "List Box", "Radio" => "Option Button", "Scroll" => "Scroll Bar", _ => "Spinner",
        };
        let n = self.sheets[s].controls.len() + self.sheets[s].objects.len() + 1;
        let mut k = n;
        loop {
            let name = format!("{} {}", base, k);
            if !self.sheets[s].controls.iter().any(|c| c.name == name) {
                return name;
            }
            k += 1;
        }
    }
}

pub const T_BAG: &str = "http://schemas.microsoft.com/office/2022/11/relationships/FeaturePropertyBag";
pub const CT_BAG: &str = "application/vnd.ms-excel.featurepropertybag+xml";
const BAG: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n<FeaturePropertyBags xmlns=\"http://schemas.microsoft.com/office/spreadsheetml/2022/featurepropertybag\"><bag type=\"Checkbox\"/><bag type=\"XFControls\"><bagId k=\"CellControl\">0</bagId></bag><bag type=\"XFComplement\"><bagId k=\"XFControls\">1</bagId></bag><bag type=\"XFComplements\" extRef=\"XFComplementsMapperExtRef\"><a k=\"MappedFeaturePropertyBags\"><bagId>2</bagId></a></bag></FeaturePropertyBags>";

impl Book {
    /// The feature property bag part that says what a cell check box is (newer versions need it
    /// to show the check boxes).
    pub fn ensure_feature_bag(&mut self) {
        self.ensure_wb_parts();
        let has = self.file.wb_rels.as_ref().map(|(_, r)| r.elems().any(|e| e.attr("Type") == Some(T_BAG))).unwrap_or(false);
        if has {
            return;
        }
        let dir = match self.file.wb_part.rfind('/') {
            Some(i) => self.file.wb_part[..i].to_string(),
            None => "xl".into(),
        };
        let part = format!("{}/featurePropertyBag/featurePropertyBag.xml", dir);
        self.file.parts.retain(|(n, _)| *n != part);
        self.file.parts.push((part.clone(), BAG.as_bytes().to_vec()));
        let wb_part = self.file.wb_part.clone();
        if let Some((_, r)) = self.file.wb_rels.as_mut() {
            let used: Vec<String> = r.elems().filter_map(|e| e.attr("Id").map(|s| s.to_string())).collect();
            let mut i = used.len() + 1;
            let id = loop {
                let id = format!("rId{}", i);
                if !used.contains(&id) {
                    break id;
                }
                i += 1;
            };
            r.push(crate::xml::Element::with_attrs("Relationship", &[("Id", &id), ("Type", T_BAG), ("Target", &crate::xlsx::relative(&wb_part, &part))]));
        }
        if let Some((_, ct)) = self.file.content_types.as_mut() {
            crate::xlsx::set_override(ct, &part, CT_BAG);
        }
    }

    /// Insert > Checkbox: the cells show check boxes; empty cells become FALSE. on=false removes
    /// them (the values stay).
    pub fn cell_checkboxes(&mut self, s: usize, ranges: &[crate::cellref::Rect], on: bool) -> Result<(), String> {
        self.check_cell(s, 0, 0)?;
        let label = if on { "Insert Checkbox" } else { "Clear Checkbox" };
        self.begin_group(label, s);
        let ch = crate::styles::Change { checkbox: Some(on), ..Default::default() };
        let r = self.apply_change(s, ranges, &ch, label);
        if r.is_ok() && on {
            for rg in ranges {
                let rg = self.clip_used(s, *rg);
                for row in rg.r1..=rg.r2 {
                    for col in rg.c1..=rg.c2 {
                        if matches!(self.sheets[s].value(row, col), crate::value::Value::Empty) && self.sheets[s].get(row, col).map(|c| c.f.is_none()).unwrap_or(true) {
                            let _ = self.set_input(s, row, col, "FALSE");
                        }
                    }
                }
            }
            self.ensure_feature_bag();
        }
        self.end_group();
        r
    }

    /// A range limited to sensible bounds (a whole column: down to the data, at least row 1).
    fn clip_used(&self, s: usize, rg: crate::cellref::Rect) -> crate::cellref::Rect {
        let mut rg = rg;
        if rg.count() > 100_000 {
            let (er, ec) = self.sheets[s].data_extent().unwrap_or((rg.r1, rg.c1));
            rg.r2 = rg.r2.min(er.max(rg.r1));
            rg.c2 = rg.c2.min(ec.max(rg.c1));
        }
        rg
    }

    /// Whether a cell shows a check box.
    pub fn is_checkbox(&self, s: usize, r: u32, c: u32) -> bool {
        self.styles.xf(self.sheets[s].style_at(r, c)).checkbox
    }

    /// Click or Space on a check-box cell: TRUE <-> FALSE (anything else becomes TRUE).
    pub fn toggle_checkbox(&mut self, s: usize, r: u32, c: u32) -> Result<(), String> {
        let now = matches!(self.sheets[s].value(r, c), crate::value::Value::Bool(true));
        if self.sheets[s].get(r, c).map(|x| x.f.is_some()).unwrap_or(false) {
            return Err("The check box shows a formula's result; change the formula instead.".into());
        }
        self.set_input(s, r, c, if now { "FALSE" } else { "TRUE" })
    }
}

impl Book {
    /// The cells a control's link or input range names (no sheet: the control's sheet).
    pub fn control_ref(&self, s: usize, text: &str) -> Option<(usize, crate::cellref::Rect)> {
        let t = text.trim();
        let t = t.strip_prefix('=').unwrap_or(t);
        if t.is_empty() {
            return None;
        }
        let (sheet, _, rest) = crate::formula::split_sheet(t);
        let si = match sheet {
            Some(n) => self.sheet_index(&n)?,
            None => s,
        };
        if let Some(rc) = crate::cellref::parse_range(&rest.replace('$', "")) {
            return Some((si, self.sheets[si].clamp_rect(rc)));
        }
        // a defined name
        let f = self.names.iter().find(|n| n.name.eq_ignore_ascii_case(rest))?.text.clone();
        let f = f.trim_start_matches('=').to_string();
        if f == text {
            return None;
        }
        let (sheet, _, rest) = crate::formula::split_sheet(&f);
        let si = match sheet {
            Some(n) => self.sheet_index(&n)?,
            None => s,
        };
        let rc = crate::cellref::parse_range(&rest.replace('$', ""))?;
        Some((si, self.sheets[si].clamp_rect(rc)))
    }

    /// The items a list or combo box shows (its input range, read down).
    pub fn control_items(&self, s: usize, c: &FormControl) -> Vec<String> {
        let (si, rc) = match c.range.as_deref().and_then(|r| self.control_ref(s, r)) {
            Some(x) => x,
            None => return vec![],
        };
        let mut out = Vec::new();
        for r in rc.r1..=rc.r2.min(rc.r1 + 32766) {
            for col in rc.c1..=rc.c1 {
                out.push(self.display(si, r, col));
            }
        }
        out
    }

    /// The option buttons that act together with control i (those in the same group box, or
    /// all those in no group box).
    pub fn radio_group(&self, s: usize, i: usize) -> Vec<usize> {
        let list = &self.sheets[s].controls;
        let boxes: Vec<(f64, f64, f64, f64)> = list.iter().filter(|c| c.kind == "GBox").map(|c| self.control_rect(s, c)).collect();
        let box_of = |c: &FormControl| -> Option<usize> {
            let (x, y, w, h) = self.control_rect(s, c);
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            boxes.iter().position(|b| cx >= b.0 && cx <= b.0 + b.2 && cy >= b.1 && cy <= b.1 + b.3)
        };
        let me = box_of(&list[i]);
        list.iter().enumerate().filter(|(_, c)| c.kind == "Radio" && box_of(c) == me).map(|(k, _)| k).collect()
    }

    fn link_value(&self, s: usize, link: &Option<String>) -> Option<crate::value::Value> {
        let (si, rc) = self.control_ref(s, link.as_deref()?)?;
        Some(self.sheets[si].value(rc.r1, rc.c1))
    }

    /// What control i shows: a check box 0/1/2 (unchecked, checked, mixed), an option button
    /// 0/1, a list or combo the selected item (1-based, 0 none), a spinner or scroll bar its value.
    pub fn control_value(&self, s: usize, i: usize) -> i64 {
        use crate::value::Value;
        let c = &self.sheets[s].controls[i];
        match c.kind.as_str() {
            "Checkbox" => match self.link_value(s, &c.link) {
                Some(Value::Bool(b)) => b as i64,
                Some(Value::Num(n)) => (n != 0.0) as i64,
                Some(Value::Err(_)) => 2,
                Some(Value::Empty) | None => c.checked as i64,
                _ => c.checked as i64,
            },
            "Radio" => {
                let group = self.radio_group(s, i);
                let link = group.iter().find_map(|k| self.sheets[s].controls[*k].link.clone());
                match self.link_value(s, &link) {
                    Some(Value::Num(n)) => (group.iter().position(|k| *k == i).map(|p| p as f64 + 1.0) == Some(n)) as i64,
                    _ => (c.checked > 0) as i64,
                }
            }
            "Drop" | "List" | "Spin" | "Scroll" => match self.link_value(s, &c.link) {
                Some(Value::Num(n)) => n as i64,
                _ => c.val,
            },
            _ => 0,
        }
    }

    /// Set what control i shows (as control_value counts it), and its linked cell; one undo step.
    pub fn set_control_value(&mut self, s: usize, i: usize, v: i64) -> Result<(), String> {
        let mut list = self.sheets[s].controls.clone();
        let kind = list[i].kind.clone();
        let mut links: Vec<(Option<String>, String)> = Vec::new();
        match kind.as_str() {
            "Checkbox" => {
                list[i].checked = v.clamp(0, 2) as u8;
                links.push((list[i].link.clone(), match v { 0 => "FALSE".into(), 1 => "TRUE".into(), _ => "=NA()".into() }));
            }
            "Radio" => {
                let group = self.radio_group(s, i);
                for (p, k) in group.iter().enumerate() {
                    list[*k].checked = (*k == i) as u8;
                    if *k == i {
                        let link = group.iter().find_map(|k| list[*k].link.clone());
                        links.push((link, (p + 1).to_string()));
                    }
                }
            }
            "Spin" | "Scroll" => {
                let c = &mut list[i];
                let (lo, hi) = (c.min.min(c.max), c.max.max(c.min));
                c.val = v.clamp(lo, hi);
                links.push((c.link.clone(), c.val.to_string()));
            }
            "Drop" | "List" => {
                list[i].val = v.max(0);
                links.push((list[i].link.clone(), v.max(0).to_string()));
            }
            _ => return Ok(()),
        }
        let label = "Change Control";
        self.begin_group(label, s);
        self.record(Delta::Controls(s, list));
        let mut err = None;
        for (link, text) in links {
            if let Some((si, rc)) = link.as_deref().and_then(|l| self.control_ref(s, l)) {
                if let Err(e) = self.set_input(si, rc.r1, rc.c1, &text) {
                    err = Some(e);
                }
            }
        }
        self.end_group();
        match err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
