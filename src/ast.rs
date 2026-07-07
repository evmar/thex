type Var = String;

fn var_from_iced(instr: &iced_x86::Instruction, op: u32) -> Var {
    use iced_x86::OpKind::*;
    match instr.op_kind(op) {
        Register => format!("{:?}", instr.op_register(op)).to_ascii_lowercase(),
        k => todo!("{k:?}"),
    }
}

#[derive(Clone)]
pub struct BinOp {
    op: char,
    left: Box<Expr>,
    right: Box<Expr>,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let BinOp { op, left, right } = self;
        write!(f, "({op} {left} {right})",)
    }
}

#[derive(Clone)]
pub enum Expr {
    Val(u32),
    Var(Var),
    BinOp(BinOp),
    Todo(String),
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Val(val) => write!(f, "{val:#x}"),
            Expr::Var(var) => write!(f, "{var}"),
            Expr::BinOp(op) => write!(f, "{op}"),
            Expr::Todo(msg) => write!(f, "(todo {msg:?})"),
        }
    }
}

impl From<u32> for Expr {
    fn from(value: u32) -> Self {
        Expr::Val(value)
    }
}

impl From<String> for Expr {
    fn from(value: String) -> Self {
        Expr::Var(value)
    }
}

impl Expr {
    fn from_iced(instr: &iced_x86::Instruction, op: u32) -> Self {
        use iced_x86::OpKind::*;
        match instr.op_kind(op) {
            Immediate8 => Expr::Val(instr.immediate8() as u32),
            NearBranch32 => Expr::Val(instr.immediate8() as u32),
            Register => Expr::Var(var_from_iced(instr, op)),
            Memory => Expr::Todo(format!("mem {instr}")),
            k => todo!("{k:?}"),
        }
    }
}

pub enum Stmt {
    Set(Expr, Expr),
    Jmp(String, Expr),
}

impl std::fmt::Display for Stmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stmt::Set(var, expr) => write!(f, "(set {var} {expr})"),
            Stmt::Jmp(cond, addr) => write!(f, "(jmp {cond} {addr})"),
        }
    }
}

impl Stmt {
    pub fn from_iced(instr: &iced_x86::Instruction) -> Self {
        use iced_x86::Mnemonic::*;
        let mnemonic = instr.mnemonic();
        match mnemonic {
            Mov => {
                let var = Expr::from_iced(instr, 0);
                let expr = Expr::from_iced(instr, 1);
                Stmt::Set(var, expr)
            }
            Inc => {
                let expr = Expr::from_iced(instr, 0);
                let bin = BinOp {
                    op: '+',
                    left: Box::new(expr.clone()),
                    right: Box::new(Expr::Val(1)),
                };
                Stmt::Set(expr, Expr::BinOp(bin))
            }
            Cmp | Test => {
                let left = Expr::from_iced(instr, 0);
                let right = Expr::from_iced(instr, 1);
                let op = match mnemonic {
                    Cmp => '-',
                    Test => '&',
                    _ => unreachable!(),
                };
                let bin = BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                };
                Stmt::Set(Expr::Var("_".into()), Expr::BinOp(bin))
            }
            Xor => {
                let left = Expr::from_iced(instr, 0);
                let right = Expr::from_iced(instr, 1);
                let op = match mnemonic {
                    Xor => '^',
                    _ => unreachable!(),
                };
                let bin = BinOp {
                    op,
                    left: Box::new(left.clone()),
                    right: Box::new(right),
                };
                Stmt::Set(left, Expr::BinOp(bin))
            }
            Je | Jne => {
                let cond = format!("{mnemonic:?}").to_ascii_lowercase();
                let dst = Expr::from_iced(instr, 0);
                Stmt::Jmp(cond, dst)
            }
            Ret => {
                let dst = Expr::Todo("stack ref".into());
                Stmt::Jmp("".into(), dst)
            }
            m => todo!("{m:?} in {instr}"),
        }
    }
}
