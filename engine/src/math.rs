//! Equations (SH-21): Office Math (OMML) as it sits in a text box (a14:m), the linear format
//! (UnicodeMath, with the common LaTeX commands) it is typed in, and drawing it.

use crate::xml::{self, Element};

#[derive(Clone, Debug, PartialEq)]
pub enum M {
    Row(Vec<M>),
    /// a run of math text: letters are drawn italic
    Text(String),
    /// upright text (function names, m:sty="p")
    Plain(String),
    /// numerator, denominator, with a bar
    Frac(Box<M>, Box<M>, bool),
    /// base, subscript, superscript
    Scr(Box<M>, Option<Box<M>>, Option<Box<M>>),
    /// degree, body
    Rad(Option<Box<M>>, Box<M>),
    /// operator, lower limit, upper limit, body, limits above and below
    Nary(char, Option<Box<M>>, Option<Box<M>>, Box<M>, bool),
    /// opening, closing, the parts (shown with separators between)
    Delim(String, String, Vec<M>),
    Func(Box<M>, Box<M>),
    Acc(char, Box<M>),
    /// a bar over (true) or under
    Bar(Box<M>, bool),
    /// base, limit, the limit below (true) or above
    Lim(Box<M>, Box<M>, bool),
    Matrix(Vec<Vec<M>>),
    /// invisible brackets
    Group(Box<M>),
}

pub const NS_M: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";
pub const NS_A14: &str = "http://schemas.microsoft.com/office/drawing/2010/main";
pub const PLACEHOLDER: &str = "Type equation here.";
pub const FONT: &str = "Cambria Math,STIX Two Math,Latin Modern Math,DejaVu Serif";

const FUNCS: &[&str] = &["sin", "cos", "tan", "cot", "sec", "csc", "sinh", "cosh", "tanh", "coth", "arcsin", "arccos",
    "arctan", "log", "ln", "lg", "exp", "lim", "max", "min", "sup", "inf", "det", "gcd", "arg", "deg", "dim", "ker", "Pr"];
const NARY: &str = "∑∏∐∫∬∭∮∯∰⋃⋂⋁⋀⨁⨂⨀";
const OPS: &str = "+-−=<>±∓×·÷,;:≤≥≠≈≡≅∝∼≪≫∈∉∋⊂⊃⊆⊇∪∩→←↔⇒⇐⇔↦∧∨¬∀∃∄∴∵⋅∗∘≔⊕⊗|‖";
const OPEN: &str = "([{⟨⌊⌈|‖";

/// The LaTeX-style commands, as characters.
pub fn command(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α", "beta" => "β", "gamma" => "γ", "delta" => "δ", "epsilon" => "ϵ", "varepsilon" => "ε",
        "zeta" => "ζ", "eta" => "η", "theta" => "θ", "vartheta" => "ϑ", "iota" => "ι", "kappa" => "κ",
        "lambda" => "λ", "mu" => "μ", "nu" => "ν", "xi" => "ξ", "pi" => "π", "varpi" => "ϖ", "rho" => "ρ",
        "sigma" => "σ", "varsigma" => "ς", "tau" => "τ", "upsilon" => "υ", "phi" => "ϕ", "varphi" => "φ",
        "chi" => "χ", "psi" => "ψ", "omega" => "ω", "Gamma" => "Γ", "Delta" => "Δ", "Theta" => "Θ",
        "Lambda" => "Λ", "Xi" => "Ξ", "Pi" => "Π", "Sigma" => "Σ", "Upsilon" => "Υ", "Phi" => "Φ", "Psi" => "Ψ",
        "Omega" => "Ω", "infty" => "∞", "pm" => "±", "mp" => "∓", "times" => "×", "cdot" => "⋅", "div" => "÷",
        "le" | "leq" => "≤", "ge" | "geq" => "≥", "ne" | "neq" => "≠", "approx" => "≈", "equiv" => "≡",
        "cong" => "≅", "sim" => "∼", "propto" => "∝", "ll" => "≪", "gg" => "≫", "in" => "∈", "notin" => "∉",
        "ni" => "∋", "subset" => "⊂", "supset" => "⊃", "subseteq" => "⊆", "supseteq" => "⊇", "cup" => "∪",
        "cap" => "∩", "emptyset" => "∅", "to" | "rightarrow" => "→", "leftarrow" => "←",
        "leftrightarrow" => "↔", "Rightarrow" => "⇒", "Leftarrow" => "⇐", "Leftrightarrow" => "⇔",
        "mapsto" => "↦", "uparrow" => "↑", "downarrow" => "↓", "forall" => "∀", "exists" => "∃",
        "partial" => "∂", "nabla" => "∇", "neg" => "¬", "wedge" => "∧", "vee" => "∨", "therefore" => "∴",
        "because" => "∵", "ldots" | "dots" => "…", "cdots" => "⋯", "vdots" => "⋮", "ddots" => "⋱",
        "angle" => "∠", "degree" => "°", "circ" => "∘", "ast" => "∗", "star" => "⋆", "oplus" => "⊕",
        "otimes" => "⊗", "aleph" => "ℵ", "hbar" => "ℏ", "ell" => "ℓ", "Re" => "ℜ", "Im" => "ℑ",
        "sum" => "∑", "prod" => "∏", "coprod" => "∐", "int" => "∫", "iint" => "∬", "iiint" => "∭",
        "oint" => "∮", "bigcup" => "⋃", "bigcap" => "⋂", "langle" => "⟨", "rangle" => "⟩", "lfloor" => "⌊",
        "rfloor" => "⌋", "lceil" => "⌈", "rceil" => "⌉", "lbrace" | "{" => "{", "rbrace" | "}" => "}",
        "|" | "Vert" => "‖", "qquad" => "\u{2003}\u{2003}", "quad" => "\u{2003}", "," => "\u{2009}", ";" => "\u{2005}",
        " " => " ", "box" => "□", "square" => "■",
        _ => return None,
    })
}

fn accent_of(name: &str) -> Option<char> {
    Some(match name {
        "hat" | "widehat" => '\u{0302}', "check" => '\u{030C}', "tilde" | "widetilde" => '\u{0303}', "acute" => '\u{0301}',
        "grave" => '\u{0300}', "dot" => '\u{0307}', "ddot" => '\u{0308}', "dddot" => '\u{20DB}', "breve" => '\u{0306}',
        "vec" => '\u{20D7}', "bar" => '\u{0305}',
        _ => return None,
    })
}

fn is_accent(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036F}' | '\u{20D0}'..='\u{20FF}')
}

fn close_of(c: char) -> char {
    match c {
        '(' => ')', '[' => ']', '{' => '}', '⟨' => '⟩', '⌊' => '⌋', '⌈' => '⌉', '|' => '|', '‖' => '‖', '〖' => '〗',
        _ => ')',
    }
}

fn is_factor_char(c: char) -> bool {
    c.is_alphanumeric() || c == '.' || c == '!' || c == '′' || c == '∞' || c == '∂' || c == '…' || c == '⋯'
}

fn row(mut v: Vec<M>) -> M {
    if v.len() == 1 {
        v.pop().unwrap()
    } else {
        M::Row(v)
    }
}

/// Brackets round a fraction's parts and a script are only there to group: they go.
fn unparen(m: M) -> M {
    match m {
        M::Delim(b, e, mut items) if b == "(" && e == ")" && items.len() == 1 => items.pop().unwrap(),
        M::Group(x) => *x,
        other => other,
    }
}

// ---------------------------------------------------------------------- the linear format

