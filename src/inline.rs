use std::collections::HashMap;

use crate::{
    ast::{Expr, Stmt, StmtKind, Var, visit_stmt_expr, visit_stmt_expr_mut},
    simp,
    ssa::Block,
};

pub fn inline_once(blocks: &mut [Block]) -> bool {
    let mut var_defs = HashMap::<Var, &Expr>::new();
    let mut var_uses = HashMap::<Var, usize>::new();
    for block in blocks.iter() {
        for stmt in block.stmts.iter() {
            if let StmtKind::Let(var, val) = &stmt.kind {
                var_defs.insert(*var, val);
            }
            visit_stmt_expr(stmt, &mut |expr| {
                let Expr::Var(var) = expr else {
                    return;
                };
                *var_uses.entry(*var).or_default() += 1;
            });
        }
    }

    let mut to_inline = vec![];
    for (var, val) in var_defs {
        let count = match var_uses.get(&var) {
            None => {
                eprintln!("BUG: {var} def but not use?");
                0
            }
            Some(count) => *count,
        };
        let is_alias = matches!(val, Expr::Name(_));
        if count == 1 || is_alias {
            to_inline.push(var);
        }
    }

    let inlined = !to_inline.is_empty();
    for inline_var in to_inline {
        // Find the statement that defines this variable and remove it.
        let stmt = blocks
            .iter_mut()
            .find_map(|block| {
                let i = block.stmts.iter().position(|stmt| {
                    let StmtKind::Let(var, _) = &stmt.kind else {
                        return false;
                    };
                    *var == inline_var
                })?;
                Some(block.stmts.remove(i))
            })
            .unwrap();

        let Stmt {
            ip,
            kind: StmtKind::Let(_, val),
        } = stmt
        else {
            unreachable!()
        };

        for block in blocks.iter_mut() {
            for stmt in block.stmts.iter_mut() {
                let mut inlined = false;
                visit_stmt_expr_mut(stmt, &mut |expr| {
                    let Expr::Var(var) = expr else {
                        return;
                    };
                    if *var == inline_var {
                        *expr = val.clone();
                        inlined = true;
                    }
                });
                if inlined {
                    visit_stmt_expr_mut(stmt, &mut |expr| {
                        if let Some(new) = simp::math_constants(expr) {
                            *expr = new;
                        }
                        if let Some(new) = simp::phi(expr) {
                            *expr = new;
                        }
                    });
                    stmt.ip.extend(ip.clone());
                    stmt.ip.sort();
                }
            }
        }
    }
    inlined
}

pub fn inline(blocks: &mut [Block]) {
    while inline_once(blocks) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Stmt;
    use crate::ssa::fmt_blocks;

    #[test]
    fn combines_ips_when_inlining() {
        let stmts = Stmt::parse_many(
            "1: (set x 1)
            2: (foo x)",
        );
        let mut blocks = crate::ssa::ssa(stmts);
        inline(&mut blocks);

        insta::assert_snapshot!(fmt_blocks(&blocks, true), @"
        1:
        00000001,00000002 (foo 1)
        ");
    }

    #[test]
    fn inlines_phis() {
        let stmts = Stmt::parse_many(
            "0: (set x 1)
            (jmp (jne) 2)
            1: (set y x)
            (use y)
            (jmp (jmp) 3)
            2: (set y x)
            (use y)
            3: (use x)",
        );
        let mut blocks = crate::ssa::ssa(stmts);
        inline(&mut blocks);
        insta::assert_snapshot!(fmt_blocks(&blocks, false), @"
        0:
        (let v1 1)
        (jmp (jne) 2)

        1:
        (let v3 v1)
        (use v3)
        (jmp (jmp) 3)

        2:
        (let v5 v1)
        (use v5)

        3:
        (use (phi v3 v5))
        ");
    }
}
