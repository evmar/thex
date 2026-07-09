use std::collections::{HashMap, HashSet};

use crate::{
    ast::{Expr, Stmt, StmtKind},
    union::Union,
};

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
    fn next(&mut self, var: &str) -> String {
        let next: u8 = self.0.get(var).copied().unwrap_or(0) + 1;
        self.0.insert(var.to_owned(), next);
        format!("{var}{next}")
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

fn ssa_names(
    block: &mut Block,
    syms: &mut Syms,
) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut ins: HashMap<String, String> = HashMap::new();
    let mut new_vars: HashMap<String, String> = HashMap::new();
    for stmt in block.stmts.iter_mut() {
        visit_stmt_expr(stmt, &mut |expr| {
            let Expr::Var(var) = expr else {
                return;
            };
            match new_vars.get(var) {
                Some(new_var) => *var = new_var.clone(),
                None => {
                    // read of new variable means it's a block input
                    let new_var = syms.next(var);
                    ins.insert(var.clone(), new_var.clone()); // never overwritten
                    new_vars.insert(var.clone(), new_var.clone()); // may be overwritten
                    *var = new_var;
                }
            }
        });
        if let StmtKind::Set(Expr::Var(var), _) = &mut stmt.kind {
            let new_var = syms.next(var);
            new_vars.insert(var.clone(), new_var.clone());
            *var = new_var;
        }
    }

    (ins, new_vars)
}

fn links(blocks: &[Block], block: usize) -> Vec<usize> {
    let last = blocks[block].stmts.last().unwrap();
    let mut nexts = vec![];
    if let StmtKind::Jmp(cond, dst) = &last.kind {
        if let Expr::Val(addr) = dst {
            let next = blocks.iter().position(|b| b.ip == *addr).unwrap();
            nexts.push(next);
        } else {
            // uhoh
        }
        if cond.func == "jmp" || cond.func == "ret" {
            return nexts;
        }
    }
    nexts.push(block + 1);
    nexts
}

pub fn ssa(stmts: Vec<Stmt>) -> Vec<Block> {
    let mut blocks = blocks(stmts);

    let mut syms = Syms::default();
    let mut block_ins: Vec<HashMap<String, (String, HashSet<String>)>> = vec![];
    let mut block_outs: Vec<HashMap<String, String>> = vec![];
    for block in blocks.iter_mut() {
        let (ins, outs) = ssa_names(block, &mut syms);
        block_ins.push(
            ins.into_iter()
                .map(|(var, new_var)| (var, (new_var, Default::default())))
                .collect(),
        );
        block_outs.push(outs);
    }

    loop {
        let mut changed = false;

        for src in 0..blocks.len() {
            for dst in links(&blocks, src) {
                let mut passthrough = HashSet::new();
                for (var, (_, vars)) in &mut block_ins[dst] {
                    if let Some(out_var) = block_outs[src].get(var) {
                        if vars.insert(out_var.clone()) {
                            changed = true;
                        }
                    } else {
                        passthrough.insert(var.clone());
                    }
                }

                for var in passthrough {
                    let new_var = syms.next(&var);
                    block_ins[src].insert(var.clone(), (new_var.clone(), HashSet::new()));
                    block_outs[src].insert(var, new_var);
                    changed = true;
                }
            }
        }

        if !changed {
            break;
        }
    }

    let mut u = Union::new();
    for ins in block_ins.iter() {
        for (_, (new_var, vars)) in ins.iter() {
            for var in vars {
                u.join(new_var, var);
            }
        }
    }
    println!("union {:#?}", u.sets());

    // for block in blocks.iter_mut() {
    //     for stmt in block.stmts.iter_mut() {
    //         if let StmtKind::Set(var, _) = &mut stmt.kind {
    //             if let Expr::Var(var) = var {
    //                 let new_var = u.find(var);
    //                 *var = new_var.clone();
    //             }
    //         }
    //         visit_stmt_expr(stmt, &mut |expr| {
    //             if let Expr::Var(var) = expr {
    //                 let new_var = u.find(var);
    //                 *var = new_var.clone();
    //             }
    //         });
    //     }
    // }

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