struct P {
    s: Vec<char>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }
    fn peek_at(&self, k: usize) -> Option<char> {
        self.s.get(self.i + k).copied()
    }
    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\u{2061}')) {
            self.i += 1;
        }
    }

    /// A sequence up to one of the stop characters (not taken).
    fn expr(&mut self, stops: &[char]) -> M {
        let mut items: Vec<M> = Vec::new();
        loop {
            let c = match self.peek() {
                None => break,
                Some(c) => c,
            };
            if stops.contains(&c) {
                break;
            }
            if c == ' ' || c == '\u{2061}' {
                self.i += 1;
                items.push(M::Text(" ".into()));
                continue;
            }
            if c == '/' || c == '¦' || c == '⊘' {
                self.i += 1;
                let num = take_factors(&mut items);
                self.skip_ws();
                let den = self.operand(stops);
                items.push(M::Frac(Box::new(unparen(num)), Box::new(unparen(den)), c != '¦'));
                continue;
            }
            if c == '\\' && self.word_at(self.i + 1) == "over" {
                self.i += 5;
                let num = row(std::mem::take(&mut items));
                let den = self.expr(stops);
                items.push(M::Frac(Box::new(num), Box::new(den), true));
                continue;
            }
            match self.factor(stops) {
                Some(m) => items.push(m),
                None => {
                    self.i += 1;
                    items.push(M::Text(c.to_string()));
                }
            }
        }
        // spaces only separate
        let items: Vec<M> = items.into_iter().filter(|m| !matches!(m, M::Text(t) if t == " ")).collect();
        merge_text(row(items))
    }

    /// The operand after a / : a bracket, or a run of letters, digits and scripts.
    fn operand(&mut self, stops: &[char]) -> M {
        self.operand_ex(stops, false)
    }

    /// `brackets`: a bracket straight after a letter belongs to the operand (f(x)dx).
    fn operand_ex(&mut self, stops: &[char], brackets: bool) -> M {
        let mut out = Vec::new();
        if let Some(c) = self.peek() {
            if OPEN.contains(c) || c == '〖' || c == '{' {
                if let Some(m) = self.factor(stops) {
                    return m;
                }
            }
        }
        loop {
            match self.peek() {
                Some(c) if (is_factor_char(c) || c == '\\' || c == '√' || c == '∛' || c == '∜' || NARY.contains(c)
                    || (brackets && !out.is_empty() && (c == '(' || c == '[')))
                    && !stops.contains(&c) => {
                    let at = self.i;
                    match self.factor(stops) {
                        Some(m) => out.push(m),
                        None => {
                            self.i = at;
                            break;
                        }
                    }
                    if matches!(out.last(), Some(M::Func(..)) | Some(M::Nary(..))) {
                        break;
                    }
                }
                _ => break,
            }
        }
        if out.is_empty() {
            return M::Text(String::new());
        }
        merge_text(row(out))
    }

    fn word_at(&self, at: usize) -> String {
        let mut w = String::new();
        let mut k = at;
        while let Some(c) = self.s.get(k) {
            if c.is_ascii_alphabetic() {
                w.push(*c);
                k += 1;
            } else {
                break;
            }
        }
        w
    }

    /// LaTeX grouping braces: {...} read as invisible brackets.
    fn group_braces(&mut self) -> M {
        self.i += 1;
        let inner = self.expr(&['}']);
        self.eat('}');
        inner
    }

    /// A script's or a limit's argument.
    fn arg(&mut self, stops: &[char]) -> M {
        self.skip_ws();
        if self.peek() == Some('{') {
            return self.group_braces();
        }
        match self.peek() {
            Some(c) if OPEN.contains(c) || c == '{' || c == '〖' => {
                let m = self.atom(stops).unwrap_or(M::Text(String::new()));
                unparen(m)
            }
            Some('\\') => self.atom(stops).unwrap_or(M::Text(String::new())),
            Some(c) if c.is_ascii_digit() => {
                let mut t = String::new();
                while let Some(d) = self.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        t.push(d);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                M::Text(t)
            }
            Some(c) if !stops.contains(&c) && c != '^' && c != '_' => {
                self.i += 1;
                M::Text(c.to_string())
            }
            _ => M::Text(String::new()),
        }
    }

    /// An atom with what follows it: scripts, accents, limits.
    fn factor(&mut self, stops: &[char]) -> Option<M> {
        let mut base = self.atom(stops)?;
        loop {
            match self.peek() {
                Some(c) if is_accent(c) => {
                    self.i += 1;
                    base = M::Acc(c, Box::new(unparen(base)));
                }
                Some('_') | Some('^') => {
                    if let M::Nary(..) = base {
                        // the limits were read with the operator
                        break;
                    }
                    let (mut sub, mut sup) = (None, None);
                    while let Some(c) = self.peek() {
                        if c == '_' && sub.is_none() {
                            self.i += 1;
                            sub = Some(Box::new(self.arg(stops)));
                        } else if c == '^' && sup.is_none() {
                            self.i += 1;
                            sup = Some(Box::new(self.arg(stops)));
                        } else {
                            break;
                        }
                    }
                    base = M::Scr(Box::new(base), sub, sup);
                }
                Some('┬') | Some('┴') => {
                    let below = self.peek() == Some('┬');
                    self.i += 1;
                    let l = self.arg(stops);
                    base = M::Lim(Box::new(base), Box::new(l), below);
                }
                _ => break,
            }
        }
        // a function name takes what follows as its argument
        let is_fn = match &base {
            M::Plain(_) => true,
            M::Lim(b, _, _) => matches!(**b, M::Plain(_)),
            M::Scr(b, _, _) => matches!(**b, M::Plain(_)),
            _ => false,
        };
        if is_fn {
            let save = self.i;
            self.skip_ws();
            match self.peek() {
                Some(c) if !stops.contains(&c) && !OPS.contains(c) || c == '(' || c == '|' => {
                    let a = self.operand(stops);
                    if !matches!(&a, M::Text(t) if t.is_empty()) {
                        return Some(M::Func(Box::new(base), Box::new(match a { M::Group(x) => *x, o => o })));
                    }
                    self.i = save;
                }
                _ => self.i = save,
            }
        }
        Some(base)
    }

    fn atom(&mut self, stops: &[char]) -> Option<M> {
        let c = self.peek()?;
        if stops.contains(&c) {
            return None;
        }
        // brackets
        if c == '〖' {
            self.i += 1;
            let close = close_of(c);
            let inner = self.expr(&[close]);
            self.eat(close);
            return Some(M::Group(Box::new(inner)));
        }
        if (OPEN.contains(c) && !(c == '|' || c == '‖')) || ((c == '|' || c == '‖') && self.closes_later(c)) {
            self.i += 1;
            let close = close_of(c);
            let mut parts = vec![];
            loop {
                parts.push(self.expr(&[close, '│', '∣']));
                match self.peek() {
                    Some('│') | Some('∣') => {
                        self.i += 1;
                    }
                    _ => break,
                }
            }
            let end = if self.eat(close) { close.to_string() } else { String::new() };
            return Some(M::Delim(c.to_string(), end, parts));
        }
        // radicals
        if c == '√' || c == '∛' || c == '∜' {
            self.i += 1;
            let mut deg = match c {
                '∛' => Some(Box::new(M::Text("3".into()))),
                '∜' => Some(Box::new(M::Text("4".into()))),
                _ => None,
            };
            if self.peek() == Some('(') {
                self.i += 1;
                let first = self.expr(&[')', '&']);
                let body = if self.eat('&') {
                    deg = Some(Box::new(first));
                    self.expr(&[')'])
                } else {
                    first
                };
                self.eat(')');
                return Some(M::Rad(deg, Box::new(body)));
            }
            let body = self.operand(stops);
            return Some(M::Rad(deg, Box::new(unparen(body))));
        }
        // big operators
        if NARY.contains(c) {
            self.i += 1;
            return Some(self.nary(c, stops, false));
        }
        // matrices
        if c == '■' {
            self.i += 1;
            return Some(self.matrix());
        }
        if c == '¯' || c == '▁' {
            self.i += 1;
            let a = self.arg(stops);
            return Some(M::Bar(Box::new(a), c == '¯'));
        }
        if c == '\\' {
            return self.command(stops);
        }
        if c.is_ascii_digit() {
            let mut t = String::new();
            while let Some(d) = self.peek() {
                if d.is_ascii_digit() || (d == '.' && self.peek_at(1).map(|x| x.is_ascii_digit()).unwrap_or(false)) {
                    t.push(d);
                    self.i += 1;
                } else {
                    break;
                }
            }
            return Some(M::Text(t));
        }
        if c.is_ascii_alphabetic() {
            // a function's name, or letters
            let w = self.word_at(self.i);
            if let Some(f) = FUNCS.iter().filter(|f| w.starts_with(**f)).max_by_key(|f| f.len()) {
                if w.len() == f.len() || FUNCS.contains(&w.as_str()) {
                    self.i += f.len();
                    return Some(M::Plain(f.to_string()));
                }
            }
            self.i += 1;
            return Some(M::Text(c.to_string()));
        }
        if c.is_alphanumeric() || "∞∂∇…⋯′!°∅ℏℓ".contains(c) {
            self.i += 1;
            return Some(M::Text(c.to_string()));
        }
        if ")]}⟩⌋⌉〗&@│▒".contains(c) {
            return None;
        }
        self.i += 1;
        Some(M::Text(c.to_string()))
    }

    fn closes_later(&self, c: char) -> bool {
        self.s[self.i + 1..].contains(&c)
    }

    fn nary(&mut self, op: char, stops: &[char], latex: bool) -> M {
        let (mut sub, mut sup) = (None, None);
        loop {
            match self.peek() {
                Some('_') if sub.is_none() => {
                    self.i += 1;
                    sub = Some(Box::new(self.arg(stops)));
                }
                Some('^') if sup.is_none() => {
                    self.i += 1;
                    sup = Some(Box::new(self.arg(stops)));
                }
                Some('┬') if sub.is_none() => {
                    self.i += 1;
                    sub = Some(Box::new(self.arg(stops)));
                }
                Some('┴') if sup.is_none() => {
                    self.i += 1;
                    sup = Some(Box::new(self.arg(stops)));
                }
                _ => break,
            }
        }
        self.skip_ws();
        self.eat('▒');
        self.skip_ws();
        let body = match self.peek() {
            Some('〖') => {
                let m = self.atom(stops).unwrap_or(M::Text(String::new()));
                unparen(m)
            }
            Some('{') if latex => self.group_braces(),
            _ => self.operand_ex(stops, true),
        };
        let und = !"∫∬∭∮∯∰".contains(op);
        M::Nary(op, sub, sup, Box::new(body), und)
    }

    fn matrix(&mut self) -> M {
        let mut rows = vec![];
        if !self.eat('(') && !self.eat('{') {
            return M::Matrix(vec![]);
        }
        let mut cur = vec![];
        loop {
            let cell = self.expr(&['&', '@', ')', '}']);
            cur.push(cell);
            match self.peek() {
                Some('&') => {
                    self.i += 1;
                }
                Some('@') => {
                    self.i += 1;
                    rows.push(std::mem::take(&mut cur));
                }
                _ => {
                    self.i += 1;
                    break;
                }
            }
        }
        rows.push(cur);
        M::Matrix(rows)
    }

    fn braced(&mut self, stops: &[char]) -> M {
        self.skip_ws();
        if self.peek() == Some('{') {
            return self.group_braces();
        }
        match self.peek() {
            Some('{') | Some('(') | Some('〖') => unparen(self.atom(stops).unwrap_or(M::Text(String::new()))),
            _ => self.arg(stops),
        }
    }

    fn command(&mut self, stops: &[char]) -> Option<M> {
        self.i += 1;
        let mut w = self.word_at(self.i);
        if w.is_empty() {
            // \{ \| \, ...
            let c = self.peek()?;
            self.i += 1;
            return Some(M::Text(command(&c.to_string()).unwrap_or("").to_string()));
        }
        self.i += w.len();
        match w.as_str() {
            "frac" | "dfrac" | "tfrac" | "binom" => {
                let a = self.braced(stops);
                let b = self.braced(stops);
                if w == "binom" {
                    return Some(M::Delim("(".into(), ")".into(), vec![M::Frac(Box::new(a), Box::new(b), false)]));
                }
                Some(M::Frac(Box::new(a), Box::new(b), true))
            }
            "sqrt" => {
                self.skip_ws();
                let deg = if self.eat('[') {
                    let d = self.expr(&[']']);
                    self.eat(']');
                    Some(Box::new(d))
                } else {
                    None
                };
                let b = self.braced(stops);
                Some(M::Rad(deg, Box::new(b)))
            }
            "overline" => Some(M::Bar(Box::new(self.braced(stops)), true)),
            "underline" => Some(M::Bar(Box::new(self.braced(stops)), false)),
            "matrix" | "pmatrix" | "bmatrix" => {
                let m = self.matrix();
                Some(match w.as_str() {
                    "pmatrix" => M::Delim("(".into(), ")".into(), vec![m]),
                    "bmatrix" => M::Delim("[".into(), "]".into(), vec![m]),
                    _ => m,
                })
            }
            "left" | "right" | "big" | "Big" | "bigg" | "Bigg" => {
                // the brackets size themselves
                self.atom(stops)
            }
            "mathrm" | "text" | "operatorname" => {
                let b = self.braced(stops);
                Some(M::Plain(flat_text(&b)))
            }
            "limits" | "displaystyle" | "nolimits" => Some(M::Text(String::new())),
            _ => {
                if let Some(a) = accent_of(&w) {
                    let b = self.braced(stops);
                    if a == '\u{0305}' {
                        return Some(M::Bar(Box::new(b), true));
                    }
                    return Some(M::Acc(a, Box::new(b)));
                }
                if FUNCS.contains(&w.as_str()) {
                    return Some(M::Plain(std::mem::take(&mut w)));
                }
                match command(&w) {
                    Some(ch) => {
                        let c = ch.chars().next().unwrap_or(' ');
                        if ch.chars().count() == 1 && NARY.contains(c) {
                            return Some(self.nary(c, stops, true));
                        }
                        Some(M::Text(ch.to_string()))
                    }
                    None => Some(M::Text(format!("\\{}", w))),
                }
            }
        }
    }
}

