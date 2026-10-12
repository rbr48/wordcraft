//! Table formulas (Table Layout › Data › Formula): `=SUM(ABOVE)`, cell references and ranges
//! (`=SUM(B2:B4)`, `=A1*2`), arithmetic, and the functions Word offers in tables. A formula field
//! is re-evaluated by Update Field (F9), so its result follows the cells it reads.
//!
//! Cell references count rows from 1 at the top and columns from A at the left, by grid column.
//! ABOVE, LEFT, RIGHT and BELOW sum the first run of numbers in that direction, as Word does: the
//! cells next to the formula's cell are skipped while they hold no number (empty or text), then
//! the numbers are summed up to the next cell that holds none. A function needs its arguments:
//! `=SUM()` is a syntax error, as it is in Word.

use wordcraft_doc::table::{MAX_COLS, MAX_ROWS};
use wordcraft_doc::{Cell, Table};

use crate::CmdError;

/// The longest formula accepted, in bytes.
pub const MAX_FORMULA: usize = 512;
/// The deepest nesting of parentheses, function calls and unary signs.
const MAX_DEPTH: usize = 32;

/// Evaluate `formula` (with or without its leading `=`) for the cell in row `r`, cell index `c`.
pub fn evaluate(t: &Table, r: usize, c: usize, formula: &str) -> Result<f64, CmdError> {
    if formula.len() > MAX_FORMULA {
        return Err(syntax(format!("the formula is longer than {MAX_FORMULA} characters")));
    }
    let body = formula.trim();
    let body = body.strip_prefix('=').unwrap_or(body);
    let mut parser = Parser { toks: tokenize(body)?, pos: 0, depth: 0 };
    let expr = parser.expr()?;
    if parser.pos != parser.toks.len() {
        return Err(syntax("unexpected text after the formula".into()));
    }
    let here = Here { t, r, c, g: t.grid_col(r, c) };
    match eval(&expr, &here)? {
        Val::Num(x) => finite(x),
        Val::List(_) => Err(syntax("a range or ABOVE/LEFT/RIGHT/BELOW needs a function, such as SUM".into())),
    }
}

/// The text a formula result shows: a whole number as is, otherwise two decimals.
pub fn format_result(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 { format!("{}", v as i64) } else { format!("{v:.2}") }
}

/// A cell's number as a formula or a sort key reads it: surrounding spaces, currency signs,
/// thousands separators and `%` are ignored, and accounting negatives in parentheses count as
/// negative. Anything else (letters included) makes the cell text, not a number.
pub fn parse_number(text: &str) -> Option<f64> {
    let t = text.trim();
    let (t, negative) = match t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        Some(inner) => (inner, true),
        None => (t, false),
    };
    let cleaned: String = t.chars().filter(|c| !matches!(c, ',' | '$' | '€' | '£' | '¥' | '৳' | '%') && !c.is_whitespace()).collect();
    if cleaned.is_empty() || !cleaned.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')) {
        return None;
    }
    let x = cleaned.parse::<f64>().ok()?;
    let x = if negative { -x } else { x };
    x.is_finite().then_some(x)
}

/// The number in a cell, if its text is one.
pub(crate) fn cell_number(c: &Cell) -> Option<f64> {
    parse_number(&super::table::cell_text(c))
}

fn syntax(msg: String) -> CmdError {
    CmdError::Failed(format!("syntax error: {msg}"))
}

fn finite(x: f64) -> Result<f64, CmdError> {
    if x.is_finite() { Ok(x) } else { Err(CmdError::Failed("the result is not a number".into())) }
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    /// A name: a function, a keyword or a cell reference (letters then digits), upper-cased.
    Name(String),
    Op(char),
}

