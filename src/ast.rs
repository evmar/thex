type Var = String;

fn var_from_iced(instr: &iced_x86::Instruction, op: u32) -> Var {
    use iced_x86::OpKind::*;
    match instr.op_kind(op) {
        Register => format!("{:?}", instr.op_register(op)).to_ascii_lowercase(),
        k => todo!("{k:?}"),
    }
}

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

impl From<String> for Expr {
    fn from(value: String) -> Self {
        Expr::Var(value)
    }
}

impl From<iced_x86::Register> for Expr {
    fn from(reg: iced_x86::Register) -> Self {
        format!("{reg:?}").to_ascii_lowercase().into()
    }
}

impl From<Call> for Expr {
    fn from(value: Call) -> Self {
        Expr::Call(Box::new(value))
    }
}

impl Expr {
    fn from_memory(instr: &iced_x86::Instruction) -> Self {
        let mut args = Vec::new();
        match instr.memory_segment() {
            iced_x86::Register::CS | iced_x86::Register::DS | iced_x86::Register::SS => {}
            r @ iced_x86::Register::FS => args.push(Expr::from(r)),
            iced_x86::Register::None => {}
            r => todo!("{r:?}"),
        }

        match instr.memory_base() {
            iced_x86::Register::None => {}
            r => args.push(Expr::from(r)),
        }

        if instr.memory_index() != iced_x86::Register::None {
            let mut expr = Expr::from(instr.memory_index());
            if instr.memory_index_scale() != 1 {
                expr = Expr::Call(Box::new(Call {
                    func: "*".into(),
                    args: vec![expr, Expr::Val(instr.memory_index_scale())],
                }));
            }
            args.push(expr);
        }

        let offset = instr.memory_displacement32();
        if offset != 0 {
            args.push(Expr::Val(offset));
        }

        Expr::Call(Box::new(Call {
            func: "mem".into(),
            args,
        }))
    }

    fn from_iced(instr: &iced_x86::Instruction, op: u32) -> Self {
        use iced_x86::OpKind::*;
        match instr.op_kind(op) {
            Immediate8 => Expr::Val(instr.immediate8() as u32),
            Immediate8to32 => Expr::Val(instr.immediate8to32() as u32),
            Immediate32 => Expr::Val(instr.immediate32()),
            NearBranch32 => Expr::Val(instr.near_branch32()),
            Register => Expr::Var(var_from_iced(instr, op)),
            Memory => Self::from_memory(instr),
            k => todo!("{k:?}"),
        }
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

impl From<&iced_x86::Instruction> for Stmt {
    fn from(instr: &iced_x86::Instruction) -> Self {
        Stmt {
            ip: instr.ip32(),
            kind: StmtKind::from(instr),
        }
    }
}

pub enum StmtKind {
    Set(Expr, Expr),
    Jmp(Box<Call>, Expr),
    Raw(String),
}

impl std::fmt::Display for StmtKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StmtKind::Set(var, expr) => write!(f, "(set {var} {expr})"),
            StmtKind::Jmp(cond, addr) => write!(f, "(jmp {cond} {addr})"),
            StmtKind::Raw(msg) => write!(f, "(todo {msg:?})"),
        }
    }
}

impl From<&iced_x86::Instruction> for StmtKind {
    fn from(instr: &iced_x86::Instruction) -> Self {
        use iced_x86::Mnemonic::*;
        let mnemonic = instr.mnemonic();
        match mnemonic {
            Mov => {
                let var = Expr::from_iced(instr, 0);
                let expr = Expr::from_iced(instr, 1);
                StmtKind::Set(var, expr)
            }
            Inc => {
                let expr = Expr::from_iced(instr, 0);
                let bin = super::Call {
                    func: "+".into(),
                    args: vec![expr.clone(), Expr::Val(1)],
                };
                StmtKind::Set(expr, Expr::from(bin))
            }
            Cmp | Test => {
                let left = Expr::from_iced(instr, 0);
                let right = Expr::from_iced(instr, 1);
                let func = match mnemonic {
                    Cmp => "cmp",
                    Test => "test",
                    _ => unreachable!(),
                }
                .into();
                let bin = super::Call {
                    func,
                    args: vec![left, right],
                };
                StmtKind::Set(Expr::from("_".to_owned()), Expr::from(bin))
            }
            Add | Shl | Sub | Xor => {
                let left = Expr::from_iced(instr, 0);
                let right = Expr::from_iced(instr, 1);
                let func = match mnemonic {
                    Add => "+",
                    Shl => "<<",
                    Sub => "-",
                    Xor => "^",
                    _ => unreachable!(),
                }
                .into();
                let bin = super::Call {
                    func,
                    args: vec![left.clone(), right],
                };
                StmtKind::Set(left, Expr::from(bin))
            }
            Jmp | Je | Jge | Jne => {
                let cond = Box::new(super::Call {
                    func: format!("{mnemonic:?}").to_ascii_lowercase(),
                    args: vec![],
                });
                let dst = Expr::from_iced(instr, 0);
                StmtKind::Jmp(cond, dst)
            }
            Ret => {
                let dst = Expr::Todo("stack ref".into());
                StmtKind::Jmp(
                    Box::new(super::Call {
                        func: "ret".into(),
                        args: vec![],
                    }),
                    dst,
                )
            }
            Push | Pop | Call | Imul => StmtKind::Raw(format!("{}", instr)),
            m => todo!("{m:?} in {instr}"),
        }
    }
}