/// The factors at the end of a list, for a fraction's numerator.
fn take_factors(items: &mut Vec<M>) -> M {
    if let Some(last) = items.last() {
        if matches!(last, M::Delim(..) | M::Group(..) | M::Frac(..) | M::Rad(..)) {
            return items.pop().unwrap();
        }
    }
    let mut out = Vec::new();
    while let Some(last) = items.last() {
        let ok = match last {
            M::Text(t) => !t.is_empty() && t != " " && t.chars().all(is_factor_char),
            M::Scr(..) | M::Acc(..) | M::Bar(..) => true,
            _ => false,
        };
        if !ok {
            break;
        }
        out.push(items.pop().unwrap());
    }
    out.reverse();
    if out.is_empty() {
        return M::Text(String::new());
    }
    merge_text(row(out))
}

/// Adjacent text runs become one.
fn merge_text(m: M) -> M {
    match m {
        M::Row(items) => {
            let mut out: Vec<M> = Vec::new();
            for it in items {
                if let (Some(M::Text(a)), M::Text(b)) = (out.last_mut(), &it) {
                    let digit_a = a.chars().last().map(|c| c.is_ascii_digit()).unwrap_or(false);
                    let digit_b = b.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false);
                    // keep letters apart from operators; numbers stay together
                    if digit_a && digit_b || (!a.chars().any(|c| OPS.contains(c)) && !b.chars().any(|c| OPS.contains(c))) {
                        a.push_str(b);
                        continue;
                    }
                }
                out.push(it);
            }
            row(out)
        }
        other => other,
    }
}

fn flat_text(m: &M) -> String {
    let mut s = String::new();
    fn walk(m: &M, s: &mut String) {
        match m {
            M::Text(t) | M::Plain(t) => s.push_str(t),
            M::Row(v) => v.iter().for_each(|x| walk(x, s)),
            M::Group(x) => walk(x, s),
            M::Delim(b, e, v) => {
                s.push_str(b);
                v.iter().for_each(|x| walk(x, s));
                s.push_str(e);
            }
            _ => {}
        }
    }
    walk(m, &mut s);
    s
}

/// Read the linear format (UnicodeMath, or LaTeX commands).
pub fn parse_linear(text: &str) -> M {
    let mut p = P { s: text.trim().chars().collect(), i: 0 };
    let mut parts = vec![];
    while p.i < p.s.len() {
        parts.push(p.expr(&[]));
        if p.i < p.s.len() {
            // a stray closing bracket: kept as text
            let c = p.s[p.i];
            p.i += 1;
            parts.push(M::Text(c.to_string()));
        }
    }
    merge_text(row(parts))
}

// ---------------------------------------------------------------------- back to linear

fn needs_paren(m: &M) -> bool {
    match m {
        M::Text(t) => t.chars().count() > 1 && !t.chars().all(|c| c.is_ascii_digit() || c == '.'),
        M::Row(_) | M::Frac(..) | M::Func(..) | M::Nary(..) | M::Lim(..) => true,
        _ => false,
    }
}

fn arg_lin(m: &M) -> String {
    let s = to_linear(m);
    if needs_paren(m) {
        format!("({})", s)
    } else {
        s
    }
}

fn frac_part(m: &M) -> String {
    let s = to_linear(m);
    let simple = match m {
        M::Text(t) => t.chars().all(is_factor_char),
        M::Scr(..) | M::Delim(..) | M::Rad(..) | M::Acc(..) => true,
        M::Row(v) => v.iter().all(|x| match x {
            M::Text(t) => t.chars().all(is_factor_char),
            M::Scr(..) | M::Acc(..) => true,
            _ => false,
        }),
        _ => false,
    };
    if simple {
        s
    } else {
        format!("({})", s)
    }
}

/// The linear format of an equation.
pub fn to_linear(m: &M) -> String {
    match m {
        M::Row(v) => v.iter().map(to_linear).collect(),
        M::Text(t) => t.clone(),
        M::Plain(t) => t.clone(),
        M::Frac(a, b, bar) => format!("{}{}{}", frac_part(a), if *bar { "/" } else { "¦" }, frac_part(b)),
        M::Scr(b, sub, sup) => {
            let mut s = match **b {
                M::Text(ref t) if t.chars().count() == 1 => t.clone(),
                M::Plain(ref t) => t.clone(),
                _ => arg_lin(b),
            };
            if let Some(x) = sub {
                s.push('_');
                s.push_str(&arg_lin(x));
            }
            if let Some(x) = sup {
                s.push('^');
                s.push_str(&arg_lin(x));
            }
            s
        }
        M::Rad(d, b) => match d {
            Some(d) => format!("√({}&{})", to_linear(d), to_linear(b)),
            None => format!("√({})", to_linear(b)),
        },
        M::Nary(op, sub, sup, body, _) => {
            let mut s = op.to_string();
            if let Some(x) = sub {
                s.push('_');
                s.push_str(&arg_lin(x));
            }
            if let Some(x) = sup {
                s.push('^');
                s.push_str(&arg_lin(x));
            }
            s.push('▒');
            match **body {
                M::Delim(..) | M::Text(_) => s.push_str(&to_linear(body)),
                _ => s.push_str(&format!("〖{}〗", to_linear(body))),
            }
            s
        }
        M::Delim(b, e, parts) => format!("{}{}{}", b, parts.iter().map(to_linear).collect::<Vec<_>>().join("│"), e),
        M::Func(n, a) => {
            let arg = match **a {
                M::Text(_) | M::Scr(..) | M::Delim(..) => to_linear(a),
                _ => format!("〖{}〗", to_linear(a)),
            };
            format!("{}⁡{}", to_linear(n), arg)
        }
        M::Acc(c, b) => format!("{}{}", arg_lin(b), c),
        M::Bar(b, top) => format!("{}{}", if *top { '¯' } else { '▁' }, arg_lin_force(b)),
        M::Lim(b, l, below) => format!("{}{}{}", to_linear(b), if *below { '┬' } else { '┴' }, arg_lin(l)),
        M::Matrix(rows) => format!("■({})", rows.iter().map(|r| r.iter().map(to_linear).collect::<Vec<_>>().join("&")).collect::<Vec<_>>().join("@")),
        M::Group(x) => format!("〖{}〗", to_linear(x)),
    }
}