fn tokenize(s: &str) -> Result<Vec<Tok>, CmdError> {
    let mut out = Vec::new();
    let mut chars = s.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
        } else if ch.is_ascii_digit() || ch == '.' {
            let mut lit = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() || d == '.' {
                    lit.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            let x = lit.parse::<f64>().map_err(|_| syntax(format!("`{lit}` isn't a number")))?;
            out.push(Tok::Num(x));
        } else if ch.is_ascii_alphabetic() {
            let mut name = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_alphanumeric() {
                    name.push(d.to_ascii_uppercase());
                    chars.next();
                } else {
                    break;
                }
            }
            out.push(Tok::Name(name));
        } else if matches!(ch, '+' | '-' | '*' | '/' | '^' | '(' | ')' | ',' | ':') {
            out.push(Tok::Op(ch));
            chars.next();
        } else {
            return Err(syntax(format!("`{ch}` isn't part of a formula")));
        }
    }
    Ok(out)
}

/// The keywords that name the cells next to the formula's cell.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Side {
    Above,
    Left,
    Right,
    Below,
}

#[derive(Clone, Debug)]
enum Expr {
    Num(f64),
    /// A cell: (row, grid column).
    Cell(usize, usize, String),
    /// A rectangle of cells: (row0, grid col0, row1, grid col1), inclusive.
    Range(usize, usize, usize, usize),
    Side(Side),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek_op(&self, c: char) -> bool {
        matches!(self.toks.get(self.pos), Some(Tok::Op(x)) if *x == c)
    }

    fn eat_op(&mut self, c: char) -> bool {
        let hit = self.peek_op(c);
        if hit {
            self.pos += 1;
        }
        hit
    }

