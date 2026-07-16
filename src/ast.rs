pub type Name = smol_str::SmolStr;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Var(pub u16);

impl std::fmt::Display for Var {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{}", self.0)
    }
}

impl Var {
    pub fn next(self) -> Var {
        Var(self.0 + 1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub func: String,
    pub args: Vec<Expr>,
}

impl std::fmt::Display for Call {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Call { func, args } = self;
        write!(f, "({func}")?;
        for arg in args.iter() {
            write!(f, " {arg}")?;
        }
        write!(f, ")")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Val(u32),
    Name(Name),
    Var(Var),
    Call(Box<Call>),
    Todo(String),
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Val(val) => {
                if *val < 10 {
                    write!(f, "{val}")
                } else {
                    write!(f, "{val:#x}")
                }
            }
            Expr::Name(name) => write!(f, "{name}"),
            Expr::Var(var) => write!(f, "{var}"),
            Expr::Call(op) => write!(f, "{op}"),
            Expr::Todo(msg) => write!(f, "(todo {msg:?})"),
        }
    }
}

impl From<u32> for Expr {
    fn from(value: u32) -> Self {
        Expr::Val(value)
    }
}

impl From<&str> for Expr {
    fn from(s: &str) -> Self {
        Expr::Name(Name::new(s))
    }
}
impl From<String> for Expr {
    fn from(s: String) -> Self {
        Expr::Name(Name::new(s))
    }
}
impl From<Name> for Expr {
    fn from(name: Name) -> Self {
        Expr::Name(name)
    }
}

impl From<Var> for Expr {
    fn from(var: Var) -> Self {
        Expr::Var(var)
    }
}

impl From<Call> for Expr {
    fn from(value: Call) -> Self {
        Expr::Call(Box::new(value))
    }
}

impl Expr {
    pub fn call(func: impl Into<String>, args: Vec<Expr>) -> Self {
        Expr::Call(
            Call {
                func: func.into(),
                args,
            }
            .into(),
        )
    }
}

#[derive(Clone, Debug)]
pub struct Stmt {
    /// Instruction addresses represented by this statement, in source order.
    pub ip: Vec<u32>,
    pub kind: StmtKind,
}

impl std::fmt::Display for Stmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            for (i, ip) in self.ip.iter().enumerate() {
                if i > 0 {
                    write!(f, ",")?;
                }
                write!(f, "{ip:08x}")?;
            }
            if self.ip.len() > 0 {
                write!(f, " ")?;
            }
        }
        write!(f, "{}", self.kind)
    }
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    Do(Expr),
    Set(Expr, Expr),
    Let(Var, Expr),
    Jmp(Box<Call>, Expr),
    Raw(String),
}

impl std::fmt::Display for StmtKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StmtKind::Do(expr) => write!(f, "{expr}"),
            StmtKind::Set(dst, expr) => write!(f, "(set {dst} {expr})"),
            StmtKind::Let(var, expr) => write!(f, "(let {var} {expr})"),
            StmtKind::Jmp(cond, addr) => write!(f, "(jmp {cond} {addr})"),
            StmtKind::Raw(msg) => write!(f, "(todo {msg:?})"),
        }
    }
}

pub fn visit_expr<'a>(expr: &'a Expr, visit: &mut impl FnMut(&'a Expr)) {
    visit(expr);
    if let Expr::Call(call) = expr {
        for arg in call.args.iter() {
            visit_expr(arg, visit);
        }
    }
}

pub fn visit_stmt_expr<'a>(stmt: &'a Stmt, visit: &mut impl FnMut(&'a Expr)) {
    match &stmt.kind {
        StmtKind::Do(expr) => visit_expr(expr, visit),
        StmtKind::Let(_, val) => {
            visit_expr(val, visit);
        }
        StmtKind::Set(dst, val) => {
            visit_expr(dst, visit);

            visit_expr(val, visit);
        }
        StmtKind::Jmp(cond, dst) => {
            for arg in cond.args.iter() {
                visit_expr(arg, visit);
            }
            visit_expr(dst, visit);
        }
        StmtKind::Raw(_) => {}
    }
}

pub fn visit_expr_mut(expr: &mut Expr, visit: &mut impl FnMut(&mut Expr)) {
    visit(expr);
    if let Expr::Call(call) = expr {
        for arg in call.args.iter_mut() {
            visit_expr_mut(arg, visit);
        }
    }
}

pub fn visit_stmt_expr_mut(stmt: &mut Stmt, visit: &mut impl FnMut(&mut Expr)) {
    match &mut stmt.kind {
        StmtKind::Do(expr) => visit_expr_mut(expr, visit),
        StmtKind::Let(_, val) => {
            visit_expr_mut(val, visit);
        }
        StmtKind::Set(dst, val) => {
            visit_expr_mut(dst, visit);
            visit_expr_mut(val, visit);
        }
        StmtKind::Jmp(cond, dst) => {
            for arg in cond.args.iter_mut() {
                visit_expr_mut(arg, visit);
            }
            visit_expr_mut(dst, visit);
        }
        StmtKind::Raw(_) => {}
    }
}