fn arg_lin_force(m: &M) -> String {
    format!("({})", to_linear(m))
}

// ---------------------------------------------------------------------- OMML

fn val<'a>(e: &'a Element, prop: &str, child: &str) -> Option<&'a str> {
    e.child(prop).and_then(|p| p.child(child)).and_then(|c| c.attr("val"))
}

fn arg_of(e: &Element, name: &str) -> M {
    e.child(name).map(read_children).unwrap_or(M::Text(String::new()))
}

fn read_children(e: &Element) -> M {
    let v: Vec<M> = e.elems().filter_map(read_omml_node).collect();
    if v.is_empty() {
        return M::Text(String::new());
    }
    merge_text(row(v))
}

fn first_char(s: Option<&str>, d: char) -> char {
    s.and_then(|x| x.chars().next()).unwrap_or(d)
}

fn read_omml_node(e: &Element) -> Option<M> {
    Some(match e.local() {
        "r" => {
            let t: String = e.elems().filter(|c| c.local() == "t").map(|c| c.text()).collect();
            let plain = e.child("rPr").and_then(|p| p.child("sty")).and_then(|s| s.attr("val")).map(|v| v == "p" || v == "b").unwrap_or(false)
                || e.child("rPr").and_then(|p| p.child("nor")).is_some();
            if plain {
                M::Plain(t)
            } else {
                M::Text(t)
            }
        }
        "f" => M::Frac(Box::new(arg_of(e, "num")), Box::new(arg_of(e, "den")), val(e, "fPr", "type") != Some("noBar")),
        "sSup" => M::Scr(Box::new(arg_of(e, "e")), None, Some(Box::new(arg_of(e, "sup")))),
        "sSub" => M::Scr(Box::new(arg_of(e, "e")), Some(Box::new(arg_of(e, "sub"))), None),
        "sSubSup" => M::Scr(Box::new(arg_of(e, "e")), Some(Box::new(arg_of(e, "sub"))), Some(Box::new(arg_of(e, "sup")))),
        "sPre" => M::Row(vec![M::Scr(Box::new(M::Text(String::new())), Some(Box::new(arg_of(e, "sub"))), Some(Box::new(arg_of(e, "sup")))), arg_of(e, "e")]),
        "rad" => {
            let hide = matches!(val(e, "radPr", "degHide"), Some("1") | Some("on") | Some("true"));
            let d = arg_of(e, "deg");
            let empty = matches!(&d, M::Text(t) if t.is_empty());
            M::Rad(if hide || empty { None } else { Some(Box::new(d)) }, Box::new(arg_of(e, "e")))
        }
        "nary" => {
            let op = first_char(val(e, "naryPr", "chr"), '∫');
            let hide = |n: &str| matches!(val(e, "naryPr", n), Some("1") | Some("on") | Some("true"));
            let und = match val(e, "naryPr", "limLoc") {
                Some("undOvr") => true,
                Some("subSup") => false,
                _ => !"∫∬∭∮∯∰".contains(op),
            };
            let sub = if hide("subHide") { None } else { Some(Box::new(arg_of(e, "sub"))) };
            let sup = if hide("supHide") { None } else { Some(Box::new(arg_of(e, "sup"))) };
            M::Nary(op, sub, sup, Box::new(arg_of(e, "e")), und)
        }
        "d" => {
            let beg = e.child("dPr").and_then(|p| p.child("begChr")).map(|c| c.attr("val").unwrap_or("").to_string()).unwrap_or_else(|| "(".into());
            let end = e.child("dPr").and_then(|p| p.child("endChr")).map(|c| c.attr("val").unwrap_or("").to_string()).unwrap_or_else(|| ")".into());
            let parts: Vec<M> = e.elems().filter(|c| c.local() == "e").map(read_children).collect();
            if beg.is_empty() && end.is_empty() && parts.len() == 1 {
                M::Group(Box::new(parts.into_iter().next().unwrap()))
            } else {
                M::Delim(beg, end, parts)
            }
        }
        "func" => M::Func(Box::new(arg_of(e, "fName")), Box::new(arg_of(e, "e"))),
        "acc" => M::Acc(first_char(val(e, "accPr", "chr"), '\u{0302}'), Box::new(arg_of(e, "e"))),
        "bar" => M::Bar(Box::new(arg_of(e, "e")), val(e, "barPr", "pos") == Some("top")),
        "limLow" => M::Lim(Box::new(arg_of(e, "e")), Box::new(arg_of(e, "lim")), true),
        "limUpp" => M::Lim(Box::new(arg_of(e, "e")), Box::new(arg_of(e, "lim")), false),
        "m" => M::Matrix(e.elems().filter(|c| c.local() == "mr").map(|r| r.elems().filter(|c| c.local() == "e").map(read_children).collect()).collect()),
        "eqArr" => M::Matrix(e.elems().filter(|c| c.local() == "e").map(|c| vec![read_children(c)]).collect()),
        "groupChr" => {
            let c = first_char(val(e, "groupChrPr", "chr"), '⏟');
            let top = val(e, "groupChrPr", "pos") == Some("top");
            M::Lim(Box::new(arg_of(e, "e")), Box::new(M::Text(c.to_string())), !top)
        }
        "box" | "borderBox" | "phant" | "oMath" => read_children(e.child("e").unwrap_or(e)),
        "rPr" | "fPr" | "ctrlPr" | "radPr" | "naryPr" | "dPr" | "funcPr" | "accPr" | "barPr" | "limLowPr" | "limUppPr"
        | "mPr" | "sSupPr" | "sSubPr" | "sSubSupPr" | "oMathParaPr" | "eqArrPr" | "groupChrPr" | "boxPr" | "argPr" => return None,
        _ => {
            let v = read_children(e);
            if matches!(&v, M::Text(t) if t.is_empty()) {
                return None;
            }
            v
        }
    })
}

/// The equation in an m:oMathPara or m:oMath element.
pub fn read_omml(e: &Element) -> M {
    if e.local() == "oMathPara" {
        let parts: Vec<M> = e.elems().filter(|c| c.local() == "oMath").map(read_children).collect();
        return match parts.len() {
            0 => M::Text(String::new()),
            1 => parts.into_iter().next().unwrap(),
            _ => M::Matrix(parts.into_iter().map(|p| vec![p]).collect()),
        };
    }
    read_children(e)
}

fn esc(s: &str) -> String {
    xml::escape_text(s)
}

fn run(t: &str, plain: bool, sz: Option<u32>) -> String {
    if t.is_empty() {
        return String::new();
    }
    let space = if t.starts_with(' ') || t.ends_with(' ') { " xml:space=\"preserve\"" } else { "" };
    let szs = sz.map(|s| format!(" sz=\"{}\"", s)).unwrap_or_default();
    if plain {
        format!("<m:r><m:rPr><m:sty m:val=\"p\"/></m:rPr><a:rPr lang=\"en-US\" b=\"0\" i=\"0\"{}><a:latin typeface=\"Cambria Math\" panose=\"02040503050406030204\" pitchFamily=\"18\" charset=\"0\"/></a:rPr><m:t{}>{}</m:t></m:r>", szs, space, esc(t))
    } else {
        format!("<m:r><a:rPr lang=\"en-US\" b=\"0\" i=\"1\"{}><a:latin typeface=\"Cambria Math\" panose=\"02040503050406030204\" pitchFamily=\"18\" charset=\"0\"/></a:rPr><m:t{}>{}</m:t></m:r>", szs, space, esc(t))
    }
}

fn ctrl(sz: Option<u32>) -> String {
    let szs = sz.map(|s| format!(" sz=\"{}\"", s)).unwrap_or_default();
    format!("<m:ctrlPr><a:rPr lang=\"en-US\" b=\"0\" i=\"1\"{}><a:latin typeface=\"Cambria Math\" panose=\"02040503050406030204\" pitchFamily=\"18\" charset=\"0\"/></a:rPr></m:ctrlPr>", szs)
}

