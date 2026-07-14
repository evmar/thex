use std::collections::{HashMap, HashSet};

use crate::{
    ast::{Expr, Stmt, StmtKind, Var, visit_stmt_expr},
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
        // Find the statement that defines this variable and remove it.
        let stmt = blocks
            .iter_mut()
            .find_map(|block| {
                let i = block.stmts.iter().position(|stmt| {
                    let StmtKind::Set(Expr::Var(var), _) = &stmt.kind else {
                        return false;
                    };
                    *var == inline_var
                })?;
                Some(block.stmts.remove(i))
            })
            .unwrap();

        let Stmt {
            ip,
            kind: StmtKind::Set(_, val),
        } = stmt
        else {
            unreachable!()
        };

        'inline_target: for block in blocks.iter_mut() {
            for stmt in block.stmts.iter_mut() {
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
                    stmt.ip.splice(0..0, ip);
                    break 'inline_target;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Stmt;
    use crate::parse::Parse;

    #[test]
    fn combines_ips_when_inlining() {
        let mut blocks = vec![Block {
            ip: 0x10,
            stmts: vec![
                Stmt {
                    ip: vec![0x10],
                    kind: StmtKind::must_parse("(set x 1)"),
                },
                Stmt {
                    ip: vec![0x12],
                    kind: StmtKind::must_parse("(foo x)"),
                },
            ],
        }];

        inline(&mut blocks);

        insta::assert_snapshot!(format!("{}", blocks.iter().map(|b| format!("{}", b)).collect::<Vec<_>>().join("\n")), @"
        10:
        00000010,00000012 (foo 1)
        ");
    }
}
