//! A MEL-style scripting language: variables, procs, loops, expressions and Maya command syntax
//! (`polyCube -w 2 -n "box";`), calling into the command registry in `commands.rs`.

use crate::commands::{self, Host};
use crate::scene::Scene;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    Int(i64),
    Float(f64),
    Str(String),
    Vec3([f64; 3]),
    Array(Vec<Val>),
    None,
}

impl Val {
    pub fn num(&self) -> f64 {
        match self {
            Val::Int(i) => *i as f64,
            Val::Float(f) => *f,
            Val::Str(s) => s.trim().parse().unwrap_or(0.0),
            Val::Vec3(v) => v[0],
            Val::Array(a) => a.first().map(|x| x.num()).unwrap_or(0.0),
            Val::None => 0.0,
        }
    }
    pub fn truthy(&self) -> bool {
        match self {
            Val::Str(s) => !s.is_empty(),
            Val::Array(a) => !a.is_empty(),
            v => v.num() != 0.0,
        }
    }
    pub fn str(&self) -> String {
        self.to_string()
    }
    pub fn list(&self) -> Vec<Val> {
        match self {
            Val::Array(a) => a.clone(),
            Val::None => vec![],
            v => vec![v.clone()],
        }
    }
    pub fn from_json(v: &serde_json::Value) -> Val {
        match v {
            serde_json::Value::Bool(b) => Val::Int(*b as i64),
            serde_json::Value::Number(n) => n.as_i64().map(Val::Int).unwrap_or(Val::Float(n.as_f64().unwrap_or(0.0))),
            serde_json::Value::String(s) => Val::Str(s.clone()),
            serde_json::Value::Array(a) => Val::Array(a.iter().map(Val::from_json).collect()),
            _ => Val::None,
        }
    }
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Val::Int(i) => serde_json::json!(i),
            Val::Float(f) => serde_json::json!(f),
            Val::Str(s) => serde_json::json!(s),
            Val::Vec3(v) => serde_json::json!(v),
            Val::Array(a) => serde_json::Value::Array(a.iter().map(|x| x.to_json()).collect()),
            Val::None => serde_json::Value::Null,
        }
    }
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Val::Int(i) => write!(f, "{i}"),
            Val::Float(x) => write!(f, "{}", crate::math::fmt_num_long(*x)),
            Val::Str(s) => write!(f, "{s}"),
            Val::Vec3(v) => write!(f, "{} {} {}", crate::math::fmt_num_long(v[0]), crate::math::fmt_num_long(v[1]), crate::math::fmt_num_long(v[2])),
            Val::Array(a) => write!(f, "{}", a.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")),
            Val::None => Ok(()),
        }
    }
}

// ------------------------------------------------------------------ lexer

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64, bool), // value, is_int
    Str(String),
    Tick(String),
    Var(String),
    Id(String),
    Op(String),
}

fn is_id(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':' | '|' | '[' | ']' | '*' | '#')
}

fn tokenize(src: &str) -> Result<Vec<(Tok, bool)>, String> {
    // (token, preceded_by_space)
    let c: Vec<char> = src.chars().collect();
    let mut t = vec![];
    let mut i = 0;
    let mut space = true;
    while i < c.len() {
        let ch = c[i];
        if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        if ch.is_whitespace() {
            i += 1;
            space = true;
            continue;
        }
        let sp = space;
        space = false;
        if ch == '"' {
            let mut s = String::new();
            i += 1;
            while i < c.len() && c[i] != '"' {
                if c[i] == '\\' && i + 1 < c.len() {
                    i += 1;
                    s.push(match c[i] {
                        'n' => '\n',
                        't' => '\t',
                        x => x,
                    });
                } else {
                    s.push(c[i]);
                }
                i += 1;
            }
            i += 1;
            t.push((Tok::Str(s), sp));
            continue;
        }
        if ch == '`' {
            let mut s = String::new();
            i += 1;
            while i < c.len() && c[i] != '`' {
                s.push(c[i]);
                i += 1;
            }
            i += 1;
            t.push((Tok::Tick(s), sp));
            continue;
        }
        if ch.is_ascii_digit() || (ch == '.' && c.get(i + 1).is_some_and(|x| x.is_ascii_digit())) {
            let mut s = String::new();
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.' || ((c[i] == 'e' || c[i] == 'E') && c.get(i + 1).is_some_and(|x| x.is_ascii_digit() || *x == '-' || *x == '+'))) {
                if (c[i] == 'e' || c[i] == 'E') && matches!(c.get(i + 1), Some('-') | Some('+')) {
                    s.push(c[i]);
                    i += 1;
                }
                s.push(c[i]);
                i += 1;
            }
            // identifiers like 1abc are not valid; numbers followed by letters become ids (e.g. 2D)
            let is_int = !s.contains(['.', 'e', 'E']);
            t.push((Tok::Num(s.parse().map_err(|_| format!("bad number {s}"))?, is_int), sp));
            continue;
        }
        if ch == '$' {
            let mut s = String::new();
            i += 1;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') {
                s.push(c[i]);
                i += 1;
            }
            t.push((Tok::Var(s), sp));
            continue;
        }
        let two: String = c[i..(i + 2).min(c.len())].iter().collect();
        if ["==", "!=", "<=", ">=", "&&", "||", "++", "--", "+=", "-=", "*=", "/=", "<<", ">>"].contains(&two.as_str()) {
            t.push((Tok::Op(two), sp));
            i += 2;
            continue;
        }
        if ch.is_ascii_alphabetic() || matches!(ch, '_' | '|') {
            let mut s = String::new();
            while i < c.len() && is_id(c[i]) {
                s.push(c[i]);
                i += 1;
            }
            t.push((Tok::Id(s), sp));
            continue;
        }
        t.push((Tok::Op(ch.to_string()), sp));
        i += 1;
    }
    Ok(t)
}

// ------------------------------------------------------------------ AST