fn w(m: &M, out: &mut String, sz: Option<u32>) {
    let arg = |name: &str, m: &M, out: &mut String| {
        out.push_str(&format!("<m:{}>", name));
        w(m, out, sz);
        out.push_str(&format!("</m:{}>", name));
    };
    match m {
        M::Row(v) => v.iter().for_each(|x| w(x, out, sz)),
        M::Text(t) => {
            // operators and numbers upright, letters italic: Office decides by the run's style;
            // writing one run is what Office does too
            out.push_str(&run(t, false, sz));
        }
        M::Plain(t) => out.push_str(&run(t, true, sz)),
        M::Frac(a, b, bar) => {
            out.push_str("<m:f><m:fPr>");
            if !*bar {
                out.push_str("<m:type m:val=\"noBar\"/>");
            }
            out.push_str(&ctrl(sz));
            out.push_str("</m:fPr>");
            arg("num", a, out);
            arg("den", b, out);
            out.push_str("</m:f>");
        }
        M::Scr(b, sub, sup) => {
            let tag = match (sub.is_some(), sup.is_some()) {
                (true, true) => "sSubSup",
                (true, false) => "sSub",
                _ => "sSup",
            };
            out.push_str(&format!("<m:{t}><m:{t}Pr>{}</m:{t}Pr>", ctrl(sz), t = tag));
            arg("e", b, out);
            if let Some(x) = sub {
                arg("sub", x, out);
            }
            if let Some(x) = sup {
                arg("sup", x, out);
            }
            if sub.is_none() && sup.is_none() {
                arg("sup", &M::Text(String::new()), out);
            }
            out.push_str(&format!("</m:{}>", tag));
        }
        M::Rad(d, b) => {
            out.push_str("<m:rad><m:radPr>");
            if d.is_none() {
                out.push_str("<m:degHide m:val=\"on\"/>");
            }
            out.push_str(&ctrl(sz));
            out.push_str("</m:radPr><m:deg>");
            if let Some(d) = d {
                w(d, out, sz);
            }
            out.push_str("</m:deg>");
            arg("e", b, out);
            out.push_str("</m:rad>");
        }
        M::Nary(op, sub, sup, body, und) => {
            out.push_str("<m:nary><m:naryPr>");
            if *op != '∫' {
                out.push_str(&format!("<m:chr m:val=\"{}\"/>", op));
            }
            out.push_str(&format!("<m:limLoc m:val=\"{}\"/>", if *und { "undOvr" } else { "subSup" }));
            if sub.is_none() {
                out.push_str("<m:subHide m:val=\"on\"/>");
            }
            if sup.is_none() {
                out.push_str("<m:supHide m:val=\"on\"/>");
            }
            out.push_str(&ctrl(sz));
            out.push_str("</m:naryPr>");
            out.push_str("<m:sub>");
            if let Some(x) = sub {
                w(x, out, sz);
            }
            out.push_str("</m:sub><m:sup>");
            if let Some(x) = sup {
                w(x, out, sz);
            }
            out.push_str("</m:sup>");
            arg("e", body, out);
            out.push_str("</m:nary>");
        }
        M::Delim(b, e, parts) => {
            out.push_str("<m:d><m:dPr>");
            if b != "(" {
                out.push_str(&format!("<m:begChr m:val=\"{}\"/>", xml::escape_attr(b)));
            }
            if e != ")" {
                out.push_str(&format!("<m:endChr m:val=\"{}\"/>", xml::escape_attr(e)));
            }
            out.push_str(&ctrl(sz));
            out.push_str("</m:dPr>");
            for p in parts {
                arg("e", p, out);
            }
            out.push_str("</m:d>");
        }
        M::Group(x) => {
            out.push_str(&format!("<m:d><m:dPr><m:begChr m:val=\"\"/><m:endChr m:val=\"\"/>{}</m:dPr>", ctrl(sz)));
            arg("e", x, out);
            out.push_str("</m:d>");
        }
        M::Func(n, a) => {
            out.push_str(&format!("<m:func><m:funcPr>{}</m:funcPr>", ctrl(sz)));
            arg("fName", n, out);
            arg("e", a, out);
            out.push_str("</m:func>");
        }
        M::Acc(c, b) => {
            out.push_str("<m:acc><m:accPr>");
            if *c != '\u{0302}' {
                out.push_str(&format!("<m:chr m:val=\"{}\"/>", c));
            }
            out.push_str(&ctrl(sz));
            out.push_str("</m:accPr>");
            arg("e", b, out);
            out.push_str("</m:acc>");
        }
        M::Bar(b, top) => {
            out.push_str("<m:bar><m:barPr>");
            if *top {
                out.push_str("<m:pos m:val=\"top\"/>");
            }
            out.push_str(&ctrl(sz));
            out.push_str("</m:barPr>");
            arg("e", b, out);
            out.push_str("</m:bar>");
        }
        M::Lim(b, l, below) => {
            let t = if *below { "limLow" } else { "limUpp" };
            out.push_str(&format!("<m:{t}><m:{t}Pr>{}</m:{t}Pr>", ctrl(sz), t = t));
            arg("e", b, out);
            arg("lim", l, out);
            out.push_str(&format!("</m:{}>", t));
        }
        M::Matrix(rows) => {
            let ncol = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
            out.push_str(&format!("<m:m><m:mPr><m:mcs><m:mc><m:mcPr><m:count m:val=\"{}\"/><m:mcJc m:val=\"center\"/></m:mcPr></m:mc></m:mcs>{}</m:mPr>", ncol, ctrl(sz)));
            for r in rows {
                out.push_str("<m:mr>");
                for k in 0..ncol {
                    arg("e", r.get(k).unwrap_or(&M::Text(String::new())), out);
                }
                out.push_str("</m:mr>");
            }
            out.push_str("</m:m>");
        }
    }
}

/// The a14:m element Office keeps an equation in (its namespaces declared on it).
pub fn write_omml(m: &M, size_pt: Option<f64>) -> String {
    let sz = size_pt.map(|s| (s * 100.0).round() as u32);
    let mut out = String::new();
    w(m, &mut out, sz);
    format!("<a14:m xmlns:a14=\"{}\"><m:oMathPara xmlns:m=\"{}\"><m:oMathParaPr><m:jc m:val=\"centerGroup\"/></m:oMathParaPr><m:oMath>{}</m:oMath></m:oMathPara></a14:m>", NS_A14, NS_M, out)
}

/// The math element in a text body, if it holds an equation.
pub fn find_math(e: &Element) -> Option<&Element> {
    for c in e.elems() {
        if c.local() == "oMathPara" || c.local() == "oMath" {
            return Some(c);
        }
        if let Some(x) = find_math(c) {
            return Some(x);
        }
    }
    None
}

/// The size (points) the equation's runs are in.
pub fn math_size(e: &Element) -> Option<f64> {
    for c in e.elems() {
        if c.local() == "rPr" && c.name.starts_with("a:") {
            if let Some(s) = c.attr("sz").and_then(|v| v.parse::<f64>().ok()) {
                return Some(s / 100.0);
            }
        }
        if let Some(s) = math_size(c) {
            return Some(s);
        }
    }
    None
}

// ---------------------------------------------------------------------- drawing

#[derive(Clone, Debug)]
pub enum Draw {
    /// text at (x, baseline y), size px, italic
    Text(f64, f64, String, f64, bool),
    Line(f64, f64, f64, f64, f64),
    /// a stroked path
    Path(Vec<(f64, f64)>, f64),
    /// a stroked cubic path: start, then (c1, c2, end) triples
    Curve((f64, f64), Vec<[(f64, f64); 3]>, f64),
}

#[derive(Clone, Debug, Default)]
pub struct Bx {
    pub w: f64,
    pub asc: f64,
    pub desc: f64,
    pub items: Vec<Draw>,
}

impl Bx {
    fn put(&mut self, b: &Bx, dx: f64, dy: f64) {
        for it in &b.items {
            self.items.push(match it {
                Draw::Text(x, y, t, s, i) => Draw::Text(x + dx, y + dy, t.clone(), *s, *i),
                Draw::Line(a, b2, c, d, l) => Draw::Line(a + dx, b2 + dy, c + dx, d + dy, *l),
                Draw::Path(p, l) => Draw::Path(p.iter().map(|(x, y)| (x + dx, y + dy)).collect(), *l),
                Draw::Curve(s, segs, l) => Draw::Curve((s.0 + dx, s.1 + dy), segs.iter().map(|t| [(t[0].0 + dx, t[0].1 + dy), (t[1].0 + dx, t[1].1 + dy), (t[2].0 + dx, t[2].1 + dy)]).collect(), *l),
            });
        }
    }
}

/// Measures text: (width, ascent, descent) in px.
pub trait Measure {
    fn text(&mut self, t: &str, size: f64, italic: bool) -> (f64, f64, f64);
}

fn hbox(parts: Vec<Bx>) -> Bx {
    let mut out = Bx::default();
    let mut x = 0.0;
    for p in parts {
        out.put(&p, x, 0.0);
        x += p.w;
        out.asc = out.asc.max(p.asc);
        out.desc = out.desc.max(p.desc);
    }
    out.w = x;
    out
}

fn text_box(ms: &mut dyn Measure, t: &str, size: f64, plain: bool) -> Bx {
    // letters italic (unless upright); digits and operators upright, with room round operators
    let mut parts = vec![];
    let mut cur = String::new();
    let mut cur_it = None::<bool>;
    let mut flush = |cur: &mut String, it: Option<bool>, parts: &mut Vec<Bx>, ms: &mut dyn Measure| {
        if cur.is_empty() {
            return;
        }
        let (w, a, d) = ms.text(cur, size, it.unwrap_or(false));
        let mut b = Bx { w, asc: a, desc: d, items: vec![Draw::Text(0.0, 0.0, cur.clone(), size, it.unwrap_or(false))] };
        // the italic letters' overhang
        if it == Some(true) {
            b.w += size * 0.04;
        }
        parts.push(b);
        cur.clear();
    };
    for c in t.chars() {
        if c == '\u{2B1A}' {
            flush(&mut cur, cur_it, &mut parts, ms);
            parts.push(empty_slot(ms, size));
            cur_it = None;
            continue;
        }
        if OPS.contains(c) && !plain && c != '|' && c != '‖' {
            flush(&mut cur, cur_it, &mut parts, ms);
            let (w, a, d) = ms.text(&c.to_string(), size, false);
            let pad = if ",;".contains(c) { size * 0.0 } else if c == ':' { size * 0.14 } else { size * 0.2 };
            let rpad = if ",;".contains(c) { size * 0.17 } else { pad };
            let b = Bx { w: w + pad + rpad, asc: a, desc: d, items: vec![Draw::Text(pad, 0.0, c.to_string(), size, false)] };
            parts.push(b);
            cur_it = None;
            continue;
        }
        let it = !plain && c.is_alphabetic() && !matches!(c, '∞' | '∂' | '…' | '⋯');
        if cur_it.is_some() && cur_it != Some(it) {
            flush(&mut cur, cur_it, &mut parts, ms);
        }
        cur_it = Some(it);
        cur.push(c);
    }
    flush(&mut cur, cur_it, &mut parts, ms);
    if parts.is_empty() {
        let (_, a, d) = ms.text("x", size, false);
        return Bx { w: 0.0, asc: a, desc: d, items: vec![] };
    }
    hbox(parts)
}

