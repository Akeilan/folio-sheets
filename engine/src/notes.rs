//! Notes (the classic cell comments) and threaded comments: the model, the commands, and
//! the file parts (comments, the VML drawing that shows notes, threaded comments, persons).

use crate::book::{Book, Delta};
use crate::cellref::{a1, parse_cell, Rect};
use crate::xml::{self, Element, Node};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Note {
    pub author: String,
    pub text: String,
    pub visible: bool,
    /// box size in pixels
    pub w: f64,
    pub h: f64,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Entry {
    pub id: String,
    pub author: String,
    pub person: String,
    /// ISO date-time
    pub date: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Thread {
    pub entries: Vec<Entry>,
    pub resolved: bool,
}

/// The comment parts a sheet had in its file.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct CommentsFile {
    pub comments: Option<String>,
    pub vml: Option<String>,
    pub vml_text: Option<String>,
    pub threads: Option<String>,
    pub notes: BTreeMap<(u32, u32), Note>,
    pub thread_map: BTreeMap<(u32, u32), Thread>,
}

pub const NOTE_W: f64 = 144.0;
pub const NOTE_H: f64 = 79.0;

fn guid(rng: &mut u64) -> String {
    let mut next = || {
        *rng ^= *rng << 13;
        *rng ^= *rng >> 7;
        *rng ^= *rng << 17;
        *rng
    };
    let a = next();
    let b = next();
    format!("{{{:08X}-{:04X}-{:04X}-{:04X}-{:012X}}}", (a >> 32) as u32, (a >> 16) as u16, (a as u16 & 0x0FFF) | 0x4000,
        ((b >> 48) as u16 & 0x3FFF) | 0x8000, b & 0xFFFF_FFFF_FFFF)
}

pub fn now_iso() -> String {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (t / 86400) as i64;
    let secs = t % 86400;
    // civil date from days since 1970-01-01
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.00", y, m, d, secs / 3600, (secs / 60) % 60, secs % 60)
}

impl Book {
    // ---------------------------------------------------------------- notes
    pub fn note(&self, s: usize, r: u32, c: u32) -> Option<&Note> {
        self.sheets.get(s)?.notes.get(&(r, c))
    }

    /// Add or change a note (text None deletes it).
    pub fn set_note(&mut self, s: usize, r: u32, c: u32, author: &str, text: Option<&str>) -> Result<(), String> {
        self.check_cell(s, r, c)?;
        let mut notes = self.sheets[s].notes.clone();
        let label = match text {
            None => {
                if notes.remove(&(r, c)).is_none() {
                    return Ok(());
                }
                "Delete Note"
            }
            Some(t) => {
                let e = notes.entry((r, c)).or_insert_with(|| Note { author: author.to_string(), w: NOTE_W, h: NOTE_H, ..Default::default() });
                if e.text == t {
                    return Ok(());
                }
                e.text = t.to_string();
                "Edit Note"
            }
        };
        self.begin(label, s);
        self.record(Delta::Notes(s, notes));
        self.commit();
        Ok(())
    }

    /// Show/Hide Note (None: toggle); Show All Notes uses `all`.
    pub fn show_notes(&mut self, s: usize, cells: Option<&[(u32, u32)]>, on: Option<bool>) -> Result<(), String> {
        let mut notes = self.sheets[s].notes.clone();
        let keys: Vec<(u32, u32)> = match cells {
            Some(k) => k.iter().copied().filter(|k| notes.contains_key(k)).collect(),
            None => notes.keys().copied().collect(),
        };
        if keys.is_empty() {
            return Ok(());
        }
        // toggling all: shown when any is hidden
        let target = on.unwrap_or_else(|| keys.iter().any(|k| !notes[k].visible));
        for k in keys {
            notes.get_mut(&k).unwrap().visible = target;
        }
        if notes == self.sheets[s].notes {
            return Ok(());
        }
        self.begin(if cells.is_none() { "Show All Notes" } else { "Show/Hide Note" }, s);
        self.record(Delta::Notes(s, notes));
        self.commit();
        Ok(())
    }

    pub fn set_note_size(&mut self, s: usize, r: u32, c: u32, w: f64, h: f64) -> Result<(), String> {
        let mut notes = self.sheets[s].notes.clone();
        let n = notes.get_mut(&(r, c)).ok_or("no note there")?;
        n.w = w.max(20.0);
        n.h = h.max(16.0);
        self.begin("Size Note", s);
        self.record(Delta::Notes(s, notes));
        self.commit();
        Ok(())
    }

    // ---------------------------------------------------------------- threads
    pub fn thread(&self, s: usize, r: u32, c: u32) -> Option<&Thread> {
        self.sheets.get(s)?.threads.get(&(r, c))
    }

    /// Start a thread, or reply to the one there.
    pub fn add_comment(&mut self, s: usize, r: u32, c: u32, author: &str, text: &str) -> Result<(), String> {
        self.check_cell(s, r, c)?;
        if text.trim().is_empty() {
            return Err("Type a comment first.".into());
        }
        if self.sheets[s].notes.contains_key(&(r, c)) {
            return Err("This cell has a note. A cell can have a note or a comment, not both.".into());
        }
        let mut threads = self.sheets[s].threads.clone();
        let mut rng = self.rng ^ (r as u64) << 20 ^ c as u64 ^ self.edits.wrapping_mul(0x9E3779B97F4A7C15);
        let person = self.person_id(author, &mut rng);
        let e = Entry { id: guid(&mut rng), author: author.to_string(), person, date: now_iso(), text: text.to_string() };
        self.rng = rng;
        let reply = threads.contains_key(&(r, c));
        threads.entry((r, c)).or_default().entries.push(e);
        self.begin(if reply { "Reply" } else { "New Comment" }, s);
        self.record(Delta::Threads(s, threads));
        self.commit();
        Ok(())
    }

    fn person_id(&self, author: &str, rng: &mut u64) -> String {
        for sh in &self.sheets {
            for t in sh.threads.values() {
                for e in &t.entries {
                    if e.author == author && !e.person.is_empty() {
                        return e.person.clone();
                    }
                }
            }
        }
        guid(rng)
    }

    /// Edit one entry of a thread (k), or delete it (None; the first deletes the thread).
    pub fn edit_comment(&mut self, s: usize, r: u32, c: u32, k: usize, text: Option<&str>) -> Result<(), String> {
        let mut threads = self.sheets[s].threads.clone();
        let t = threads.get_mut(&(r, c)).ok_or("no comment there")?;
        match text {
            Some(x) => t.entries.get_mut(k).ok_or("no such reply")?.text = x.to_string(),
            None => {
                if k == 0 {
                    threads.remove(&(r, c));
                } else if k < t.entries.len() {
                    t.entries.remove(k);
                }
            }
        }
        self.begin(if text.is_some() { "Edit Comment" } else { "Delete Comment" }, s);
        self.record(Delta::Threads(s, threads));
        self.commit();
        Ok(())
    }

    pub fn resolve_comment(&mut self, s: usize, r: u32, c: u32, resolved: bool) -> Result<(), String> {
        let mut threads = self.sheets[s].threads.clone();
        threads.get_mut(&(r, c)).ok_or("no comment there")?.resolved = resolved;
        self.begin(if resolved { "Resolve Thread" } else { "Reopen Thread" }, s);
        self.record(Delta::Threads(s, threads));
        self.commit();
        Ok(())
    }

    /// Delete the notes and comments in the ranges (Clear Comments and Notes, Delete).
    pub fn clear_comments(&mut self, s: usize, ranges: &[Rect]) -> Result<(), String> {
        let inside = |k: &(u32, u32)| ranges.iter().any(|r| r.contains(k.0, k.1));
        let notes: BTreeMap<(u32, u32), Note> = self.sheets[s].notes.iter().filter(|(k, _)| !inside(k)).map(|(k, v)| (*k, v.clone())).collect();
        let threads: BTreeMap<(u32, u32), Thread> = self.sheets[s].threads.iter().filter(|(k, _)| !inside(k)).map(|(k, v)| (*k, v.clone())).collect();
        if notes.len() == self.sheets[s].notes.len() && threads.len() == self.sheets[s].threads.len() {
            return Ok(());
        }
        self.begin("Delete", s);
        if notes.len() != self.sheets[s].notes.len() {
            self.record(Delta::Notes(s, notes));
        }
        if threads.len() != self.sheets[s].threads.len() {
            self.record(Delta::Threads(s, threads));
        }
        self.commit();
        Ok(())
    }

    /// Convert Notes to Comments.
    pub fn notes_to_comments(&mut self, s: usize) -> Result<usize, String> {
        let notes = self.sheets[s].notes.clone();
        if notes.is_empty() {
            return Ok(0);
        }
        let mut threads = self.sheets[s].threads.clone();
        let mut rng = self.rng ^ self.edits.wrapping_mul(0x9E3779B97F4A7C15) ^ 0x5bd1e995;
        let mut n = 0;
        let mut converted = Vec::new();
        for ((r, c), note) in &notes {
            if threads.contains_key(&(*r, *c)) {
                continue;
            }
            // the note's "Author:" line becomes the author
            let mut text = note.text.clone();
            let prefix = format!("{}:", note.author);
            if let Some(rest) = text.strip_prefix(&prefix) {
                text = rest.trim_start_matches(['\n', ' ']).to_string();
            }
            let person = self.person_id(&note.author, &mut rng);
            threads.insert((*r, *c), Thread { entries: vec![Entry { id: guid(&mut rng), author: note.author.clone(), person, date: now_iso(), text }], resolved: false });
            converted.push((*r, *c));
            n += 1;
        }
        self.rng = rng;
        let left: BTreeMap<(u32, u32), Note> = notes.into_iter().filter(|(k, _)| !converted.contains(k)).collect();
        self.begin("Convert to Comments", s);
        self.record(Delta::Threads(s, threads));
        self.record(Delta::Notes(s, left));
        self.commit();
        Ok(n)
    }

    /// The next (or previous) cell with a note or comment after (r, c), row by row.
    pub fn next_comment(&self, s: usize, r: u32, c: u32, back: bool, notes: bool) -> Option<(u32, u32)> {
        let sh = &self.sheets[s];
        let keys: Vec<(u32, u32)> = if notes { sh.notes.keys().copied().collect() } else { sh.threads.keys().copied().collect() };
        if keys.is_empty() {
            return None;
        }
        if back {
            keys.iter().rev().find(|k| **k < (r, c)).copied().or_else(|| keys.last().copied())
        } else {
            keys.iter().find(|k| **k > (r, c)).copied().or_else(|| keys.first().copied())
        }
    }

    /// Notes of src copied onto dst (recorded; the caller has a step open).
    pub(crate) fn copy_notes(&mut self, s: usize, src: Rect, dst: Rect) {
        let sh = &self.sheets[s];
        let dr = dst.r1 as i64 - src.r1 as i64;
        let dc = dst.c1 as i64 - src.c1 as i64;
        let add: Vec<((u32, u32), Note)> = sh.notes.iter().filter(|(k, _)| src.contains(k.0, k.1))
            .map(|(k, n)| (((k.0 as i64 + dr) as u32, (k.1 as i64 + dc) as u32), n.clone())).filter(|(k, _)| dst.contains(k.0, k.1)).collect();
        if add.is_empty() {
            return;
        }
        let mut notes = sh.notes.clone();
        for (k, n) in add {
            notes.insert(k, n);
        }
        self.record(Delta::Notes(s, notes));
    }

    /// Notes and threads move with inserted and deleted cells (recorded).
    pub(crate) fn shift_notes(&mut self, s: usize, map: &dyn Fn(u32, u32) -> Option<(u32, u32)>) {
        let sh = &self.sheets[s];
        if sh.notes.is_empty() && sh.threads.is_empty() {
            return;
        }
        let notes: BTreeMap<(u32, u32), Note> = sh.notes.iter().filter_map(|((r, c), n)| map(*r, *c).map(|k| (k, n.clone()))).collect();
        let threads: BTreeMap<(u32, u32), Thread> = sh.threads.iter().filter_map(|((r, c), n)| map(*r, *c).map(|k| (k, n.clone()))).collect();
        if notes != sh.notes {
            self.record(Delta::Notes(s, notes));
        }
        if threads != self.sheets[s].threads {
            self.record(Delta::Threads(s, threads));
        }
    }
}

// ---------------------------------------------------------------- the file

/// Notes from a comments part (and visibility and size from its VML drawing).
pub fn read_comments(text: &str, vml: Option<&str>) -> Option<(BTreeMap<(u32, u32), Note>, Vec<String>)> {
    let (_, e) = xml::parse(text).ok()?;
    let authors: Vec<String> = e.child("authors").map(|a| a.elems().map(|x| x.text()).collect()).unwrap_or_default();
    let mut notes = BTreeMap::new();
    let mut tc_ids = Vec::new();
    if let Some(list) = e.child("commentList") {
        for c in list.elems() {
            let (r, col) = match c.attr("ref").and_then(parse_cell) {
                Some(x) => x,
                None => continue,
            };
            let author = c.attr("authorId").and_then(|v| v.parse::<usize>().ok()).and_then(|i| authors.get(i)).cloned().unwrap_or_default();
            if author.starts_with("tc=") {
                // the stand-in Excel writes for a threaded comment
                tc_ids.push(author);
                continue;
            }
            let mut t = String::new();
            if let Some(tx) = c.child("text") {
                for k in tx.elems() {
                    match k.local() {
                        "t" => t.push_str(&xml::unescape_ooxml(&k.text())),
                        "r" => t.push_str(&xml::unescape_ooxml(&k.child("t").map(|x| x.text()).unwrap_or_default())),
                        _ => {}
                    }
                }
            }
            notes.insert((r, col), Note { author, text: t.replace("\r\n", "\n"), visible: false, w: NOTE_W, h: NOTE_H });
        }
    }
    if let Some(v) = vml {
        for (r, c, visible, w, h) in vml_notes(v) {
            if let Some(n) = notes.get_mut(&(r, c)) {
                n.visible = visible;
                if w > 0.0 {
                    n.w = w;
                }
                if h > 0.0 {
                    n.h = h;
                }
            }
        }
    }
    Some((notes, tc_ids))
}

/// (row, column, visible, width px, height px) of each note shape in a VML drawing.
fn vml_notes(v: &str) -> Vec<(u32, u32, bool, f64, f64)> {
    let mut out = Vec::new();
    let mut rest = v;
    while let Some(i) = rest.find("<v:shape ") {
        let after = &rest[i..];
        let end = after.find("</v:shape>").map(|e| e + 10).unwrap_or(after.len());
        let shape = &after[..end];
        rest = &after[end..];
        if !shape.contains("ObjectType=\"Note\"") {
            continue;
        }
        let tag = |name: &str| -> Option<String> {
            let open = format!("<x:{}>", name);
            let i = shape.find(&open)? + open.len();
            let j = shape[i..].find("</x:")? + i;
            Some(shape[i..j].trim().to_string())
        };
        let r: u32 = match tag("Row").and_then(|x| x.parse().ok()) {
            Some(x) => x,
            None => continue,
        };
        let c: u32 = match tag("Column").and_then(|x| x.parse().ok()) {
            Some(x) => x,
            None => continue,
        };
        let visible = shape.contains("<x:Visible") && !shape.contains("visibility:hidden");
        let style = shape.find("style=\"").map(|i| &shape[i + 7..]).and_then(|s| s.find('"').map(|j| &s[..j])).unwrap_or("");
        let dim = |key: &str| -> f64 {
            style.split(';').find_map(|p| {
                let (k, v) = p.split_once(':')?;
                if k.trim() != key {
                    return None;
                }
                let v = v.trim();
                if let Some(n) = v.strip_suffix("pt") {
                    n.parse::<f64>().ok().map(|x| x * 96.0 / 72.0)
                } else if let Some(n) = v.strip_suffix("px") {
                    n.parse::<f64>().ok()
                } else {
                    v.parse::<f64>().ok()
                }
            }).unwrap_or(0.0)
        };
        out.push((r, c, visible, dim("width"), dim("height")));
    }
    out
}

/// Threaded comments from their part (persons: id -> display name).
pub fn read_threads(text: &str, persons: &std::collections::HashMap<String, String>) -> BTreeMap<(u32, u32), Thread> {
    let mut out: BTreeMap<(u32, u32), Thread> = BTreeMap::new();
    let (_, e) = match xml::parse(text) {
        Ok(x) => x,
        Err(_) => return out,
    };
    for t in e.elems().filter(|x| x.local() == "threadedComment") {
        let (r, c) = match t.attr("ref").and_then(parse_cell) {
            Some(x) => x,
            None => continue,
        };
        let person = t.attr("personId").unwrap_or("").to_string();
        let entry = Entry {
            id: t.attr("id").unwrap_or("").to_string(),
            author: persons.get(&person).cloned().unwrap_or_else(|| "Author".into()),
            person,
            date: t.attr("dT").unwrap_or("").to_string(),
            text: t.child("text").map(|x| xml::unescape_ooxml(&x.text())).unwrap_or_default(),
        };
        let th = out.entry((r, c)).or_default();
        if t.attr("parentId").is_none() {
            th.resolved = matches!(t.attr("done"), Some("1") | Some("true"));
            th.entries.insert(0, entry);
        } else {
            th.entries.push(entry);
        }
    }
    out
}

pub fn read_persons(text: &str) -> std::collections::HashMap<String, String> {
    let mut m = std::collections::HashMap::new();
    if let Ok((_, e)) = xml::parse(text) {
        for p in e.elems() {
            if let (Some(id), Some(n)) = (p.attr("id"), p.attr("displayName")) {
                m.insert(id.to_string(), n.to_string());
            }
        }
    }
    m
}

fn esc(t: &str) -> String {
    let mut s = String::new();
    for ch in t.chars() {
        match ch {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '\r' => {}
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => s.push_str(&format!("_x{:04X}_", c as u32)),
            c => s.push(c),
        }
    }
    s
}

/// The comments part: notes, and stand-ins for threaded comments.
pub fn write_comments(notes: &BTreeMap<(u32, u32), Note>, threads: &BTreeMap<(u32, u32), Thread>) -> String {
    let mut authors: Vec<String> = Vec::new();
    let mut idx = |a: &str, authors: &mut Vec<String>| -> usize {
        match authors.iter().position(|x| x == a) {
            Some(i) => i,
            None => {
                authors.push(a.to_string());
                authors.len() - 1
            }
        }
    };
    let mut list = String::new();
    let mut all: Vec<((u32, u32), String)> = Vec::new();
    for ((r, c), n) in notes {
        let ai = idx(&n.author, &mut authors);
        let mut body = String::new();
        let prefix = format!("{}:", n.author);
        let run = |t: &str, bold: bool| -> String {
            format!("<r><rPr>{}<sz val=\"9\"/><color indexed=\"81\"/><rFont val=\"Tahoma\"/><family val=\"2\"/></rPr><t xml:space=\"preserve\">{}</t></r>",
                if bold { "<b/>" } else { "" }, esc(t))
        };
        if !n.author.is_empty() && n.text.starts_with(&prefix) {
            body.push_str(&run(&prefix, true));
            body.push_str(&run(&n.text[prefix.len()..], false));
        } else {
            body.push_str(&run(&n.text, false));
        }
        all.push(((*r, *c), format!("<comment ref=\"{}\" authorId=\"{}\" shapeId=\"0\"><text>{}</text></comment>", a1(*r, *c), ai, body)));
    }
    for ((r, c), t) in threads {
        let first = match t.entries.first() {
            Some(e) => e,
            None => continue,
        };
        let ai = idx(&format!("tc={}", first.id), &mut authors);
        let mut txt = String::from("[Threaded comment]\n\nComment:\n    ");
        txt.push_str(&first.text);
        for e in t.entries.iter().skip(1) {
            txt.push_str(&format!("\nReply:\n    {}", e.text));
        }
        all.push(((*r, *c), format!("<comment ref=\"{}\" authorId=\"{}\" shapeId=\"0\" xr:uid=\"{}\"><text><t xml:space=\"preserve\">{}</t></text></comment>",
            a1(*r, *c), ai, xml::escape_attr(&first.id), esc(&txt))));
    }
    all.sort_by_key(|x| x.0);
    for (_, x) in all {
        list.push_str(&x);
    }
    let mut out = String::from(xml::DECL);
    out.push_str("<comments xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" mc:Ignorable=\"xr\" xmlns:xr=\"http://schemas.microsoft.com/office/spreadsheetml/2014/revision\"><authors>");
    for a in &authors {
        out.push_str(&format!("<author>{}</author>", esc(a)));
    }
    out.push_str("</authors><commentList>");
    out.push_str(&list);
    out.push_str("</commentList></comments>");
    out
}

/// The VML drawing that shows the notes (and the threaded comments' stand-ins).
/// `keep`: the old drawing, whose other shapes (form controls) stay.
pub fn write_vml(book: &Book, s: usize, idmap: usize, keep: Option<&str>) -> String {
    let sh = &book.sheets[s];
    let mut shapes = String::new();
    let mut k = 1;
    let mut cells: Vec<((u32, u32), bool, f64, f64)> = sh.notes.iter().map(|(p, n)| (*p, n.visible, n.w, n.h)).collect();
    cells.extend(sh.threads.keys().map(|p| (*p, false, NOTE_W, NOTE_H)));
    cells.sort_by_key(|x| x.0);
    for ((r, c), visible, w, h) in cells {
        // the box sits to the right of the cell, a little up
        let x = book.col_x(s, c + 1) + 15.0;
        let y = (book.row_y(s, r) - 10.0).max(0.0);
        let mf = book.px_marker(s, x, y);
        let mt = book.px_marker(s, x + w, y + h);
        let to_pt = |px: f64| px * 72.0 / 96.0;
        shapes.push_str(&format!(
            "<v:shape id=\"_x0000_s{}\" type=\"#_x0000_t202\" style=\"position:absolute;margin-left:{:.2}pt;margin-top:{:.2}pt;width:{:.2}pt;height:{:.2}pt;z-index:{};visibility:{}\" fillcolor=\"#ffffe1\" o:insetmode=\"auto\"><v:fill color2=\"#ffffe1\"/><v:shadow on=\"t\" color=\"black\" obscured=\"t\"/><v:path o:connecttype=\"none\"/><v:textbox style=\"mso-direction-alt:auto\"><div style=\"text-align:left\"></div></v:textbox><x:ClientData ObjectType=\"Note\"><x:MoveWithCells/><x:SizeWithCells/><x:Anchor>{}, {}, {}, {}, {}, {}, {}, {}</x:Anchor><x:AutoFill>False</x:AutoFill><x:Row>{}</x:Row><x:Column>{}</x:Column>{}</x:ClientData></v:shape>",
            idmap * 1024 + k, to_pt(x), to_pt(y), to_pt(w), to_pt(h), k, if visible { "visible" } else { "hidden" },
            mf.col, (mf.col_off as f64 / crate::drawing::EMU).round() as i64, mf.row, (mf.row_off as f64 / crate::drawing::EMU).round() as i64,
            mt.col, (mt.col_off as f64 / crate::drawing::EMU).round() as i64, mt.row, (mt.row_off as f64 / crate::drawing::EMU).round() as i64,
            r, c, if visible { "<x:Visible/>" } else { "" }));
        k += 1;
    }
    // shapes of other kinds from the old drawing stay
    let mut others = String::new();
    let mut has_note_type = false;
    if let Some(old) = keep {
        let mut rest = old;
        while let Some(i) = rest.find("<v:shape ") {
            let after = &rest[i..];
            let end = after.find("</v:shape>").map(|e| e + 10).unwrap_or(after.len());
            let shape = &after[..end];
            if !shape.contains("ObjectType=\"Note\"") {
                others.push_str(shape);
            }
            rest = &after[end..];
        }
        has_note_type = old.contains("_x0000_t202");
        // the other shape types the old drawing declared
        let mut rest = old;
        while let Some(i) = rest.find("<v:shapetype ") {
            let after = &rest[i..];
            let end = after.find("</v:shapetype>").map(|e| e + 14).unwrap_or(after.len());
            let st = &after[..end];
            if !st.contains("_x0000_t202") {
                others.insert_str(0, st);
            }
            rest = &after[end..];
        }
    }
    let _ = has_note_type;
    format!("<xml xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" xmlns:x=\"urn:schemas-microsoft-com:office:excel\"><o:shapelayout v:ext=\"edit\"><o:idmap v:ext=\"edit\" data=\"{}\"/></o:shapelayout><v:shapetype id=\"_x0000_t202\" coordsize=\"21600,21600\" o:spt=\"202\" path=\"m,l,21600r21600,l21600,xe\"><v:stroke joinstyle=\"miter\"/><v:path gradientshapeok=\"t\" o:connecttype=\"rect\"/></v:shapetype>{}{}</xml>",
        idmap, others, shapes)
}

pub fn write_threads(threads: &BTreeMap<(u32, u32), Thread>) -> String {
    let mut out = String::from(xml::DECL);
    out.push_str("<ThreadedComments xmlns=\"http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments\" xmlns:x=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">");
    for ((r, c), t) in threads {
        let root = match t.entries.first() {
            Some(e) => e.id.clone(),
            None => continue,
        };
        for (k, e) in t.entries.iter().enumerate() {
            out.push_str(&format!("<threadedComment ref=\"{}\" dT=\"{}\" personId=\"{}\" id=\"{}\"", a1(*r, *c), xml::escape_attr(&e.date), xml::escape_attr(&e.person), xml::escape_attr(&e.id)));
            if k > 0 {
                out.push_str(&format!(" parentId=\"{}\"", xml::escape_attr(&root)));
            } else if t.resolved {
                out.push_str(" done=\"1\"");
            }
            out.push_str(&format!("><text>{}</text></threadedComment>", esc(&e.text)));
        }
    }
    out.push_str("</ThreadedComments>");
    out
}

pub fn write_persons(book: &Book) -> Option<String> {
    let mut seen: Vec<(String, String)> = Vec::new();
    for sh in &book.sheets {
        for t in sh.threads.values() {
            for e in &t.entries {
                if !seen.iter().any(|x| x.0 == e.person) {
                    seen.push((e.person.clone(), e.author.clone()));
                }
            }
        }
    }
    if seen.is_empty() {
        return None;
    }
    let mut out = String::from(xml::DECL);
    out.push_str("<personList xmlns=\"http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments\" xmlns:x=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">");
    for (id, name) in seen {
        out.push_str(&format!("<person displayName=\"{}\" id=\"{}\" userId=\"{}\" providerId=\"None\"/>", xml::escape_attr(&name), xml::escape_attr(&id), xml::escape_attr(&name)));
    }
    out.push_str("</personList>");
    Some(out)
}

#[allow(dead_code)]
fn unused(_: Element, _: Node) {}