#[derive(Clone, Debug)]
enum Expr {
    Lit(Val),
    Var(String, Option<Box<Expr>>),
    Unary(String, Box<Expr>),
    Bin(String, Box<Expr>, Box<Expr>),
    Tern(Box<Expr>, Box<Expr>, Box<Expr>),
    Assign(String, Option<Box<Expr>>, String, Box<Expr>),
    IncDec(String, String, bool), // var, op, prefix
    Call(String, Vec<Expr>),
    Cmd(String, Vec<Expr>),
    Array(Vec<Expr>),
    Vector(Vec<Expr>),
    Cast(String, Box<Expr>),
}

#[derive(Clone, Debug)]
enum Stmt {
    Expr(Expr),
    Decl(String, bool, Vec<(String, Option<Expr>)>), // type, is_array, vars
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    For(Option<Box<Stmt>>, Option<Expr>, Option<Expr>, Box<Stmt>),
    ForIn(String, Expr, Box<Stmt>),
    Block(Vec<Stmt>),
    Return(Option<Expr>),
    Break,
    Continue,
    Proc(String, Vec<String>, Vec<Stmt>),
    Switch(Expr, Vec<(Option<Expr>, Vec<Stmt>)>),
    Empty,
}

const TYPES: &[&str] = &["int", "float", "string", "vector", "matrix"];