fn empty_slot(ms: &mut dyn Measure, size: f64) -> Bx {
    // a dotted box where something is to be typed
    let (_, a, _) = ms.text("x", size, false);
    let w = size * 0.55;
    let h = a * 0.75;
    let lw = (size * 0.05).max(0.7);
    Bx { w: w + size * 0.1, asc: h, desc: 0.0, items: vec![Draw::Path(vec![(size * 0.05, 0.0), (w, 0.0), (w, -h), (size * 0.05, -h), (size * 0.05, 0.0)], lw)] }
}

fn is_empty(m: &M) -> bool {
    matches!(m, M::Text(t) if t.is_empty())
}

/// Lay an equation out at a size (px).
pub fn layout(m: &M, ms: &mut dyn Measure, size: f64) -> Bx {
    let small = (size * 0.7).max(4.0);
    let axis = size * 0.27; // the maths axis above the baseline
    let lw = (size * 0.055).max(0.75);
    match m {
        M::Text(t) if t.is_empty() => empty_slot(ms, size),
        M::Text(t) => text_box(ms, t, size, false),
        M::Plain(t) => text_box(ms, t, size, true),
        M::Row(v) => hbox(v.iter().map(|x| layout(x, ms, size)).collect()),
        M::Group(x) => layout(x, ms, size),
        M::Frac(a, b, bar) => {
            let fs = size;
            let na = layout(a, ms, fs);
            let nb = layout(b, ms, fs);
            let gap = size * 0.12;
            let wd = na.w.max(nb.w) + size * 0.2;
            let mut out = Bx { w: wd + size * 0.1, ..Default::default() };
            let x0 = size * 0.05;
            out.put(&na, x0 + (wd - na.w) / 2.0, -axis - gap - na.desc - lw / 2.0);
            out.put(&nb, x0 + (wd - nb.w) / 2.0, -axis + gap + nb.asc + lw / 2.0);
            if *bar {
                out.items.push(Draw::Line(x0, -axis, x0 + wd, -axis, lw));
            }
            out.asc = axis + gap + na.desc + na.asc + lw / 2.0;
            out.desc = (-axis + gap + nb.asc + nb.desc + lw / 2.0).max(0.0);
            out
        }
        M::Scr(b, sub, sup) => {
            let base = if is_empty(b) { Bx { w: 0.0, asc: size * 0.7, desc: 0.0, items: vec![] } } else { layout(b, ms, size) };
            let mut out = Bx::default();
            out.put(&base, 0.0, 0.0);
            let x = base.w + size * 0.03;
            let mut wd: f64 = 0.0;
            out.asc = base.asc;
            out.desc = base.desc;
            if let Some(sp) = sup {
                let s = layout(sp, ms, small);
                let up = (base.asc - s.asc * 0.55).max(size * 0.38);
                out.put(&s, x, -up);
                out.asc = out.asc.max(up + s.asc);
                wd = wd.max(s.w);
            }
            if let Some(sb) = sub {
                let s = layout(sb, ms, small);
                let down = (base.desc + s.asc * 0.4).max(size * 0.2);
                out.put(&s, x, down);
                out.desc = out.desc.max(down + s.desc);
                wd = wd.max(s.w);
            }
            out.w = x + wd + size * 0.05;
            out
        }
        M::Rad(d, b) => {
            let body = layout(b, ms, size);
            let gap = size * 0.12;
            let top = body.asc + gap + lw;
            let bot = body.desc;
            let hook = size * 0.55;
            let mut out = Bx::default();
            let mut x = 0.0;
            let mut dbox = None;
            if let Some(dg) = d {
                let db = layout(dg, ms, small * 0.85);
                x = (db.w - hook * 0.4).max(0.0);
                dbox = Some(db);
            }
            let pts = vec![(x, -top * 0.42), (x + hook * 0.25, -top * 0.5), (x + hook * 0.55, bot), (x + hook, -top), (x + hook + body.w + size * 0.1, -top)];
            out.items.push(Draw::Path(pts, lw));
            if let Some(db) = dbox {
                out.put(&db, 0.0, -top * 0.5 - db.desc - size * 0.05);
                out.asc = out.asc.max(top * 0.5 + db.desc + db.asc + size * 0.05);
            }
            out.put(&body, x + hook + size * 0.05, 0.0);
            out.w = x + hook + body.w + size * 0.18;
            out.asc = out.asc.max(top + lw);
            out.desc = bot + lw;
            out
        }
        M::Nary(op, sub, sup, body, und) => {
            let integral = "∫∬∭∮∯∰".contains(*op);
            let osz = size * if integral { 1.6 } else { 1.45 };
            let (ow, oa, od) = ms.text(&op.to_string(), osz, false);
            // centre the operator on the maths axis
            let shift = (oa - od) / 2.0 - axis;
            let mut opb = Bx { w: ow, asc: oa - shift, desc: od + shift, items: vec![Draw::Text(0.0, shift, op.to_string(), osz, false)] };
            let subb = sub.as_ref().map(|x| layout(x, ms, small));
            let supb = sup.as_ref().map(|x| layout(x, ms, small));
            let mut out = Bx::default();
            let mut x;
            if *und {
                let wd = [opb.w, subb.as_ref().map(|b| b.w).unwrap_or(0.0), supb.as_ref().map(|b| b.w).unwrap_or(0.0)].iter().cloned().fold(0.0, f64::max);
                let ox = (wd - opb.w) / 2.0;
                out.put(&opb, ox, 0.0);
                out.asc = opb.asc;
                out.desc = opb.desc;
                if let Some(s) = &supb {
                    let y = -opb.asc - size * 0.08 - s.desc;
                    out.put(s, (wd - s.w) / 2.0, y);
                    out.asc = out.asc.max(-y + s.asc);
                }
                if let Some(s) = &subb {
                    let y = opb.desc + size * 0.08 + s.asc;
                    out.put(s, (wd - s.w) / 2.0, y);
                    out.desc = out.desc.max(y + s.desc);
                }
                x = wd + size * 0.12;
            } else {
                opb.w += size * 0.05;
                out.put(&opb, 0.0, 0.0);
                out.asc = opb.asc;
                out.desc = opb.desc;
                let sx = opb.w;
                let mut wd: f64 = 0.0;
                if let Some(s) = &supb {
                    let y = -opb.asc + s.asc * 0.9;
                    out.put(s, sx, y);
                    wd = wd.max(s.w);
                    out.asc = out.asc.max(-y + s.asc);
                }
                if let Some(s) = &subb {
                    let y = opb.desc - s.desc * 0.4;
                    out.put(s, sx - size * 0.15, y);
                    wd = wd.max(s.w - size * 0.15);
                    out.desc = out.desc.max(y + s.desc);
                }
                x = sx + wd + size * 0.12;
            }
            let bb = layout(body, ms, size);
            out.put(&bb, x, 0.0);
            x += bb.w;
            out.asc = out.asc.max(bb.asc);
            out.desc = out.desc.max(bb.desc);
            out.w = x;
            out
        }
        M::Delim(b, e, parts) => {
            let mut inner_parts: Vec<Bx> = Vec::new();
            for (k, p) in parts.iter().enumerate() {
                if k > 0 {
                    // the separator, drawn as a bar later
                    inner_parts.push(Bx { w: size * 0.3, asc: 0.0, desc: 0.0, items: vec![Draw::Text(f64::NAN, 0.0, "│".into(), 0.0, false)] });
                }
                inner_parts.push(layout(p, ms, size));
            }
            let inner = hbox(inner_parts);
            let (_, xa, xd) = ms.text("(", size, false);
            // brackets reach as far above the axis as below it
            let half = (inner.asc - axis).max(inner.desc + axis).max((xa + xd) / 2.0);
            let top = -axis - half;
            let bot = -axis + half;
            let bw = size * 0.32 * (1.0 + ((half * 2.0) / (xa + xd) - 1.0).max(0.0) * 0.15).min(1.6);
            let mut out = Bx::default();
            let mut x = 0.0;
            if !b.is_empty() {
                delim_path(&mut out, b, x, top, bot, bw, lw, true);
                x += bw;
            }
            let mut ib = inner.clone();
            // the separators span the brackets' height
            ib.items = ib.items.into_iter().map(|it| match it {
                Draw::Text(xx, _, ref t, _, _) if xx.is_nan() && t == "│" => Draw::Line(0.0, 0.0, 0.0, 0.0, -1.0),
                o => o,
            }).collect();
            let mut sx = 0.0;
            let mut seps = vec![];
            for (k, p) in parts.iter().enumerate() {
                let _ = p;
                if k > 0 {
                    seps.push(sx);
                    sx += size * 0.3;
                }
                sx += layout(&parts[k], ms, size).w;
            }
            ib.items.retain(|it| !matches!(it, Draw::Line(_, _, _, _, l) if *l < 0.0));
            out.put(&ib, x, 0.0);
            for s in seps {
                let cx = x + s + size * 0.15;
                out.items.push(Draw::Line(cx, top, cx, bot, lw));
            }
            x += inner.w;
            if !e.is_empty() {
                delim_path(&mut out, e, x, top, bot, bw, lw, false);
                x += bw;
            }
            out.w = x;
            out.asc = (-top).max(inner.asc);
            out.desc = bot.max(inner.desc);
            out
        }
        M::Func(n, a) => {
            let nb = layout(n, ms, size);
            let ab = layout(a, ms, size);
            let gap = if matches!(**a, M::Delim(..)) { 0.0 } else { size * 0.17 };
            let lead = size * 0.17;
            let mut out = Bx::default();
            out.put(&nb, lead, 0.0);
            out.put(&ab, lead + nb.w + gap, 0.0);
            out.w = lead + nb.w + gap + ab.w + size * 0.1;
            out.asc = nb.asc.max(ab.asc);
            out.desc = nb.desc.max(ab.desc);
            out
        }
        M::Acc(c, b) => {
            let bb = layout(b, ms, size);
            let mut out = Bx::default();
            out.put(&bb, 0.0, 0.0);
            let y = -bb.asc - size * 0.08;
            let cx = bb.w / 2.0 + size * 0.04;
            let hw = (bb.w / 2.0).min(size * 0.25).max(size * 0.14);
            match c {
                '\u{0302}' => out.items.push(Draw::Path(vec![(cx - hw, y), (cx, y - size * 0.15), (cx + hw, y)], lw)),
                '\u{030C}' => out.items.push(Draw::Path(vec![(cx - hw, y - size * 0.15), (cx, y), (cx + hw, y - size * 0.15)], lw)),
                '\u{0303}' => out.items.push(Draw::Curve((cx - hw, y - size * 0.03), vec![[(cx - hw / 2.0, y - size * 0.2), (cx + hw / 2.0, y + size * 0.1), (cx + hw, y - size * 0.1)]], lw)),
                '\u{0307}' => out.items.push(Draw::Path(vec![(cx - lw, y - size * 0.06), (cx + lw, y - size * 0.06)], lw * 2.2)),
                '\u{0308}' => {
                    out.items.push(Draw::Path(vec![(cx - size * 0.12 - lw, y - size * 0.06), (cx - size * 0.12 + lw, y - size * 0.06)], lw * 2.2));
                    out.items.push(Draw::Path(vec![(cx + size * 0.12 - lw, y - size * 0.06), (cx + size * 0.12 + lw, y - size * 0.06)], lw * 2.2));
                }
                '\u{20D7}' => {
                    let (l, r) = (cx - bb.w / 2.0, cx + bb.w / 2.0);
                    out.items.push(Draw::Line(l, y - size * 0.08, r, y - size * 0.08, lw));
                    out.items.push(Draw::Path(vec![(r - size * 0.12, y - size * 0.16), (r, y - size * 0.08), (r - size * 0.12, y)], lw));
                }
                '\u{0305}' | '\u{00AF}' => out.items.push(Draw::Line(0.0, y - size * 0.04, bb.w, y - size * 0.04, lw)),
                _ => {
                    let (w2, _, _) = ms.text(&c.to_string(), size, false);
                    out.items.push(Draw::Text(cx - w2 / 2.0, y + size * 0.6, format!("\u{25CC}{}", c), size, false));
                }
            }
            out.w = bb.w;
            out.asc = bb.asc + size * 0.28;
            out.desc = bb.desc;
            out
        }
        M::Bar(b, top) => {
            let bb = layout(b, ms, size);
            let mut out = Bx::default();
            out.put(&bb, 0.0, 0.0);
            if *top {
                let y = -bb.asc - size * 0.08;
                out.items.push(Draw::Line(0.0, y, bb.w, y, lw));
                out.asc = bb.asc + size * 0.12;
                out.desc = bb.desc;
            } else {
                let y = bb.desc + size * 0.1;
                out.items.push(Draw::Line(0.0, y, bb.w, y, lw));
                out.asc = bb.asc;
                out.desc = bb.desc + size * 0.14;
            }
            out.w = bb.w;
            out
        }
        M::Lim(b, l, below) => {
            let bb = layout(b, ms, size);
            let lb = layout(l, ms, small);
            let wd = bb.w.max(lb.w);
            let mut out = Bx::default();
            out.put(&bb, (wd - bb.w) / 2.0, 0.0);
            if *below {
                let y = bb.desc + size * 0.05 + lb.asc;
                out.put(&lb, (wd - lb.w) / 2.0, y);
                out.asc = bb.asc;
                out.desc = y + lb.desc;
            } else {
                let y = -bb.asc - size * 0.05 - lb.desc;
                out.put(&lb, (wd - lb.w) / 2.0, y);
                out.asc = -y + lb.asc;
                out.desc = bb.desc;
            }
            out.w = wd;
            out
        }
        M::Matrix(rows) => {
            let cells: Vec<Vec<Bx>> = rows.iter().map(|r| r.iter().map(|c| layout(c, ms, size)).collect()).collect();
            let ncol = cells.iter().map(|r| r.len()).max().unwrap_or(0);
            let colw: Vec<f64> = (0..ncol).map(|k| cells.iter().filter_map(|r| r.get(k)).map(|b| b.w).fold(0.0, f64::max)).collect();
            let gapx = size * 0.6;
            let gapy = size * 0.25;
            let heights: Vec<(f64, f64)> = cells.iter().map(|r| (r.iter().map(|b| b.asc).fold(size * 0.7, f64::max), r.iter().map(|b| b.desc).fold(size * 0.2, f64::max))).collect();
            let total: f64 = heights.iter().map(|(a, d)| a + d).sum::<f64>() + gapy * (heights.len().saturating_sub(1)) as f64;
            let mut out = Bx::default();
            let mut y = -axis - total / 2.0;
            for (ri, r) in cells.iter().enumerate() {
                let (a, d) = heights[ri];
                let mut x = 0.0;
                for (k, b) in r.iter().enumerate() {
                    out.put(b, x + (colw[k] - b.w) / 2.0, y + a);
                    x += colw[k] + gapx;
                }
                y += a + d + gapy;
            }
            out.w = colw.iter().sum::<f64>() + gapx * (ncol.saturating_sub(1)) as f64;
            out.asc = axis + total / 2.0;
            out.desc = (total / 2.0 - axis).max(0.0);
            out
        }
    }
}

