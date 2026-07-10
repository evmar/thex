pub type Var = smol_str::SmolStr;

#[derive(Clone, PartialEq, Eq)]
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

#[derive(Clone, PartialEq, Eq)]
pub enum Expr {
    Val(u32),
    Var(Var),
    Call(Box<Call>),
    Todo(String),
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Val(val) => write!(f, "{val:#x}"),
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
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}
impl From<String> for Expr {
    fn from(value: String) -> Self {
        Expr::Var(Var::new(value))
    }
}

impl From<Call> for Expr {
    fn from(value: Call) -> Self {
        Expr::Call(Box::new(value))
    }
}

pub struct Stmt {
    pub ip: u32,
    pub kind: StmtKind,
}

impl std::fmt::Display for Stmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08x} {}", self.ip, self.kind)
    }
}

pub enum StmtKind {
    Do(Expr),
    Set(Expr, Expr),
    Jmp(Box<Call>, Expr),
    Raw(String),
}

impl std::fmt::Display for StmtKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StmtKind::Do(expr) => write!(f, "{expr}"),
            StmtKind::Set(var, expr) => write!(f, "(set {var} {expr})"),
            StmtKind::Jmp(cond, addr) => write!(f, "(jmp {cond} {addr})"),
            StmtKind::Raw(msg) => write!(f, "(todo {msg:?})"),
        }
    }
}

pub fn visit_expr(expr: &mut Expr, visit: &mut impl FnMut(&mut Expr)) {
    visit(expr);
    if let Expr::Call(call) = expr {
        for arg in call.args.iter_mut() {
            visit_expr(arg, visit);
        }
    }
}

pub fn visit_stmt_expr(stmt: &mut Stmt, visit: &mut impl FnMut(&mut Expr)) {
    match &mut stmt.kind {
        StmtKind::Do(expr) => visit_expr(expr, visit),
        StmtKind::Set(dst, val) => {
            if let Expr::Var(_) = dst {
            } else {
                visit_expr(dst, visit);
            }
            visit_expr(val, visit);
        }
        StmtKind::Jmp(cond, dst) => {
            for arg in cond.args.iter_mut() {
                visit_expr(arg, visit);
            }
            visit_expr(dst, visit);
        }
        StmtKind::Raw(_) => {}
    }
}
