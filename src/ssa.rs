use std::collections::{HashMap, HashSet};

use crate::ast::{Expr, Stmt, StmtKind};

pub struct Block {
    pub ip: u32,
    pub stmts: Vec<Stmt>,
}

impl From<Vec<Stmt>> for Block {
    fn from(stmts: Vec<Stmt>) -> Self {
        Block {
            ip: stmts[0].ip,
            stmts,
        }
    }
}

fn blocks(stmts: Vec<Stmt>) -> Vec<Block> {
    let mut jmp_targets = vec![];
    for stmt in &stmts {
        if let StmtKind::Jmp(_, dst) = &stmt.kind {
            if let Expr::Val(dst) = dst {
                jmp_targets.push(*dst);
            }
        }
    }

    let it = std::iter::from_fn({
        let mut stmts = stmts.into_iter().peekable();
        move || {
            let mut block_stmts = vec![];
            while let Some(stmt) = stmts.next() {
                let stmt = block_stmts.push_mut(stmt);
                if let StmtKind::Jmp(_, _) = stmt.kind {
                    break;
                };
                if let Some(next) = stmts.peek() {
                    if jmp_targets.contains(&next.ip) {
                        break;
                    }
                };
            }
            if block_stmts.is_empty() {
                None
            } else {
                Some(block_stmts)
            }
        }
    });

    it.map(|stmts| Block {
        ip: stmts[0].ip,
        stmts,
    })
    .collect()
}

#[derive(Default)]
struct Syms(HashMap<String, u8>);
impl Syms {
    fn next(&mut self, var: &str) -> u8 {
        let next: u8 = self.0.get(var).copied().unwrap_or(0) + 1;
        self.0.insert(var.to_owned(), next);
        next
    }
}

fn visit_expr(expr: &mut Expr, visit: &mut impl FnMut(&mut Expr)) {
    visit(expr);
    if let Expr::Call(call) = expr {
        for arg in call.args.iter_mut() {
            visit_expr(arg, visit);
        }
    }
}

fn visit_stmt_expr(stmt: &mut Stmt, visit: &mut impl FnMut(&mut Expr)) {
    match &mut stmt.kind {
        StmtKind::Set(_, val) => {
            visit_expr(val, visit);
        }
        StmtKind::Jmp(cond, dst) => {
            visit_expr(cond, visit);
            visit_expr(dst, visit);
        }
        StmtKind::Raw(_) => {}
    }
}

fn ssa_names(block: &mut Block, syms: &mut Syms) -> (HashSet<String>, HashMap<String, String>) {
    // variables introduced in this block
    let mut locals: HashSet<String> = HashSet::new();
    // block outputs; variables written to map from e.g. "eax" => "eax3"
    let mut outs: HashMap<String, String> = HashMap::new();
    for i in (0..block.stmts.len()).rev() {
        let (stmt, rest) = block.stmts[i..].split_first_mut().unwrap();
        match &mut stmt.kind {
            StmtKind::Set(Expr::Var(var), _) => {
                let new_name = format!("{var}{}", syms.next(var));
                locals.insert(new_name.clone());
                if !outs.contains_key(var) {
                    outs.insert(var.clone(), new_name.clone());
                }
                // Update references var=>new_name in subsequent statements.
                for stmt in rest {
                    visit_stmt_expr(stmt, &mut |expr| {
                        if let Expr::Var(name) = expr
                            && name == var
                        {
                            *name = new_name.clone();
                        }
                    });
                }
                *var = new_name;
            }
            _ => {}
        };
    }

    // block inputs; variables read from outside
    let mut ins: HashSet<String> = HashSet::new();
    for stmt in block.stmts.iter_mut() {
        visit_stmt_expr(stmt, &mut |expr| {
            if let Expr::Var(name) = expr {
                if !locals.contains(name) {
                    ins.insert(name.clone());
                }
            }
        });
    }

    (ins, outs)
}

pub fn ssa(stmts: Vec<Stmt>) -> Vec<Block> {
    let mut blocks = blocks(stmts);

    let mut syms = Syms::default();
    let mut block_ins: Vec<HashSet<String>> = vec![];
    let mut block_outs: Vec<HashMap<String, String>> = vec![];
    for block in blocks.iter_mut() {
        let (ins, outs) = ssa_names(block, &mut syms);
        println!("{:x} ins {ins:?} outs {outs:?}", block.ip);
        block_ins.push(ins);
        block_outs.push(outs);
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ssa_names(instrs: &[iced_x86::Instruction]) -> String {
        let mut block: Block = instrs
            .into_iter()
            .map(Stmt::from)
            .collect::<Vec<_>>()
            .into();
        let mut syms = Syms::default();
        super::ssa_names(&mut block, &mut syms);
        block
            .stmts
            .into_iter()
            .map(|stmt| format!("{}", stmt.kind))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn names() -> anyhow::Result<()> {
        use iced_x86::code_asm::*;
        let mut a = CodeAssembler::new(32)?;
        a.mov(eax, ebx)?;
        a.mov(ebx, eax)?;
        a.mov(eax, edx)?;
        a.mov(ecx, eax)?;
        insta::assert_snapshot!(ssa_names(a.instructions()), @"
        (set eax2 ebx)
        (set ebx1 eax2)
        (set eax1 edx)
        (set ecx1 eax1)
        ");
        Ok(())
    }
}