fn delim_path(out: &mut Bx, ch: &str, x: f64, top: f64, bot: f64, bw: f64, lw: f64, left: bool) {
    let h = bot - top;
    let (a, b) = if left { (x + bw * 0.8, x + bw * 0.25) } else { (x + bw * 0.2, x + bw * 0.75) };
    let mid = (top + bot) / 2.0;
    match ch {
        "(" | ")" => out.items.push(Draw::Curve((a, top), vec![[(b - (a - b) * 0.15, top + h * 0.25), (b - (a - b) * 0.15, bot - h * 0.25), (a, bot)]], lw * 1.15)),
        "[" | "]" | "⌊" | "⌋" | "⌈" | "⌉" => {
            let (t, bo) = match ch {
                "⌊" | "⌋" => (false, true),
                "⌈" | "⌉" => (true, false),
                _ => (true, true),
            };
            let mut p = vec![];
            if t {
                p.push((a, top));
            }
            p.push((b, top));
            p.push((b, bot));
            if bo {
                p.push((a, bot));
            }
            out.items.push(Draw::Path(p, lw));
        }
        "{" | "}" => {
            let tip = if left { x + bw * 0.1 } else { x + bw * 0.9 };
            let c = (a + b) / 2.0;
            out.items.push(Draw::Curve((a, top), vec![
                [(c, top), (c, top + h * 0.05), (c, top + h * 0.2)],
                [(c, mid - h * 0.08), (c, mid), (tip, mid)],
                [(c, mid), (c, mid + h * 0.08), (c, bot - h * 0.2)],
                [(c, bot - h * 0.05), (c, bot), (a, bot)],
            ], lw));
        }
        "⟨" | "⟩" | "〈" | "〉" => out.items.push(Draw::Path(vec![(a, top), (b, mid), (a, bot)], lw)),
        "|" => out.items.push(Draw::Line(x + bw / 2.0, top, x + bw / 2.0, bot, lw)),
        "‖" => {
            out.items.push(Draw::Line(x + bw * 0.35, top, x + bw * 0.35, bot, lw));
            out.items.push(Draw::Line(x + bw * 0.65, top, x + bw * 0.65, bot, lw));
        }
        other => out.items.push(Draw::Text(x, (top + bot) / 2.0 + bw, other.to_string(), h, false)),
    }
}

// ---------------------------------------------------------------------- painting with cairo/pango

pub struct PangoMeasure {
    pub layout: *mut crate::ffi::PangoLayout,
    fonts: std::collections::HashMap<(u64, bool), crate::layout::FontDesc>,
}

