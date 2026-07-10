use std::collections::{HashMap, HashSet};

use crate::{
    ast::{Expr, StmtKind, Var, visit_stmt_expr},
    ssa::Block,
};

pub fn inline(blocks: &mut [Block]) {
    let mut var_defs = HashSet::<Var>::new();
    let mut var_uses = HashMap::<Var, usize>::new();
    for block in blocks.iter_mut() {
        for stmt in block.stmts.iter_mut() {
            if let StmtKind::Set(Expr::Var(var), _) = &stmt.kind {
                var_defs.insert(var.clone());
            }
            visit_stmt_expr(stmt, &mut |expr| {
                let Expr::Var(var) = expr else {
                    return;
                };
                *var_uses.entry(var.clone()).or_default() += 1;
            });
        }
    }

    let mut to_inline = vec![];
    for var in var_defs {
        let Some(&count) = var_uses.get(&var) else {
            eprintln!("BUG: {var} def but not use?");
            continue;
        };
        if count == 1 {
            to_inline.push(var);
        }
    }

    for inline_var in to_inline {
        for block in blocks.iter_mut() {
            let Some(i) = block.stmts.iter().position(|stmt| {
                let StmtKind::Set(Expr::Var(var), _) = &stmt.kind else {
                    return false;
                };
                *var == inline_var
            }) else {
                continue;
            };
            let StmtKind::Set(_, val) = block.stmts.remove(i).kind else {
                unreachable!();
            };
            for stmt in block.stmts[i..].iter_mut() {
                visit_stmt_expr(stmt, &mut |expr| {
                    let Expr::Var(var) = expr else {
                        return;
                    };
                    if *var == inline_var {
                        *expr = val.clone();
                    }
                });
            }
            break;
        }
    }
}