    fn enter(&mut self) -> Result<(), CmdError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH { Err(syntax("the formula is nested too deeply".into())) } else { Ok(()) }
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn expr(&mut self) -> Result<Expr, CmdError> {
        let mut lhs = self.term()?;
        loop {
            let op = if self.eat_op('+') {
                '+'
            } else if self.eat_op('-') {
                '-'
            } else {
                return Ok(lhs);
            };
            let rhs = self.term()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
    }

    fn term(&mut self) -> Result<Expr, CmdError> {
        let mut lhs = self.power()?;
        loop {
            let op = if self.eat_op('*') {
                '*'
            } else if self.eat_op('/') {
                '/'
            } else {
                return Ok(lhs);
            };
            let rhs = self.power()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
    }

    fn power(&mut self) -> Result<Expr, CmdError> {
        let base = self.unary()?;
        if self.eat_op('^') {
            let exp = self.power()?;
            return Ok(Expr::Bin('^', Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn unary(&mut self) -> Result<Expr, CmdError> {
        if self.eat_op('-') {
            self.enter()?;
            let inner = self.unary()?;
            self.leave();
            return Ok(Expr::Neg(Box::new(inner)));
        }
        if self.eat_op('+') {
            return self.unary();
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<Expr, CmdError> {
        let Some(tok) = self.toks.get(self.pos).cloned() else {
            return Err(syntax("the formula ends too soon".into()));
        };
        self.pos += 1;
        match tok {
            Tok::Num(x) => Ok(Expr::Num(x)),
            Tok::Op('(') => {
                self.enter()?;
                let inner = self.expr()?;
                if !self.eat_op(')') {
                    return Err(syntax("a `(` is missing its `)`".into()));
                }
                self.leave();
                Ok(inner)
            }
            Tok::Name(name) => {
                if self.eat_op('(') {
                    self.enter()?;
                    let mut args = Vec::new();
                    if !self.eat_op(')') {
                        loop {
                            args.push(self.expr()?);
                            if self.eat_op(')') {
                                break;
                            }
                            if !self.eat_op(',') {
                                return Err(syntax(format!("`{name}` needs `,` or `)` between its arguments")));
                            }
                        }
                    }
                    self.leave();
                    return Ok(Expr::Call(name, args));
                }
                let side = match name.as_str() {
                    "ABOVE" => Some(Side::Above),
                    "LEFT" => Some(Side::Left),
                    "RIGHT" => Some(Side::Right),
                    "BELOW" => Some(Side::Below),
                    _ => None,
                };
                if let Some(side) = side {
                    return Ok(Expr::Side(side));
                }
                let (row, col) = cell_ref(&name).ok_or_else(|| syntax(format!("`{name}` isn't a cell reference or a function")))?;
                if self.eat_op(':') {
                    let Some(Tok::Name(other)) = self.toks.get(self.pos).cloned() else {
                        return Err(syntax("a range ends with a cell reference, such as B4".into()));
                    };
                    self.pos += 1;
                    let (row2, col2) = cell_ref(&other).ok_or_else(|| syntax(format!("`{other}` isn't a cell reference")))?;
                    return Ok(Expr::Range(row.min(row2), col.min(col2), row.max(row2), col.max(col2)));
                }
                Ok(Expr::Cell(row, col, name))
            }
            Tok::Op(c) => Err(syntax(format!("`{c}` is in the wrong place"))),
        }
    }
}

/// `B4` → (row 3, column 1): the row counts from 1 and the column from A.
fn cell_ref(name: &str) -> Option<(usize, usize)> {
    let letters: String = name.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let digits = name.get(letters.len()..)?;
    if letters.is_empty() || digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let col = letters.bytes().try_fold(0usize, |acc, b| acc.checked_mul(26)?.checked_add((b.to_ascii_uppercase() - b'A') as usize + 1))? - 1;
    let row = digits.parse::<usize>().ok()?.checked_sub(1)?;
    (col < MAX_COLS && row < MAX_ROWS).then_some((row, col))
}

/// Where the formula's cell sits.
struct Here<'a> {
    t: &'a Table,
    r: usize,
    c: usize,
    g: usize,
}

enum Val {
    Num(f64),
    List(Vec<f64>),
}

/// The number in the cell covering grid column `g` of row `r`.
fn number_at(t: &Table, r: usize, g: usize) -> Option<f64> {
    let ci = t.cell_at_grid(r, g)?;
    t.rows.get(r)?.cells.get(ci).and_then(cell_number)
}

fn eval(e: &Expr, here: &Here) -> Result<Val, CmdError> {
    Ok(match e {
        Expr::Num(x) => Val::Num(*x),
        Expr::Cell(row, col, name) => {
            let x = number_at(here.t, *row, *col).ok_or_else(|| CmdError::Failed(format!("{name} doesn't hold a number")))?;
            Val::Num(x)
        }
        Expr::Range(r0, g0, r1, g1) => {
            let mut v = Vec::new();
            for row in *r0..=*r1 {
                for g in *g0..=*g1 {
                    v.extend(number_at(here.t, row, g));
                }
            }
            Val::List(v)
        }
        Expr::Side(side) => Val::List(side_numbers(here, *side)),
        Expr::Neg(inner) => Val::Num(-scalar(eval(inner, here)?)?),
        Expr::Bin(op, a, b) => {
            let (x, y) = (scalar(eval(a, here)?)?, scalar(eval(b, here)?)?);
            Val::Num(match op {
                '+' => x + y,
                '-' => x - y,
                '*' => x * y,
                '/' => {
                    if y == 0.0 {
                        return Err(CmdError::Failed("division by zero".into()));
                    }
                    x / y
                }
                _ => {
                    let p = x.powf(y);
                    finite(p)?
                }
            })
        }
        Expr::Call(name, args) => Val::Num(call(name, args, here)?),
    })
}

/// A value that must be one number (arithmetic and single-argument functions).
fn scalar(v: Val) -> Result<f64, CmdError> {
    match v {
        Val::Num(x) => finite(x),
        Val::List(_) => Err(syntax("a range can only be used inside a function".into())),
    }
}

/// The first run of numbers next to the formula's cell on one side (see the module note): the
/// cells are skipped while they hold no number, then summed while they do.
fn side_numbers(here: &Here, side: Side) -> Vec<f64> {
    let mut out = Vec::new();
    let mut started = false;
    let mut take = |value: Option<f64>| -> bool {
        match value {
            Some(x) => {
                out.push(x);
                started = true;
                true
            }
            None => !started,
        }
    };
    match side {
        Side::Above => {
            for row in (0..here.r).rev() {
                if !take(number_at(here.t, row, here.g)) {
                    break;
                }
            }
        }
        Side::Below => {
            for row in (here.r + 1)..here.t.rows.len() {
                if !take(number_at(here.t, row, here.g)) {
                    break;
                }
            }
        }
        Side::Left | Side::Right => {
            let Some(row) = here.t.rows.get(here.r) else { return out };
            let cells: Vec<Option<f64>> = row.cells.iter().map(cell_number).collect();
            let order: Vec<usize> = if side == Side::Left { (0..here.c).rev().collect() } else { ((here.c + 1)..cells.len()).collect() };
            for ci in order {
                if !take(cells.get(ci).copied().flatten()) {
                    break;
                }
            }
        }
    }
    out
}

/// Gather a function's arguments: numbers and lists (ranges and keywords) become one list.
fn gather(args: &[Expr], here: &Here) -> Result<Vec<f64>, CmdError> {
    let mut all = Vec::new();
    for a in args {
        match eval(a, here)? {
            Val::Num(x) => all.push(finite(x)?),
            Val::List(v) => all.extend(v),
        }
    }
    Ok(all)
}

/// The one number a single-argument function takes.
fn one(name: &str, args: &[Expr], here: &Here) -> Result<f64, CmdError> {
    match args {
        [a] => scalar(eval(a, here)?),
        _ => Err(CmdError::Failed(format!("{name} takes one argument"))),
    }
}

fn call(name: &str, args: &[Expr], here: &Here) -> Result<f64, CmdError> {
    if args.is_empty() && matches!(name, "SUM" | "COUNT" | "AVERAGE" | "MAX" | "MIN" | "PRODUCT" | "ABS" | "INT" | "SIGN" | "ROUND" | "MOD") {
        return Err(syntax(format!("{name}() needs an argument")));
    }
    match name {
        "SUM" => Ok(gather(args, here)?.iter().sum()),
        "COUNT" => Ok(gather(args, here)?.len() as f64),
        "AVERAGE" => {
            let v = gather(args, here)?;
            Ok(if v.is_empty() { 0.0 } else { v.iter().sum::<f64>() / v.len() as f64 })
        }
        "MAX" => {
            let v = gather(args, here)?;
            Ok(if v.is_empty() { 0.0 } else { v.iter().copied().fold(f64::NEG_INFINITY, f64::max) })
        }
        "MIN" => {
            let v = gather(args, here)?;
            Ok(if v.is_empty() { 0.0 } else { v.iter().copied().fold(f64::INFINITY, f64::min) })
        }
        "PRODUCT" => {
            let v = gather(args, here)?;
            Ok(if v.is_empty() { 1.0 } else { v.iter().product() })
        }
        "ABS" => Ok(one(name, args, here)?.abs()),
        "INT" => Ok(one(name, args, here)?.floor()),
        "SIGN" => Ok(one(name, args, here)?.signum()),
        "ROUND" => match args {
            [a, b] => {
                let x = scalar(eval(a, here)?)?;
                let digits = scalar(eval(b, here)?)?.clamp(-15.0, 15.0).trunc();
                let scale = 10f64.powi(digits as i32);
                finite((x * scale).round() / scale)
            }
            _ => Err(CmdError::Failed("ROUND takes two arguments".into())),
        },
        "MOD" => match args {
            [a, b] => {
                let (x, y) = (scalar(eval(a, here)?)?, scalar(eval(b, here)?)?);
                if y == 0.0 {
                    return Err(CmdError::Failed("division by zero".into()));
                }
                Ok(x - y * (x / y).floor())
            }
            _ => Err(CmdError::Failed("MOD takes two arguments".into())),
        },
        _ => Err(CmdError::Failed(format!("unknown formula function `{name}`"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: &[&[&str]]) -> Table {
        let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
        let mut t = Table::new(rows.len(), cols, 468.0);
        for (r, row) in rows.iter().enumerate() {
            for (c, text) in row.iter().enumerate() {
                if let Some(cl) = t.rows.get_mut(r).and_then(|x| x.cells.get_mut(c)) {
                    cl.blocks = vec![wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(text, Default::default()))];
                }
            }
        }
        t
    }

    fn ev(t: &Table, r: usize, c: usize, f: &str) -> Result<f64, CmdError> {
        evaluate(t, r, c, f)
    }

    #[test]
    fn cell_references_count_from_one_and_from_a() {
        assert_eq!(cell_ref("A1"), Some((0, 0)));
        assert_eq!(cell_ref("B4"), Some((3, 1)));
        assert_eq!(cell_ref("AA2"), Some((1, 26)));
        assert_eq!(cell_ref("A0"), None);
        assert_eq!(cell_ref("1A"), None);
    }

    #[test]
    fn a_run_of_numbers_above_is_summed_after_skipping_cells_without_numbers() {
        // Observed in Word: the cells above are skipped while empty or text, then the numbers are
        // summed up to the next cell that holds none.
        let t = table(&[&["5"], &["x"], &["2"], &[""]]);
        assert_eq!(ev(&t, 3, 0, "=SUM(ABOVE)"), Ok(2.0), "text stops the run; the 5 above it is not counted");
        assert_eq!(ev(&t, 2, 0, "=SUM(ABOVE)"), Ok(5.0), "text directly above is skipped, so the run is the 5 above it");
        let t = table(&[&["5"], &[""], &[""], &[""]]);
        assert_eq!(ev(&t, 3, 0, "=SUM(ABOVE)"), Ok(5.0), "empty cells are skipped up to the 5");
        let t = table(&[&["5"], &["3"], &[""], &["9"], &[""]]);
        assert_eq!(ev(&t, 4, 0, "=SUM(ABOVE)"), Ok(9.0), "the run is the 9 alone: the blank cuts it off");
        let t = table(&[&["x"], &["5"], &[""]]);
        assert_eq!(ev(&t, 2, 0, "=SUM(ABOVE)"), Ok(5.0));
    }

    #[test]
    fn a_function_needs_its_arguments() {
        let t = table(&[&["1"], &[""]]);
        assert!(ev(&t, 1, 0, "=SUM()").is_err(), "empty parentheses are a syntax error, as in Word");
        assert!(ev(&t, 1, 0, "=SUM").is_err());
    }

    #[test]
    fn unknown_functions_are_errors_not_sums() {
        let t = table(&[&["1"], &["4"], &["7"], &[""]]);
        let err = ev(&t, 3, 0, "=MEDIAN(ABOVE)").err().map(|e| e.to_string()).unwrap_or_default();
        assert!(err.contains("MEDIAN"), "{err}");
    }

    #[test]
    fn ranges_and_references_and_operators() {
        let t = table(&[&["1", "2"], &["3", "4"], &["", ""]]);
        assert_eq!(ev(&t, 2, 0, "=SUM(A1:B2)"), Ok(10.0));
        assert_eq!(ev(&t, 2, 1, "=A1*B2+1"), Ok(5.0));
        assert_eq!(ev(&t, 2, 0, "=(A1+A2)/2"), Ok(2.0));
        assert_eq!(ev(&t, 2, 0, "=2^3-(-1)"), Ok(9.0));
        assert_eq!(ev(&t, 2, 0, "=AVERAGE(A1:B2)"), Ok(2.5));
        assert_eq!(ev(&t, 2, 0, "=MAX(A1:B2)"), Ok(4.0));
        assert_eq!(ev(&t, 2, 0, "=ROUND(2.5, 0)"), Ok(3.0));
        assert_eq!(ev(&t, 2, 0, "=MOD(7, 3)"), Ok(1.0));
    }

    #[test]
    fn division_by_zero_and_bad_syntax_are_errors() {
        let t = table(&[&["1"], &["0"]]);
        assert!(ev(&t, 1, 0, "=A1/A2").is_err());
        assert!(ev(&t, 1, 0, "=SUM(A1").is_err());
        assert!(ev(&t, 1, 0, "=A1 A1").is_err());
        assert!(ev(&t, 1, 0, "=A1:").is_err());
        assert!(ev(&t, 1, 0, &format!("={}", "(".repeat(64))).is_err(), "deep nesting is refused");
    }

    #[test]
    fn numbers_read_currency_and_separators_but_not_letters() {
        assert_eq!(parse_number("$1,200"), Some(1200.0));
        assert_eq!(parse_number(" 12% "), Some(12.0));
        assert_eq!(parse_number("(5)"), Some(-5.0));
        assert_eq!(parse_number("x9"), None);
        assert_eq!(parse_number("1a"), None);
        assert_eq!(parse_number("inf"), None);
        assert_eq!(parse_number(""), None);
    }
}
