//! Cell and formula values.

use std::fmt;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum XlErr {
    Null,
    Div0,
    Value,
    Ref,
    Name,
    Num,
    NA,
    GettingData,
    Spill,
    Calc,
    Field,
    Blocked,
    Connect,
    Busy,
    Unknown,
}

impl XlErr {
    pub fn text(self) -> &'static str {
        match self {
            XlErr::Null => "#NULL!",
            XlErr::Div0 => "#DIV/0!",
            XlErr::Value => "#VALUE!",
            XlErr::Ref => "#REF!",
            XlErr::Name => "#NAME?",
            XlErr::Num => "#NUM!",
            XlErr::NA => "#N/A",
            XlErr::GettingData => "#GETTING_DATA",
            XlErr::Spill => "#SPILL!",
            XlErr::Calc => "#CALC!",
            XlErr::Field => "#FIELD!",
            XlErr::Blocked => "#BLOCKED!",
            XlErr::Connect => "#CONNECT!",
            XlErr::Busy => "#BUSY!",
            XlErr::Unknown => "#UNKNOWN!",
        }
    }
    pub fn parse(s: &str) -> Option<XlErr> {
        let u = s.to_ascii_uppercase();
        Some(match u.as_str() {
            "#NULL!" => XlErr::Null,
            "#DIV/0!" => XlErr::Div0,
            "#VALUE!" => XlErr::Value,
            "#REF!" => XlErr::Ref,
            "#NAME?" => XlErr::Name,
            "#NUM!" => XlErr::Num,
            "#N/A" => XlErr::NA,
            "#GETTING_DATA" => XlErr::GettingData,
            "#SPILL!" => XlErr::Spill,
            "#CALC!" => XlErr::Calc,
            "#FIELD!" => XlErr::Field,
            "#BLOCKED!" => XlErr::Blocked,
            "#CONNECT!" => XlErr::Connect,
            "#BUSY!" => XlErr::Busy,
            "#UNKNOWN!" => XlErr::Unknown,
            _ => return None,
        })
    }
    /// ERROR.TYPE number
    pub fn code(self) -> f64 {
        match self {
            XlErr::Null => 1.0,
            XlErr::Div0 => 2.0,
            XlErr::Value => 3.0,
            XlErr::Ref => 4.0,
            XlErr::Name => 5.0,
            XlErr::Num => 6.0,
            XlErr::NA => 7.0,
            XlErr::GettingData => 8.0,
            XlErr::Spill => 9.0,
            XlErr::Connect => 10.0,
            XlErr::Blocked => 11.0,
            XlErr::Unknown => 12.0,
            XlErr::Field => 13.0,
            XlErr::Calc => 14.0,
            XlErr::Busy => 15.0,
        }
    }
    pub const ALL: [XlErr; 15] = [
        XlErr::Null, XlErr::Div0, XlErr::Value, XlErr::Ref, XlErr::Name, XlErr::Num, XlErr::NA,
        XlErr::GettingData, XlErr::Spill, XlErr::Calc, XlErr::Field, XlErr::Blocked, XlErr::Connect,
        XlErr::Busy, XlErr::Unknown,
    ];
}

#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<Value>,
}

impl Array {
    pub fn new(rows: usize, cols: usize, data: Vec<Value>) -> Self {
        debug_assert_eq!(rows * cols, data.len());
        Array { rows, cols, data }
    }
    pub fn filled(rows: usize, cols: usize, v: Value) -> Self {
        Array { rows, cols, data: vec![v; rows * cols] }
    }
    pub fn get(&self, r: usize, c: usize) -> &Value {
        &self.data[r * self.cols + c]
    }
    /// Element with broadcasting of single rows/columns (as array arithmetic does);
    /// out of range gives #N/A.
    pub fn get_b(&self, r: usize, c: usize) -> Value {
        let rr = if self.rows == 1 { 0 } else { r };
        let cc = if self.cols == 1 { 0 } else { c };
        if rr < self.rows && cc < self.cols {
            self.data[rr * self.cols + cc].clone()
        } else {
            Value::Err(XlErr::NA)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Value {
    #[default]
    Empty,
    Num(f64),
    Str(Rc<str>),
    Bool(bool),
    Err(XlErr),
    Arr(Rc<Array>),
}

impl Value {
    pub fn str(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }
    pub fn string(s: String) -> Value {
        Value::Str(Rc::from(s.as_str()))
    }
    pub fn num(v: f64) -> Value {
        if v.is_finite() {
            Value::Num(if v == 0.0 { 0.0 } else { v })
        } else {
            Value::Err(XlErr::Num)
        }
    }
    pub fn is_empty(&self) -> bool {
        matches!(self, Value::Empty)
    }
    pub fn is_err(&self) -> bool {
        matches!(self, Value::Err(_))
    }
    pub fn type_rank(&self) -> u8 {
        // sort order of kinds: numbers < text < logicals < errors < blanks
        match self {
            Value::Num(_) => 0,
            Value::Str(_) => 1,
            Value::Bool(_) => 2,
            Value::Err(_) => 3,
            Value::Empty => 4,
            Value::Arr(_) => 5,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Empty => Ok(()),
            Value::Num(n) => write!(f, "{}", crate::numfmt::num_to_text(*n)),
            Value::Str(s) => write!(f, "{s}"),
            Value::Bool(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            Value::Err(e) => write!(f, "{}", e.text()),
            Value::Arr(a) => write!(f, "{}", array_text(a)),
        }
    }
}

/// {1,2;3,4} as the formula bar shows array constants.
pub fn array_text(a: &Array) -> String {
    let mut s = String::from("{");
    for r in 0..a.rows {
        if r > 0 {
            s.push(';');
        }
        for c in 0..a.cols {
            if c > 0 {
                s.push(',');
            }
            match a.get(r, c) {
                Value::Str(t) => {
                    s.push('"');
                    s.push_str(&t.replace('"', "\"\""));
                    s.push('"');
                }
                v => s.push_str(&v.to_string()),
            }
        }
    }
    s.push('}');
    s
}
