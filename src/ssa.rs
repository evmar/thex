use std::collections::HashMap;

use crate::ast::{Expr, Stmt, StmtKind};

pub struct Block {
    pub ip: u32,
    pub stmts: Vec<Stmt>,
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

fn rename_expr(expr: &mut Expr, from: &str, to: &str) {
    match expr {
        Expr::Val(_) => {}
        Expr::Var(name) => {
            if name == from {
                *name = to.to_owned();
            }
        }
        Expr::Call(call) => {
            for arg in call.args.iter_mut() {
                rename_expr(arg, from, to);
            }
        }
        Expr::Todo(_) => todo!(),
    }
}

fn rename_stmt(stmt: &mut Stmt, from: &str, to: &str) {
    match &mut stmt.kind {
        StmtKind::Set(_, val) => {
            rename_expr(val, from, to);
        }
        StmtKind::Jmp(cond, dst) => {
            rename_expr(cond, from, to);
            rename_expr(dst, from, to);
        }
        StmtKind::Raw(_) => todo!(),
    }
}

fn ren(block: &mut Block, syms: &mut Syms) {
    for i in 0..block.stmts.len() {
        let (stmt, rest) = block.stmts[i..].split_first_mut().unwrap();
        match &mut stmt.kind {
            StmtKind::Set(Expr::Var(var), _) => {
                let new_name = format!("{var}{}", syms.next(var));
                for stmt in rest {
                    rename_stmt(stmt, var, &new_name);
                }
                *var = new_name;
            }
            _ => {}
        };
    }
}

pub fn ssa(stmts: Vec<Stmt>) -> Vec<Block> {
    let mut blocks = blocks(stmts);

    let mut syms = Syms::default();
    for block in blocks.iter_mut() {
        ren(block, &mut syms);
    }

    blocks
}
