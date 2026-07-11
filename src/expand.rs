//! Expand/rewrite instructions to simpler set.
//!
//! E.g. 'push' is a memory write plus stack manipulation, and
//! writing to al is writing to ax with masking.

use crate::ast::{Expr, Stmt, StmtKind};

fn push(stmt: &Stmt, out: &mut Vec<Stmt>) -> bool {
    let StmtKind::Do(Expr::Call(call)) = &stmt.kind else {
        return false;
    };
    let "push" = call.func.as_str() else {
        return false;
    };
    let [arg] = call.args.as_slice() else {
        unreachable!()
    };

    out.push(Stmt {
        ip: stmt.ip.clone(),
        kind: StmtKind::Set("sp".into(), Expr::call("-", vec!["sp".into(), 4.into()])),
    });
    out.push(Stmt {
        ip: stmt.ip.clone(),
        kind: StmtKind::Set(Expr::call("mem", vec!["sp".into()]), arg.clone()),
    });
    true
}

fn pop(stmt: &Stmt, out: &mut Vec<Stmt>) -> bool {
    let StmtKind::Set(var, Expr::Call(call)) = &stmt.kind else {
        return false;
    };
    let "pop" = call.func.as_str() else {
        return false;
    };

    out.push(Stmt {
        ip: stmt.ip.clone(),
        kind: StmtKind::Set(var.clone(), Expr::call("mem", vec!["sp".into()])),
    });
    out.push(Stmt {
        ip: stmt.ip.clone(),
        kind: StmtKind::Set("sp".into(), Expr::call("+", vec!["sp".into(), 4.into()])),
    });
    true
}

pub fn expand(stmts: Vec<Stmt>) -> Vec<Stmt> {
    let mut out = vec![];
    'stmts: for stmt in stmts {
        for func in &[push, pop] {
            if func(&stmt, &mut out) {
                continue 'stmts;
            }
        }
        out.push(stmt);
    }
    out
}