struct Parser {
    t: Vec<(Tok, bool)>,
    p: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.p).map(|x| &x.0)
    }
    fn peek_at(&self, o: usize) -> Option<&Tok> {
        self.t.get(self.p + o).map(|x| &x.0)
    }
    fn space_before(&self, o: usize) -> bool {
        self.t.get(self.p + o).map(|x| x.1).unwrap_or(true)
    }
    fn is_op(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if o == s)
    }
    fn is_id(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Tok::Id(o)) if o == s)
    }
    fn eat_op(&mut self, s: &str) -> Result<(), String> {
        if self.is_op(s) {
            self.p += 1;
            Ok(())
        } else {
            Err(format!("Syntax error: expected \"{s}\" near \"{}\"", self.near()))
        }
    }
    fn near(&self) -> String {
        match self.peek() {
            Some(Tok::Op(o)) | Some(Tok::Id(o)) | Some(Tok::Str(o)) => o.clone(),
            Some(Tok::Var(v)) => format!("${v}"),
            Some(Tok::Num(n, _)) => n.to_string(),
            Some(Tok::Tick(s)) => format!("`{s}`"),
            None => "end of script".into(),
        }
    }

    fn program(&mut self) -> Result<Vec<Stmt>, String> {
        let mut out = vec![];
        while self.p < self.t.len() {
            out.push(self.stmt()?);
        }
        Ok(out)
    }

    fn block_or_stmt(&mut self) -> Result<Stmt, String> {
        self.stmt()
    }

    fn stmt(&mut self) -> Result<Stmt, String> {
        if self.is_op(";") {
            self.p += 1;
            return Ok(Stmt::Empty);
        }
        if self.is_op("{") {
            self.p += 1;
            let mut v = vec![];
            while !self.is_op("}") {
                if self.p >= self.t.len() {
                    return Err("Syntax error: missing \"}\"".into());
                }
                v.push(self.stmt()?);
            }
            self.p += 1;
            return Ok(Stmt::Block(v));
        }
        if let Some(Tok::Id(id)) = self.peek().cloned() {
            match id.as_str() {
                "global" if matches!(self.peek_at(1), Some(Tok::Id(x)) if x == "proc" || TYPES.contains(&x.as_str())) => {
                    self.p += 1;
                    return self.stmt();
                }
                "proc" => return self.proc_def(),
                "if" => {
                    self.p += 1;
                    self.eat_op("(")?;
                    let c = self.expr()?;
                    self.eat_op(")")?;
                    let a = self.block_or_stmt()?;
                    let b = if self.is_id("else") {
                        self.p += 1;
                        Some(Box::new(self.block_or_stmt()?))
                    } else {
                        None
                    };
                    return Ok(Stmt::If(c, Box::new(a), b));
                }
                "while" => {
                    self.p += 1;
                    self.eat_op("(")?;
                    let c = self.expr()?;
                    self.eat_op(")")?;
                    return Ok(Stmt::While(c, Box::new(self.block_or_stmt()?)));
                }
                "do" => {
                    self.p += 1;
                    let b = self.block_or_stmt()?;
                    if !self.is_id("while") {
                        return Err("Syntax error: expected while".into());
                    }
                    self.p += 1;
                    self.eat_op("(")?;
                    let c = self.expr()?;
                    self.eat_op(")")?;
                    if self.is_op(";") {
                        self.p += 1;
                    }
                    return Ok(Stmt::DoWhile(Box::new(b), c));
                }
                "for" => {
                    self.p += 1;
                    self.eat_op("(")?;
                    // for ($x in $list)
                    if let (Some(Tok::Var(v)), Some(Tok::Id(kw))) = (self.peek().cloned(), self.peek_at(1).cloned()) {
                        if kw == "in" {
                            self.p += 2;
                            let l = self.expr()?;
                            self.eat_op(")")?;
                            return Ok(Stmt::ForIn(v, l, Box::new(self.block_or_stmt()?)));
                        }
                    }
                    let init = if self.is_op(";") { None } else { Some(Box::new(self.simple_stmt()?)) };
                    self.eat_op(";")?;
                    let cond = if self.is_op(";") { None } else { Some(self.expr()?) };
                    self.eat_op(";")?;
                    let step = if self.is_op(")") { None } else { Some(self.expr()?) };
                    self.eat_op(")")?;
                    return Ok(Stmt::For(init, cond, step, Box::new(self.block_or_stmt()?)));
                }
                "switch" => {
                    self.p += 1;
                    self.eat_op("(")?;
                    let e = self.expr()?;
                    self.eat_op(")")?;
                    self.eat_op("{")?;
                    let mut cases = vec![];
                    while !self.is_op("}") {
                        let label = if self.is_id("case") {
                            self.p += 1;
                            let v = self.unary()?;
                            Some(v)
                        } else if self.is_id("default") {
                            self.p += 1;
                            None
                        } else {
                            return Err("Syntax error in switch".into());
                        };
                        self.eat_op(":")?;
                        let mut body = vec![];
                        while !self.is_id("case") && !self.is_id("default") && !self.is_op("}") {
                            body.push(self.stmt()?);
                        }
                        cases.push((label, body));
                    }
                    self.p += 1;
                    return Ok(Stmt::Switch(e, cases));
                }
                "return" => {
                    self.p += 1;
                    let e = if self.is_op(";") { None } else { Some(self.expr()?) };
                    self.end()?;
                    return Ok(Stmt::Return(e));
                }
                "break" => {
                    self.p += 1;
                    self.end()?;
                    return Ok(Stmt::Break);
                }
                "continue" => {
                    self.p += 1;
                    self.end()?;
                    return Ok(Stmt::Continue);
                }
                _ => {}
            }
        }
        let s = self.simple_stmt()?;
        self.end()?;
        Ok(s)
    }

    fn end(&mut self) -> Result<(), String> {
        if self.is_op(";") {
            self.p += 1;
            Ok(())
        } else if self.p >= self.t.len() || self.is_op("}") {
            Ok(())
        } else {
            Err(format!("Syntax error near \"{}\"", self.near()))
        }
    }

    fn proc_def(&mut self) -> Result<Stmt, String> {
        self.p += 1; // proc
        // optional return type
        if let (Some(Tok::Id(t)), Some(Tok::Id(_))) = (self.peek().cloned(), self.peek_at(1).cloned()) {
            if TYPES.contains(&t.as_str()) {
                self.p += 1;
                if self.is_op("[") {
                    self.p += 2;
                }
            }
        }
        if self.is_op("[") {
            self.p += 2;
        }
        let Some(Tok::Id(name)) = self.peek().cloned() else { return Err("Syntax error: proc name".into()) };
        self.p += 1;
        self.eat_op("(")?;
        let mut args = vec![];
        while !self.is_op(")") {
            if matches!(self.peek(), Some(Tok::Id(_))) {
                self.p += 1;
            }
            let Some(Tok::Var(v)) = self.peek().cloned() else { return Err("Syntax error: proc argument".into()) };
            self.p += 1;
            if self.is_op("[") {
                self.p += 1;
                self.eat_op("]")?;
            }
            args.push(v);
            if self.is_op(",") {
                self.p += 1;
            }
        }
        self.p += 1;
        let Stmt::Block(body) = self.stmt()? else { return Err("Syntax error: proc body".into()) };
        Ok(Stmt::Proc(name, args, body))
    }

    /// Declaration, command or expression (no trailing ';').
    fn simple_stmt(&mut self) -> Result<Stmt, String> {
        if let Some(Tok::Id(t)) = self.peek().cloned() {
            if TYPES.contains(&t.as_str()) && matches!(self.peek_at(1), Some(Tok::Var(_))) {
                self.p += 1;
                let mut vars = vec![];
                let mut is_array = false;
                loop {
                    let Some(Tok::Var(v)) = self.peek().cloned() else { return Err("Syntax error in declaration".into()) };
                    self.p += 1;
                    if self.is_op("[") {
                        self.p += 1;
                        while !self.is_op("]") {
                            self.p += 1;
                        }
                        self.p += 1;
                        is_array = true;
                    }
                    let init = if self.is_op("=") {
                        self.p += 1;
                        Some(self.expr()?)
                    } else {
                        None
                    };
                    vars.push((v, init));
                    if self.is_op(",") {
                        self.p += 1;
                        continue;
                    }
                    break;
                }
                return Ok(Stmt::Decl(t, is_array, vars));
            }
            // command syntax: identifier not followed by '(' or an operator
            let next = self.peek_at(1).cloned();
            let call_syntax = matches!(&next, Some(Tok::Op(o)) if o == "(") && !self.space_before(1);
            let is_cmd = !call_syntax && !matches!(&next, Some(Tok::Op(o)) if ["=", "+=", "-=", "*=", "/=", "++", "--", "==", ".", "?", "+", "*", "/", "<", ">", "&&", "||", "!=", "<=", ">="].contains(&o.as_str()));
            if is_cmd || matches!(&next, Some(Tok::Op(o)) if o == "(" ) && self.space_before(1) && !self.is_call_paren() {
                self.p += 1;
                let args = self.cmd_args()?;
                return Ok(Stmt::Expr(Expr::Cmd(t, args)));
            }
        }
        Ok(Stmt::Expr(self.expr()?))
    }

    /// `cmd (expr) ...` with a space before '(' is still command syntax unless the whole thing is a call.
    fn is_call_paren(&self) -> bool {
        false
    }

    fn cmd_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = vec![];
        loop {
            match self.peek().cloned() {
                None => break,
                Some(Tok::Op(o)) if o == ";" || o == "}" || o == ")" => break,
                Some(Tok::Op(o)) if o == "-" => {
                    // -flag or negative number
                    match self.peek_at(1).cloned() {
                        Some(Tok::Id(f)) if !self.space_before(1) => {
                            self.p += 2;
                            args.push(Expr::Lit(Val::Str(format!("-{f}"))));
                        }
                        Some(Tok::Num(n, i)) if !self.space_before(1) => {
                            self.p += 2;
                            args.push(Expr::Lit(if i { Val::Int(-(n as i64)) } else { Val::Float(-n) }));
                        }
                        _ => {
                            self.p += 1;
                            args.push(Expr::Lit(Val::Str("-".into())));
                        }
                    }
                }
                Some(Tok::Op(o)) if o == "(" => {
                    self.p += 1;
                    let e = self.expr()?;
                    self.eat_op(")")?;
                    args.push(e);
                }
                Some(Tok::Op(o)) if o == "{" => args.push(self.primary()?),
                Some(Tok::Op(o)) if o == "<<" => args.push(self.primary()?),
                Some(Tok::Num(n, i)) => {
                    self.p += 1;
                    args.push(Expr::Lit(if i { Val::Int(n as i64) } else { Val::Float(n) }));
                }
                Some(Tok::Str(s)) => {
                    self.p += 1;
                    args.push(Expr::Lit(Val::Str(s)));
                }
                Some(Tok::Var(_)) => args.push(self.postfix()?),
                Some(Tok::Tick(s)) => {
                    self.p += 1;
                    args.push(Expr::Call("`".into(), vec![Expr::Lit(Val::Str(s))]));
                }
                Some(Tok::Id(s)) => {
                    self.p += 1;
                    args.push(Expr::Lit(Val::Str(s)));
                }
                Some(Tok::Op(o)) => {
                    self.p += 1;
                    args.push(Expr::Lit(Val::Str(o)));
                }
            }
        }
        Ok(args)
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.assign()
    }

    fn assign(&mut self) -> Result<Expr, String> {
        if let Some(Tok::Var(v)) = self.peek().cloned() {
            let save = self.p;
            self.p += 1;
            let mut idx = None;
            if self.is_op("[") {
                self.p += 1;
                idx = Some(Box::new(self.expr()?));
                self.eat_op("]")?;
            }
            if let Some(Tok::Op(o)) = self.peek().cloned() {
                if ["=", "+=", "-=", "*=", "/="].contains(&o.as_str()) {
                    self.p += 1;
                    let rhs = self.assign()?;
                    return Ok(Expr::Assign(v, idx, o, Box::new(rhs)));
                }
            }
            self.p = save;
        }
        self.ternary()
    }

    fn ternary(&mut self) -> Result<Expr, String> {
        let c = self.binary(0)?;
        if self.is_op("?") {
            self.p += 1;
            let a = self.assign()?;
            self.eat_op(":")?;
            let b = self.assign()?;
            return Ok(Expr::Tern(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
    }

    fn binary(&mut self, min: u8) -> Result<Expr, String> {
        let prec = |o: &str| -> Option<u8> {
            Some(match o {
                "||" => 1,
                "&&" => 2,
                "==" | "!=" => 3,
                "<" | ">" | "<=" | ">=" => 4,
                "+" | "-" => 5,
                "*" | "/" | "%" => 6,
                _ => return None,
            })
        };
        let mut lhs = self.unary()?;
        loop {
            let Some(Tok::Op(o)) = self.peek().cloned() else { break };
            let Some(p) = prec(&o) else { break };
            if p < min {
                break;
            }
            self.p += 1;
            let rhs = self.binary(p + 1)?;
            lhs = Expr::Bin(o, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if let Some(Tok::Op(o)) = self.peek().cloned() {
            if o == "-" || o == "!" || o == "+" {
                self.p += 1;
                let e = self.unary()?;
                return Ok(Expr::Unary(o, Box::new(e)));
            }
            if o == "++" || o == "--" {
                self.p += 1;
                let Some(Tok::Var(v)) = self.peek().cloned() else { return Err("Syntax error: ++ needs a variable".into()) };
                self.p += 1;
                return Ok(Expr::IncDec(v, o, true));
            }
            if o == "(" {
                // cast: (int) $x
                if let (Some(Tok::Id(t)), Some(Tok::Op(c))) = (self.peek_at(1).cloned(), self.peek_at(2).cloned()) {
                    if TYPES.contains(&t.as_str()) && c == ")" {
                        self.p += 3;
                        let e = self.unary()?;
                        return Ok(Expr::Cast(t, Box::new(e)));
                    }
                }
            }
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, String> {
        let e = self.primary()?;
        if let Expr::Var(v, None) = &e {
            if let Some(Tok::Op(o)) = self.peek().cloned() {
                if o == "++" || o == "--" {
                    self.p += 1;
                    return Ok(Expr::IncDec(v.clone(), o, false));
                }
            }
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, String> {
        match self.peek().cloned() {
            Some(Tok::Num(n, i)) => {
                self.p += 1;
                Ok(Expr::Lit(if i { Val::Int(n as i64) } else { Val::Float(n) }))
            }
            Some(Tok::Str(s)) => {
                self.p += 1;
                Ok(Expr::Lit(Val::Str(s)))
            }
            Some(Tok::Tick(s)) => {
                self.p += 1;
                Ok(Expr::Call("`".into(), vec![Expr::Lit(Val::Str(s))]))
            }
            Some(Tok::Var(v)) => {
                self.p += 1;
                if self.is_op("[") && !self.space_before(0) {
                    self.p += 1;
                    let i = self.expr()?;
                    self.eat_op("]")?;
                    return Ok(Expr::Var(v, Some(Box::new(i))));
                }
                Ok(Expr::Var(v, None))
            }
            Some(Tok::Op(o)) if o == "(" => {
                self.p += 1;
                let e = self.expr()?;
                self.eat_op(")")?;
                Ok(e)
            }
            Some(Tok::Op(o)) if o == "{" => {
                self.p += 1;
                let mut v = vec![];
                while !self.is_op("}") {
                    v.push(self.expr()?);
                    if self.is_op(",") {
                        self.p += 1;
                    }
                }
                self.p += 1;
                Ok(Expr::Array(v))
            }
            Some(Tok::Op(o)) if o == "<<" => {
                self.p += 1;
                let mut v = vec![];
                while !self.is_op(">>") {
                    v.push(self.binary(5)?);
                    if self.is_op(",") {
                        self.p += 1;
                    }
                    if self.p >= self.t.len() {
                        return Err("Syntax error: missing >>".into());
                    }
                }
                self.p += 1;
                Ok(Expr::Vector(v))
            }
            Some(Tok::Id(name)) => {
                self.p += 1;
                if self.is_op("(") {
                    self.p += 1;
                    let mut args = vec![];
                    while !self.is_op(")") {
                        args.push(self.expr()?);
                        if self.is_op(",") {
                            self.p += 1;
                        } else if !self.is_op(")") {
                            return Err(format!("Syntax error near \"{}\"", self.near()));
                        }
                    }
                    self.p += 1;
                    return Ok(Expr::Call(name, args));
                }
                if name == "true" || name == "on" || name == "yes" {
                    return Ok(Expr::Lit(Val::Int(1)));
                }
                if name == "false" || name == "off" || name == "no" {
                    return Ok(Expr::Lit(Val::Int(0)));
                }
                Ok(Expr::Lit(Val::Str(name)))
            }
            _ => Err(format!("Syntax error near \"{}\"", self.near())),
        }
    }
}

// ------------------------------------------------------------------ interpreter

enum Flow {
    Normal(Val),
    Return(Val),
    Break,
    Continue,
}

#[derive(Default)]
pub struct Interp {
    globals: HashMap<String, Val>,
    frames: Vec<HashMap<String, Val>>,
    procs: HashMap<String, (Vec<String>, Vec<Stmt>)>,
    pub output: Vec<String>,
    depth: usize,
}

impl Interp {
    pub fn new() -> Self {
        Self::default()
    }

    /// Run a script; returns the value of the last statement.
    pub fn run(&mut self, s: &mut Scene, host: &mut dyn Host, src: &str) -> Result<Val, String> {
        let toks = tokenize(src)?;
        let prog = Parser { t: toks, p: 0 }.program()?;
        let mut last = Val::None;
        for st in &prog {
            match self.exec(s, host, st)? {
                Flow::Normal(v) => last = v,
                Flow::Return(v) => return Ok(v),
                _ => {}
            }
        }
        Ok(last)
    }

    fn get(&self, v: &str) -> Val {
        if let Some(f) = self.frames.last() {
            if let Some(x) = f.get(v) {
                return x.clone();
            }
        }
        self.globals.get(v).cloned().unwrap_or(Val::None)
    }
    fn set(&mut self, v: &str, val: Val) {
        if let Some(f) = self.frames.last_mut() {
            if f.contains_key(v) || !self.globals.contains_key(v) {
                f.insert(v.into(), val);
                return;
            }
        }
        self.globals.insert(v.into(), val);
    }
    fn declare(&mut self, v: &str, val: Val) {
        match self.frames.last_mut() {
            Some(f) => {
                f.insert(v.into(), val);
            }
            None => {
                self.globals.insert(v.into(), val);
            }
        }
    }

    fn exec(&mut self, s: &mut Scene, h: &mut dyn Host, st: &Stmt) -> Result<Flow, String> {
        Ok(match st {
            Stmt::Empty => Flow::Normal(Val::None),
            Stmt::Expr(e) => Flow::Normal(self.eval(s, h, e)?),
            Stmt::Decl(ty, arr, vars) => {
                for (v, init) in vars {
                    let val = match init {
                        Some(e) => cast(ty, *arr, self.eval(s, h, e)?),
                        None => {
                            if *arr {
                                Val::Array(vec![])
                            } else {
                                default_of(ty)
                            }
                        }
                    };
                    self.declare(v, val);
                }
                Flow::Normal(Val::None)
            }
            Stmt::If(c, a, b) => {
                if self.eval(s, h, c)?.truthy() {
                    self.exec(s, h, a)?
                } else if let Some(b) = b {
                    self.exec(s, h, b)?
                } else {
                    Flow::Normal(Val::None)
                }
            }
            Stmt::While(c, b) => {
                let mut n = 0;
                while self.eval(s, h, c)?.truthy() {
                    n += 1;
                    if n > 10_000_000 {
                        return Err("loop limit".into());
                    }
                    match self.exec(s, h, b)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Flow::Normal(Val::None)
            }
            Stmt::DoWhile(b, c) => {
                loop {
                    match self.exec(s, h, b)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                    if !self.eval(s, h, c)?.truthy() {
                        break;
                    }
                }
                Flow::Normal(Val::None)
            }
            Stmt::For(init, c, step, b) => {
                if let Some(i) = init {
                    self.exec(s, h, i)?;
                }
                loop {
                    if let Some(c) = c {
                        if !self.eval(s, h, c)?.truthy() {
                            break;
                        }
                    }
                    match self.exec(s, h, b)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                    if let Some(st) = step {
                        self.eval(s, h, st)?;
                    }
                }
                Flow::Normal(Val::None)
            }
            Stmt::ForIn(v, l, b) => {
                for item in self.eval(s, h, l)?.list() {
                    self.set(v, item);
                    match self.exec(s, h, b)? {
                        Flow::Break => break,
                        Flow::Return(v) => return Ok(Flow::Return(v)),
                        _ => {}
                    }
                }
                Flow::Normal(Val::None)
            }
            Stmt::Switch(e, cases) => {
                let v = self.eval(s, h, e)?;
                let mut on = false;
                for (label, body) in cases {
                    if !on {
                        on = match label {
                            None => true,
                            Some(l) => {
                                let lv = self.eval(s, h, l)?;
                                lv.str() == v.str()
                            }
                        };
                    }
                    if on {
                        for st in body {
                            match self.exec(s, h, st)? {
                                Flow::Break => return Ok(Flow::Normal(Val::None)),
                                Flow::Return(v) => return Ok(Flow::Return(v)),
                                _ => {}
                            }
                        }
                    }
                }
                Flow::Normal(Val::None)
            }
            Stmt::Block(v) => {
                let mut last = Val::None;
                for st in v {
                    match self.exec(s, h, st)? {
                        Flow::Normal(x) => last = x,
                        f => return Ok(f),
                    }
                }
                Flow::Normal(last)
            }
            Stmt::Return(e) => Flow::Return(match e {
                Some(e) => self.eval(s, h, e)?,
                None => Val::None,
            }),
            Stmt::Break => Flow::Break,
            Stmt::Continue => Flow::Continue,
            Stmt::Proc(name, args, body) => {
                self.procs.insert(name.clone(), (args.clone(), body.clone()));
                Flow::Normal(Val::None)
            }
        })
    }

    fn eval(&mut self, s: &mut Scene, h: &mut dyn Host, e: &Expr) -> Result<Val, String> {
        Ok(match e {
            Expr::Lit(v) => v.clone(),
            Expr::Var(v, None) => self.get(v),
            Expr::Var(v, Some(i)) => {
                let i = self.eval(s, h, i)?.num() as usize;
                match self.get(v) {
                    Val::Array(a) => a.get(i).cloned().unwrap_or(Val::None),
                    Val::Vec3(a) => Val::Float(a.get(i).copied().unwrap_or(0.0)),
                    _ => Val::None,
                }
            }
            Expr::Unary(o, x) => {
                let v = self.eval(s, h, x)?;
                match o.as_str() {
                    "!" => Val::Int(!v.truthy() as i64),
                    "-" => match v {
                        Val::Int(i) => Val::Int(-i),
                        Val::Vec3(a) => Val::Vec3([-a[0], -a[1], -a[2]]),
                        v => Val::Float(-v.num()),
                    },
                    _ => v,
                }
            }
            Expr::Bin(o, a, b) => {
                if o == "&&" {
                    let a = self.eval(s, h, a)?;
                    return Ok(Val::Int((a.truthy() && self.eval(s, h, b)?.truthy()) as i64));
                }
                if o == "||" {
                    let a = self.eval(s, h, a)?;
                    return Ok(Val::Int((a.truthy() || self.eval(s, h, b)?.truthy()) as i64));
                }
                let a = self.eval(s, h, a)?;
                let b = self.eval(s, h, b)?;
                binop(o, &a, &b)?
            }
            Expr::Tern(c, a, b) => {
                if self.eval(s, h, c)?.truthy() {
                    self.eval(s, h, a)?
                } else {
                    self.eval(s, h, b)?
                }
            }
            Expr::Assign(v, idx, op, rhs) => {
                let r = self.eval(s, h, rhs)?;
                let val = if op == "=" {
                    r
                } else {
                    let cur = match idx {
                        Some(i) => {
                            let i = self.eval(s, h, i)?.num() as usize;
                            self.get(v).list().get(i).cloned().unwrap_or(Val::Int(0))
                        }
                        None => self.get(v),
                    };
                    binop(&op[..1], &cur, &r)?
                };
                match idx {
                    Some(i) => {
                        let i = self.eval(s, h, i)?.num() as usize;
                        let mut a = self.get(v).list();
                        if a.len() <= i {
                            a.resize(i + 1, Val::None);
                        }
                        a[i] = val.clone();
                        self.set(v, Val::Array(a));
                    }
                    None => {
                        // keep int/float typing of the existing variable
                        let val = match (self.get(v), val) {
                            (Val::Int(_), Val::Float(f)) => Val::Int(f as i64),
                            (Val::Float(_), Val::Int(i)) => Val::Float(i as f64),
                            (_, x) => x,
                        };
                        self.set(v, val.clone());
                        return Ok(val);
                    }
                }
                val
            }
            Expr::IncDec(v, op, prefix) => {
                let cur = self.get(v);
                let nv = binop(&op[..1], &cur, &Val::Int(1))?;
                self.set(v, nv.clone());
                if *prefix {
                    nv
                } else {
                    cur
                }
            }
            Expr::Array(items) => Val::Array(items.iter().map(|x| self.eval(s, h, x)).collect::<Result<_, _>>()?),
            Expr::Vector(items) => {
                let v: Vec<f64> = items.iter().map(|x| self.eval(s, h, x).map(|v| v.num())).collect::<Result<_, _>>()?;
                Val::Vec3([v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)])
            }
            Expr::Cast(t, x) => cast(t, false, self.eval(s, h, x)?),
            Expr::Call(name, args) if name == "`" => {
                let Expr::Lit(Val::Str(src)) = &args[0] else { unreachable!() };
                self.run_nested(s, h, src)?
            }
            Expr::Call(name, args) | Expr::Cmd(name, args) => {
                let vals: Vec<Val> = args.iter().map(|x| self.eval(s, h, x)).collect::<Result<_, _>>()?;
                self.call(s, h, name, vals, matches!(e, Expr::Cmd(..)))?
            }
        })
    }

    fn run_nested(&mut self, s: &mut Scene, h: &mut dyn Host, src: &str) -> Result<Val, String> {
        let toks = tokenize(src)?;
        let prog = Parser { t: toks, p: 0 }.program()?;
        let mut last = Val::None;
        for st in &prog {
            if let Flow::Normal(v) | Flow::Return(v) = self.exec(s, h, st)? {
                last = v;
            }
        }
        Ok(last)
    }

    fn call(&mut self, s: &mut Scene, h: &mut dyn Host, name: &str, args: Vec<Val>, cmd_syntax: bool) -> Result<Val, String> {
        if let Some((params, body)) = self.procs.get(name).cloned() {
            if self.depth > 200 {
                return Err("recursion too deep".into());
            }
            let mut frame = HashMap::new();
            for (i, p) in params.iter().enumerate() {
                frame.insert(p.clone(), args.get(i).cloned().unwrap_or(Val::None));
            }
            self.frames.push(frame);
            self.depth += 1;
            let mut ret = Val::None;
            for st in &body {
                match self.exec(s, h, st) {
                    Ok(Flow::Return(v)) => {
                        ret = v;
                        break;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        self.frames.pop();
                        self.depth -= 1;
                        return Err(e);
                    }
                }
            }
            self.frames.pop();
            self.depth -= 1;
            return Ok(ret);
        }
        if let Some(v) = builtin(name, &args) {
            return v;
        }
        match name {
            "print" => {
                let text: String = args.iter().map(|a| a.str()).collect();
                self.output.push(text.clone());
                s.msg(crate::scene::MsgKind::Info, text);
                return Ok(Val::None);
            }
            "eval" => {
                let src: String = args.iter().map(|a| a.str()).collect::<Vec<_>>().join(" ");
                return self.run_nested(s, h, &src);
            }
            "exists" => {
                let n = args.first().map(|a| a.str()).unwrap_or_default();
                return Ok(Val::Int((self.procs.contains_key(&n) || commands::exists(&n)) as i64));
            }
            "source" => return Ok(Val::None),
            _ => {}
        }
        let _ = cmd_syntax;
        commands::run(s, h, name, &args)
    }
}

fn default_of(ty: &str) -> Val {
    match ty {
        "int" => Val::Int(0),
        "float" => Val::Float(0.0),
        "string" => Val::Str(String::new()),
        "vector" => Val::Vec3([0.0; 3]),
        _ => Val::None,
    }
}

fn cast(ty: &str, arr: bool, v: Val) -> Val {
    if arr {
        return Val::Array(v.list().into_iter().map(|x| cast(ty, false, x)).collect());
    }
    match ty {
        "int" => Val::Int(v.num() as i64),
        "float" => Val::Float(v.num()),
        "string" => Val::Str(v.str()),
        "vector" => match v {
            Val::Vec3(a) => Val::Vec3(a),
            Val::Array(a) => Val::Vec3([a.first().map(|x| x.num()).unwrap_or(0.0), a.get(1).map(|x| x.num()).unwrap_or(0.0), a.get(2).map(|x| x.num()).unwrap_or(0.0)]),
            v => Val::Vec3([v.num(); 3]),
        },
        _ => v,
    }
}

fn binop(o: &str, a: &Val, b: &Val) -> Result<Val, String> {
    use Val::*;
    if o == "+" && (matches!(a, Str(_)) || matches!(b, Str(_))) {
        return Ok(Str(format!("{a}{b}")));
    }
    if let (Vec3(x), Vec3(y)) = (a, b) {
        return Ok(match o {
            "+" => Vec3([x[0] + y[0], x[1] + y[1], x[2] + y[2]]),
            "-" => Vec3([x[0] - y[0], x[1] - y[1], x[2] - y[2]]),
            "*" => Float(x[0] * y[0] + x[1] * y[1] + x[2] * y[2]),
            "==" => Int((x == y) as i64),
            "!=" => Int((x != y) as i64),
            _ => return Err(format!("bad vector operator {o}")),
        });
    }
    if let (Vec3(x), s) | (s, Vec3(x)) = (a, b) {
        if matches!(s, Int(_) | Float(_)) {
            let k = s.num();
            return Ok(match o {
                "*" => Vec3([x[0] * k, x[1] * k, x[2] * k]),
                "/" => Vec3([x[0] / k, x[1] / k, x[2] / k]),
                _ => return Err(format!("bad vector operator {o}")),
            });
        }
    }
    if matches!(o, "==" | "!=") && (matches!(a, Str(_)) || matches!(b, Str(_))) {
        let eq = a.str() == b.str();
        return Ok(Int(if o == "==" { eq } else { !eq } as i64));
    }
    if matches!(o, "+") {
        if let (Array(x), Array(y)) = (a, b) {
            let mut v = x.clone();
            v.extend(y.iter().cloned());
            return Ok(Array(v));
        }
    }
    let ints = matches!(a, Int(_)) && matches!(b, Int(_));
    let (x, y) = (a.num(), b.num());
    Ok(match o {
        "+" => num(x + y, ints),
        "-" => num(x - y, ints),
        "*" => num(x * y, ints),
        "/" => {
            if ints {
                if y == 0.0 {
                    return Err("division by zero".into());
                }
                Int((x / y).trunc() as i64)
            } else {
                Float(x / y)
            }
        }
        "%" => num(x % y, ints),
        "<" => Int((x < y) as i64),
        ">" => Int((x > y) as i64),
        "<=" => Int((x <= y) as i64),
        ">=" => Int((x >= y) as i64),
        "==" => Int((x == y) as i64),
        "!=" => Int((x != y) as i64),
        _ => return Err(format!("unknown operator {o}")),
    })
}
fn num(x: f64, int: bool) -> Val {
    if int {
        Val::Int(x as i64)
    } else {
        Val::Float(x)
    }
}

/// Pure functions (no scene access).
fn builtin(name: &str, a: &[Val]) -> Option<Result<Val, String>> {
    let n = |i: usize| a.get(i).map(|v| v.num()).unwrap_or(0.0);
    let s = |i: usize| a.get(i).map(|v| v.str()).unwrap_or_default();
    let f = |x: f64| Some(Ok(Val::Float(x)));
    match name {
        "abs" => Some(Ok(match a.first() {
            Some(Val::Int(i)) => Val::Int(i.abs()),
            _ => Val::Float(n(0).abs()),
        })),
        "sin" => f(n(0).sin()),
        "cos" => f(n(0).cos()),
        "tan" => f(n(0).tan()),
        "asin" => f(n(0).asin()),
        "acos" => f(n(0).acos()),
        "atan" => f(n(0).atan()),
        "atan2" => f(n(0).atan2(n(1))),
        "sqrt" => f(n(0).sqrt()),
        "pow" => f(n(0).powf(n(1))),
        "exp" => f(n(0).exp()),
        "log" => f(n(0).ln()),
        "floor" => f(n(0).floor()),
        "ceil" => f(n(0).ceil()),
        "trunc" => f(n(0).trunc()),
        "sign" => f(n(0).signum()),
        "min" => f(n(0).min(n(1))),
        "max" => f(n(0).max(n(1))),
        "clamp" => f(n(2).clamp(n(0), n(1))),
        "deg_to_rad" => f(n(0).to_radians()),
        "rad_to_deg" => f(n(0).to_degrees()),
        "linstep" => f(((n(2) - n(0)) / (n(1) - n(0))).clamp(0.0, 1.0)),
        "smoothstep" => {
            let t = ((n(2) - n(0)) / (n(1) - n(0))).clamp(0.0, 1.0);
            f(t * t * (3.0 - 2.0 * t))
        }
        "rand" => {
            let r = pseudo_rand();
            if a.len() >= 2 {
                f(n(0) + (n(1) - n(0)) * r)
            } else {
                f(n(0) * r)
            }
        }
        "size" => Some(Ok(Val::Int(match a.first() {
            Some(Val::Array(v)) => v.len() as i64,
            Some(Val::Str(st)) => st.chars().count() as i64,
            Some(Val::None) | None => 0,
            _ => 1,
        }))),
        "strip" => Some(Ok(Val::Str(s(0).trim().to_string()))),
        "toupper" => Some(Ok(Val::Str(s(0).to_uppercase()))),
        "tolower" => Some(Ok(Val::Str(s(0).to_lowercase()))),
        "substring" => {
            let st: Vec<char> = s(0).chars().collect();
            let (b, e) = ((n(1) as usize).max(1) - 1, (n(2) as usize).min(st.len()));
            Some(Ok(Val::Str(st.get(b..e).map(|x| x.iter().collect()).unwrap_or_default())))
        }
        "substitute" => Some(Ok(Val::Str(s(1).replacen(&s(0), &s(2), 1)))),
        "substituteAllString" => Some(Ok(Val::Str(s(0).replace(&s(1), &s(2))))),
        "gmatch" => Some(Ok(Val::Int(glob_match(&s(1), &s(0)) as i64))),
        "startsWith" => Some(Ok(Val::Int(s(0).starts_with(&s(1)) as i64))),
        "endsWith" => Some(Ok(Val::Int(s(0).ends_with(&s(1)) as i64))),
        "tokenize" => {
            let sep = if a.len() > 1 && !matches!(a.get(1), Some(Val::Array(_)) | Some(Val::None)) { s(1) } else { " ".into() };
            let parts: Vec<Val> = s(0).split(|c| sep.contains(c)).filter(|x| !x.is_empty()).map(|x| Val::Str(x.into())).collect();
            Some(Ok(Val::Array(parts)))
        }
        "stringArrayToString" => Some(Ok(Val::Str(a.first().map(|v| v.list().iter().map(|x| x.str()).collect::<Vec<_>>().join(&s(1))).unwrap_or_default()))),
        "stringArrayCount" => Some(Ok(Val::Int(a.get(1).map(|v| v.list().iter().filter(|x| x.str() == s(0)).count()).unwrap_or(0) as i64))),
        "mag" => Some(Ok(Val::Float(match a.first() {
            Some(Val::Vec3(v)) => (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
            _ => n(0).abs(),
        }))),
        "unit" => Some(Ok(match a.first() {
            Some(Val::Vec3(v)) => {
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-12);
                Val::Vec3([v[0] / l, v[1] / l, v[2] / l])
            }
            _ => Val::Float(n(0).signum()),
        })),
        "cross" => Some(Ok(match (a.first(), a.get(1)) {
            (Some(Val::Vec3(x)), Some(Val::Vec3(y))) => Val::Vec3([x[1] * y[2] - x[2] * y[1], x[2] * y[0] - x[0] * y[2], x[0] * y[1] - x[1] * y[0]]),
            _ => Val::Vec3([0.0; 3]),
        })),
        "dot" => Some(Ok(match (a.first(), a.get(1)) {
            (Some(Val::Vec3(x)), Some(Val::Vec3(y))) => Val::Float(x[0] * y[0] + x[1] * y[1] + x[2] * y[2]),
            _ => Val::Float(n(0) * n(1)),
        })),
        _ => None,
    }
}

fn pseudo_rand() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(0x2545F4914F6CDD1D);
    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// Shell-style wildcard match (`*`, `?`).
pub fn glob_match(pat: &str, s: &str) -> bool {
    let p: Vec<char> = pat.chars().collect();
    let t: Vec<char> = s.chars().collect();
    fn m(p: &[char], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some('*') => (0..=t.len()).any(|i| m(&p[1..], &t[i..])),
            Some('?') => !t.is_empty() && m(&p[1..], &t[1..]),
            Some(c) => t.first() == Some(c) && m(&p[1..], &t[1..]),
        }
    }
    m(&p, &t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::NoHost;

    fn run(src: &str) -> (Scene, Interp, Result<Val, String>) {
        let mut s = Scene::new();
        let mut i = Interp::new();
        let r = i.run(&mut s, &mut NoHost, src);
        (s, i, r)
    }

    #[test]
    fn expressions_and_loops() {
        let (_, _, r) = run("int $t = 0; for ($i = 0; $i < 5; $i++) { $t += $i; } $t;");
        assert_eq!(r.unwrap(), Val::Int(10));
        let (_, _, r) = run("string $a[] = {\"x\", \"y\"}; size($a) * 2 + 1;");
        assert_eq!(r.unwrap(), Val::Int(5));
        let (_, _, r) = run("proc int sq(int $x) { return $x * $x; } sq(7);");
        assert_eq!(r.unwrap(), Val::Int(49));
        let (_, _, r) = run("vector $v = <<1, 2, 3>>; $v * 2;");
        assert_eq!(r.unwrap(), Val::Vec3([2.0, 4.0, 6.0]));
        let (_, i, _) = run("print (\"a\" + 1 + \"\\n\");");
        assert_eq!(i.output, vec!["a1\n"]);
    }

    #[test]
    fn commands_build_a_scene() {
        let (s, _, r) = run("polyCube -w 2 -h 3 -n box; move -r 0 5 0; string $sel[] = `ls -sl`; $sel[0];");
        assert_eq!(r.unwrap(), Val::Str("box".into()));
        let id = s.find("box").unwrap();
        assert_eq!(s.get_attr_f(&id, "ty"), 5.0);
        let b = s.world_bbox(&id);
        assert!((b.size().y - 3.0).abs() < 1e-9);
    }

    #[test]
    fn get_set_attr_and_history() {
        let (s, _, r) = run("polySphere; setAttr polySphere1.radius 2; setAttr pSphere1.translateX 4; getAttr pSphere1.tx;");
        assert_eq!(r.unwrap().num(), 4.0);
        let id = s.find("pSphere1").unwrap();
        assert!((s.world_bbox(&id).size().x - 4.0).abs() < 1e-6);
    }
}