impl PangoMeasure {
    pub unsafe fn new(cr: *mut crate::ffi::cairo_t) -> PangoMeasure {
        PangoMeasure { layout: crate::ffi::pango_cairo_create_layout(cr), fonts: Default::default() }
    }

    unsafe fn set(&mut self, t: &str, size: f64, italic: bool) {
        use crate::ffi::*;
        let key = ((size * 100.0).round() as u64, italic);
        let d = self.fonts.entry(key).or_insert_with(|| {
            let f = crate::layout::FontSpec { family: FONT.into(), size: size * 72.0 / 96.0, bold: false, italic, underline: 0, strike: false, vert: 0, color: 0 };
            crate::layout::FontDesc::new(&f, 1.0)
        }).0;
        pango_layout_set_font_description(self.layout, d);
        pango_layout_set_attributes(self.layout, std::ptr::null_mut());
        pango_layout_set_width(self.layout, -1);
        let c = std::ffi::CString::new(t.replace('\0', "")).unwrap_or_default();
        pango_layout_set_text(self.layout, c.as_ptr(), -1);
    }
}

impl Drop for PangoMeasure {
    fn drop(&mut self) {
        self.fonts.clear();
        unsafe { crate::ffi::g_object_unref(self.layout as *mut _) }
    }
}

impl Measure for PangoMeasure {
    fn text(&mut self, t: &str, size: f64, italic: bool) -> (f64, f64, f64) {
        unsafe {
            use crate::ffi::*;
            self.set(t, size, italic);
            let mut ink = PangoRectangle::default();
            let mut lg = PangoRectangle::default();
            pango_layout_get_extents(self.layout, &mut ink, &mut lg);
            let base = pango_layout_get_baseline(self.layout) as f64 / PANGO_SCALE;
            let h = lg.height as f64 / PANGO_SCALE;
            // the ink's height for the ascent, so brackets and bars hug the text
            let ia = base - ink.y as f64 / PANGO_SCALE;
            let id = (ink.y + ink.height) as f64 / PANGO_SCALE - base;
            let asc = ia.max(size * 0.5).min(base);
            let desc = id.max(0.0).min(h - base);
            (lg.width as f64 / PANGO_SCALE, asc, desc)
        }
    }
}

/// Draw a laid-out equation with its baseline's left end at (x, y).
pub unsafe fn paint(cr: *mut crate::ffi::cairo_t, ms: &mut PangoMeasure, b: &Bx, x: f64, y: f64, color: u32) {
    use crate::ffi::*;
    let (r, g, bl) = (((color >> 16) & 255) as f64 / 255.0, ((color >> 8) & 255) as f64 / 255.0, (color & 255) as f64 / 255.0);
    cairo_set_source_rgb(cr, r, g, bl);
    cairo_set_line_cap(cr, 0);
    cairo_set_line_join(cr, 1);
    for it in &b.items {
        match it {
            Draw::Text(tx, ty, t, size, italic) => {
                if tx.is_nan() {
                    continue;
                }
                ms.set(t, *size, *italic);
                let base = pango_layout_get_baseline(ms.layout) as f64 / PANGO_SCALE;
                cairo_move_to(cr, x + tx, y + ty - base);
                pango_cairo_show_layout(cr, ms.layout);
            }
            Draw::Line(x1, y1, x2, y2, lw) => {
                cairo_set_line_width(cr, *lw);
                cairo_move_to(cr, x + x1, y + y1);
                cairo_line_to(cr, x + x2, y + y2);
                cairo_stroke(cr);
            }
            Draw::Path(pts, lw) => {
                cairo_set_line_width(cr, *lw);
                for (k, (px, py)) in pts.iter().enumerate() {
                    if k == 0 {
                        cairo_move_to(cr, x + px, y + py);
                    } else {
                        cairo_line_to(cr, x + px, y + py);
                    }
                }
                cairo_stroke(cr);
            }
            Draw::Curve(s, segs, lw) => {
                cairo_set_line_width(cr, *lw);
                cairo_move_to(cr, x + s.0, y + s.1);
                for t in segs {
                    cairo_curve_to(cr, x + t[0].0, y + t[0].1, x + t[1].0, y + t[1].1, x + t[2].0, y + t[2].1);
                }
                cairo_stroke(cr);
            }
        }
    }
}

/// The equations Insert › Equation offers ready-made, as Office does.
pub const BUILT_IN: &[(&str, &str)] = &[
    ("Area of Circle", "A=πr^2"),
    ("Binomial Theorem", "(x+a)^n=∑_(k=0)^n▒〖(n¦k) x^k a^(n−k)〗"),
    ("Expansion of a Sum", "(1+x)^n=1+nx/1!+(n(n−1) x^2)/2!+⋯"),
    ("Fourier Series", "f(x)=a_0+∑_(n=1)^∞▒(a_n  cos⁡〖nπx/L〗+b_n  sin⁡〖nπx/L〗 )"),
    ("Pythagorean Theorem", "a^2+b^2=c^2"),
    ("Quadratic Formula", "x=(−b±√(b^2−4ac))/2a"),
    ("Taylor Expansion", "e^x=1+x/1!+x^2/2!+x^3/3!+⋯,    −∞<x<∞"),
    ("Trig Identity 1", "sin⁡α±sin⁡β=2 sin⁡〖1/2〗 (α±β)  cos⁡〖1/2〗 (α∓β)"),
    ("Trig Identity 2", "cos⁡α+cos⁡β=2 cos⁡〖1/2〗 (α+β)  cos⁡〖1/2〗 (α−β)"),
];

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;
    impl Measure for Fixed {
        fn text(&mut self, t: &str, size: f64, _: bool) -> (f64, f64, f64) {
            (t.chars().count() as f64 * size * 0.5, size * 0.7, size * 0.2)
        }
    }

    #[test]
    fn linear_to_structures() {
        let q = parse_linear("x=(−b±√(b^2−4ac))/2a");
        match &q {
            M::Row(v) => match &v[2] {
                M::Frac(n, d, true) => {
                    assert!(matches!(**d, M::Text(ref t) if t == "2a"), "{:?}", d);
                    assert!(matches!(**n, M::Row(_)), "{:?}", n);
                }
                o => panic!("{:?}", o),
            },
            o => panic!("{:?}", o),
        }
        assert!(matches!(parse_linear("x^2"), M::Scr(_, None, Some(_))));
        assert!(matches!(parse_linear("x_i^2"), M::Scr(_, Some(_), Some(_))));
        assert!(matches!(parse_linear("\\frac{a}{b}"), M::Frac(_, _, true)));
        assert!(matches!(parse_linear("\\sqrt[3]{x}"), M::Rad(Some(_), _)));
        assert!(matches!(parse_linear("√(3&x)"), M::Rad(Some(_), _)));
        assert!(matches!(parse_linear("∑_(i=1)^n▒x_i"), M::Nary('∑', Some(_), Some(_), _, true)));
        assert!(matches!(parse_linear("\\int_0^1 f(x)dx"), M::Nary('∫', Some(_), Some(_), _, false)));
        assert!(matches!(parse_linear("■(a&b@c&d)"), M::Matrix(ref r) if r.len() == 2 && r[0].len() == 2));
        match parse_linear("sin⁡x") {
            M::Func(n, _) => assert!(matches!(*n, M::Plain(ref t) if t == "sin")),
            o => panic!("{:?}", o),
        }
        match parse_linear("lim┬(x→0)⁡〖sin x/x〗") {
            M::Func(n, _) => assert!(matches!(*n, M::Lim(..))),
            o => panic!("{:?}", o),
        }
        assert!(matches!(parse_linear("\\alpha+\\beta"), M::Row(_)));
        assert!(matches!(parse_linear("{a,b}"), M::Delim(ref b, ref e, _) if b == "{" && e == "}"));
        assert!(matches!(parse_linear("x^{a+b}"), M::Scr(_, None, Some(ref s)) if matches!(**s, M::Row(_))));
        assert_eq!(flat_text(&parse_linear("\\alpha+\\beta")), "α+β");
    }

    #[test]
    fn every_built_in_survives_omml_and_back() {
        for (name, lin) in BUILT_IN {
            let m = parse_linear(lin);
            let x = write_omml(&m, None);
            let (_, e) = xml::parse(&x).unwrap();
            let back = read_omml(find_math(&e).unwrap());
            assert_eq!(to_linear(&back), to_linear(&m), "{}", name);
            // and the linear form reads back the same
            let again = parse_linear(&to_linear(&m));
            assert_eq!(to_linear(&again), to_linear(&m), "{}", name);
            let b = layout(&m, &mut Fixed, 20.0);
            assert!(b.w > 20.0 && b.asc > 5.0, "{}", name);
        }
    }

    #[test]
    fn reads_office_omml() {
        let x = "<a14:m xmlns:a14=\"x\"><m:oMathPara xmlns:m=\"y\"><m:oMath><m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f><m:r><m:t>+</m:t></m:r><m:sSup><m:e><m:r><m:t>x</m:t></m:r></m:e><m:sup><m:r><m:t>2</m:t></m:r></m:sup></m:sSup></m:oMath></m:oMathPara></a14:m>";
        let (_, e) = xml::parse(x).unwrap();
        let m = read_omml(find_math(&e).unwrap());
        assert_eq!(to_linear(&m), "a/b+x^2");
    }
}
