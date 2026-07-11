use std::collections::{HashMap, HashSet};

use crate::{
    ast::{Expr, StmtKind, Var, visit_stmt_expr},
    simp,
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
            let stmt = block.stmts.remove(i);
            let ip = stmt.ip;
            let StmtKind::Set(_, val) = stmt.kind else {
                unreachable!();
            };
            for stmt in block.stmts[i..].iter_mut() {
                let mut inlined = false;
                visit_stmt_expr(stmt, &mut |expr| {
                    let Expr::Var(var) = expr else {
                        return;
                    };
                    if *var == inline_var {
                        *expr = val.clone();
                        inlined = true;
                    }
                });
                if inlined {
                    visit_stmt_expr(stmt, &mut |expr| {
                        if let Some(new) = simp::math_constants(expr) {
                            *expr = new;
                        }
                    });
                    stmt.ip.splice(0..0, ip.iter().copied());
                }
            }
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Stmt;

    #[test]
    fn combines_ips_when_inlining() {
        let mut blocks = vec![Block {
            ip: 0x10,
            stmts: vec![
                Stmt {
                    ip: vec![0x10],
                    kind: StmtKind::Set("x".into(), 1.into()),
                },
                Stmt {
                    ip: vec![0x12],
                    kind: StmtKind::Do("x".into()),
                },
            ],
        }];

        inline(&mut blocks);

        assert_eq!(blocks[0].stmts.len(), 1);
        assert_eq!(blocks[0].stmts[0].ip, vec![0x10, 0x12]);
        assert!(matches!(
            blocks[0].stmts[0].kind,
            StmtKind::Do(Expr::Val(1))
        ));
    }
}
