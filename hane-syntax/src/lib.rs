pub mod eval;
pub mod lower;
pub mod parser;
pub mod print;

use std::fmt::{self, Display, Write};

#[derive(Clone, Copy)]
pub struct Location {
    pub pos: usize,
    pub line: usize,
    pub col: usize,
}

impl Location {
    fn from_pest(pos: pest::Position) -> Self {
        let (line, col) = pos.line_col();
        Location {
            pos: pos.pos(),
            line,
            col,
        }
    }
}

#[derive(Clone)]
pub struct Span {
    start: Location,
    end: Location,
}

impl Span {
    fn from_pest(span: pest::Span) -> Self {
        Span {
            start: Location::from_pest(span.start_pos()),
            end: Location::from_pest(span.end_pos()),
        }
    }
}

#[derive(Clone)]
pub struct Ident {
    pub span: Span,
    pub name: String,
}

impl Eq for Ident {}
impl PartialEq for Ident {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

pub struct Command {
    pub span: Span,
    pub variant: CommandVariant,
}

pub enum CommandVariant {
    Definition(Ident, Vec<Binder>, Expr, Expr),
    Axiom(Ident, Expr),
    Inductive(Vec<IndBody>),
    Print(Ident),
    Check(Expr),
    Compute(Expr),
}

/// A single type in a mutually defined inductive type set
pub struct IndBody {
    pub name: Ident,
    pub params: Vec<Binder>,
    pub ttype: Expr,
    pub constructors: Vec<IndConstructor>,
}

/// A single constructor of an inductive type
pub struct IndConstructor {
    pub name: Ident,
    pub ttype: Expr,
}

#[derive(PartialEq, Eq)]
pub enum Sort {
    Prop,
    Set,
    Type(usize),
}

pub struct Expr {
    pub span: Span,
    pub variant: Box<ExprVariant>,
}

impl Eq for Expr {}
impl PartialEq for Expr {
    fn eq(&self, other: &Self) -> bool {
        self.variant == other.variant
    }
}

#[derive(PartialEq, Eq)]
pub struct Binder {
    pub ident: Ident,
    pub ttype: Expr,
}

pub struct Pattern {
    constructor: Ident,
    params: Vec<Ident>,
}

impl Eq for Pattern {}
impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.constructor == other.constructor && self.params == other.params
    }
}

#[derive(PartialEq, Eq)]
pub enum ExprVariant {
    Sort(Sort),
    Var(String),
    App(Expr, Expr),
    Product(Vec<Binder>, Expr),
    Abstract(Vec<Binder>, Expr),
    Bind(Ident, Expr, Expr, Expr),
    Match(Expr, Ident, Pattern, Expr, Vec<(Pattern, Expr)>),
}

pub struct SpanError<E> {
    pub span: Span,
    pub err: E,
}

impl Span {
    pub fn write(&self, path: Option<&str>, input: &str, f: &mut impl Write) -> fmt::Result {
        let len = format!("{}", self.end.line).len();
        if let Some(path) = path {
            writeln!(
                f,
                "{0: >len$}--> {1}:{2}:{3}",
                "", path, self.start.line, self.start.col
            )?;
        } else {
            writeln!(
                f,
                "{0: >len$}--> {1}:{2}",
                "", self.start.line, self.start.col
            )?;
        }
        if self.start.line == self.end.line {
            let line = input.lines().nth(self.start.line - 1).unwrap();
            writeln!(f, "{0: >len$} |", "")?;
            writeln!(f, "{0} | {1}", self.start.line, line)?;
            writeln!(
                f,
                "{0: >len$} | {0: >col$}{0:^>span$}",
                "",
                col = self.start.col - 1,
                span = 1.max(self.end.col - self.start.col)
            )?;
            writeln!(f, "{0: >len$} |", "")?;
            write!(f, "{0: >len$} = ", "")?;
        } else {
            writeln!(f, "{0: >len$} |", "")?;
            let mut sep = "/";
            for (i, line) in input
                .lines()
                .enumerate()
                .take(self.end.line)
                .skip(self.start.line - 1)
            {
                writeln!(f, "{0} | {sep} {line}", i + 1)?;
                sep = "|";
            }
            writeln!(f, "{0: >len$} | |_{0:_>1$}^", "", self.end.col)?;
            writeln!(f, "{0: >len$} |", "")?;
            write!(f, "{0: >len$} = ", "")?;
        }
        Ok(())
    }
}

impl<E: Display> SpanError<E> {
    pub fn write(&self, path: Option<&str>, input: &str, f: &mut impl Write) -> fmt::Result {
        self.span.write(path, input, f)?;
        write!(f, "{}", self.err)
    }

    pub fn print(&self, path: Option<&str>, input: &str) -> String {
        let mut buf = String::new();
        self.write(path, input, &mut buf).unwrap();
        buf
    }
}
