//! Flash Fill: learn how an example output is put together from the texts on its row
//! (pieces of the neighbouring cells, changed in case, with fixed text between), then
//! apply that to the other rows.

#[derive(Clone, Debug, PartialEq)]
enum Ext {
    Whole,
    /// k-th token (from the end when negative) splitting on these delimiters
    Token(i32, &'static str),
    /// the first letter of the k-th token
    Initial(i32, &'static str),
    Before(char, bool),
    After(char, bool),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Case {
    Same,
    Upper,
    Lower,
    Proper,
}

#[derive(Clone, Debug, PartialEq)]
enum Part {
    Const(String),
    Sub(usize, Ext, Case),
}

#[derive(Clone, Debug)]
pub struct Program {
    parts: Vec<Part>,
}

const DELIMS: [&str; 4] = [" ", " ,", "-", "@."];
const MARKS: [char; 6] = [' ', ',', '-', '@', '.', '/'];

fn tokens<'a>(s: &'a str, d: &str) -> Vec<&'a str> {
    s.split(|c: char| d.contains(c)).filter(|t| !t.is_empty()).collect()
}

fn pick<'a>(v: &[&'a str], k: i32) -> Option<&'a str> {
    if k >= 0 {
        v.get(k as usize).copied()
    } else {
        let i = v.len() as i32 + k;
        if i >= 0 { v.get(i as usize).copied() } else { None }
    }
}

fn extract(s: &str, e: &Ext) -> Option<String> {
    match e {
        Ext::Whole => Some(s.to_string()),
        Ext::Token(k, d) => pick(&tokens(s, d), *k).map(|t| t.to_string()),
        Ext::Initial(k, d) => pick(&tokens(s, d), *k).and_then(|t| t.chars().next()).map(|c| c.to_string()),
        Ext::Before(c, last) => {
            let i = if *last { s.rfind(*c) } else { s.find(*c) }?;
            Some(s[..i].to_string())
        }
        Ext::After(c, last) => {
            let i = if *last { s.rfind(*c) } else { s.find(*c) }?;
            Some(s[i + c.len_utf8()..].to_string())
        }
    }
}

fn apply_case(s: &str, c: Case) -> String {
    match c {
        Case::Same => s.to_string(),
        Case::Upper => s.to_uppercase(),
        Case::Lower => s.to_lowercase(),
        Case::Proper => {
            let mut out = String::new();
            let mut start = true;
            for ch in s.chars() {
                if start {
                    out.extend(ch.to_uppercase());
                } else {
                    out.extend(ch.to_lowercase());
                }
                start = !ch.is_alphanumeric();
            }
            out
        }
    }
}

fn atoms(inputs: &[String]) -> Vec<(Part, String)> {
    let mut out = Vec::new();
    for (i, s) in inputs.iter().enumerate() {
        if s.is_empty() {
            continue;
        }
        let mut exts = vec![Ext::Whole];
        for d in DELIMS {
            let n = tokens(s, d).len() as i32;
            for k in 0..n.min(6) {
                exts.push(Ext::Token(k, d));
                exts.push(Ext::Initial(k, d));
            }
            for k in 1..=n.min(3) {
                exts.push(Ext::Token(-k, d));
                exts.push(Ext::Initial(-k, d));
            }
        }
        for m in MARKS {
            if s.contains(m) {
                for last in [false, true] {
                    exts.push(Ext::Before(m, last));
                    exts.push(Ext::After(m, last));
                }
            }
        }
        for e in exts {
            if let Some(v) = extract(s, &e) {
                if v.is_empty() {
                    continue;
                }
                for c in [Case::Same, Case::Upper, Case::Lower, Case::Proper] {
                    let t = apply_case(&v, c);
                    if c != Case::Same && t == v {
                        continue;
                    }
                    out.push((Part::Sub(i, e.clone(), c), t));
                }
            }
        }
    }
    out
}

impl Program {
    pub fn run(&self, inputs: &[String]) -> Option<String> {
        let mut out = String::new();
        for p in &self.parts {
            match p {
                Part::Const(t) => out.push_str(t),
                Part::Sub(i, e, c) => out.push_str(&apply_case(&extract(inputs.get(*i)?, e)?, *c)),
            }
        }
        Some(out)
    }
}

/// Find a program that reproduces every example.
pub fn learn(examples: &[(Vec<String>, String)]) -> Option<Program> {
    let (inputs, target) = &examples[0];
    let cand = atoms(inputs);
    let t: Vec<char> = target.chars().collect();
    let mut found: Option<Program> = None;
    let mut tried = 0usize;
    let mut parts: Vec<Part> = Vec::new();
    search(&t, 0, &cand, &mut parts, examples, &mut found, &mut tried);
    found
}

fn search(t: &[char], pos: usize, cand: &[(Part, String)], parts: &mut Vec<Part>, ex: &[(Vec<String>, String)],
          found: &mut Option<Program>, tried: &mut usize) {
    if found.is_some() || *tried > 20000 {
        return;
    }
    if pos == t.len() {
        *tried += 1;
        let p = Program { parts: parts.clone() };
        // a program of constants only is no pattern
        if !p.parts.iter().any(|x| matches!(x, Part::Sub(..))) {
            return;
        }
        if ex.iter().all(|(i, o)| p.run(i).as_deref() == Some(o.as_str())) {
            *found = Some(p);
        }
        return;
    }
    // the longest pieces of the inputs first
    let rest: String = t[pos..].iter().collect();
    let mut matches: Vec<&(Part, String)> = cand.iter().filter(|(_, v)| rest.starts_with(v.as_str())).collect();
    matches.sort_by_key(|(_, v)| std::cmp::Reverse(v.chars().count()));
    for (p, v) in matches.iter().take(12) {
        parts.push(p.clone());
        search(t, pos + v.chars().count(), cand, parts, ex, found, tried);
        parts.pop();
        if found.is_some() {
            return;
        }
    }
    // or a fixed character
    let ch = t[pos].to_string();
    match parts.last_mut() {
        Some(Part::Const(s)) => {
            s.push_str(&ch);
            search(t, pos + 1, cand, parts, ex, found, tried);
            if let Some(Part::Const(s)) = parts.last_mut() {
                s.pop();
            }
        }
        _ => {
            parts.push(Part::Const(ch));
            search(t, pos + 1, cand, parts, ex, found, tried);
            parts.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ex(i: &[&str], o: &str) -> (Vec<String>, String) {
        (i.iter().map(|s| s.to_string()).collect(), o.to_string())
    }
    #[test]
    fn learns() {
        let p = learn(&[ex(&["John Smith"], "Smith, John")]).unwrap();
        assert_eq!(p.run(&["Ada Lovelace".into()]).unwrap(), "Lovelace, Ada");
        let p = learn(&[ex(&["john.doe@x.com"], "John")]).unwrap();
        assert_eq!(p.run(&["mary.ann@y.org".into()]).unwrap(), "Mary");
        let p = learn(&[ex(&["Grace", "Hopper"], "G. Hopper")]).unwrap();
        assert_eq!(p.run(&["Alan".into(), "Turing".into()]).unwrap(), "A. Turing");
        let p = learn(&[ex(&["2026-10-22"], "10")]).unwrap();
        assert_eq!(p.run(&["2025-03-01".into()]).unwrap(), "03");
    }
}
