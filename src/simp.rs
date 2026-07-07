use crate::ast::{Expr, Stmt};

fn xor(stmt: &Stmt) -> Option<Stmt> {
    let Stmt::Set(left, Expr::Call(call)) = stmt else {
        return None;
    };
    let "^" = call.func.as_str() else { return None };
    let [arg1, arg2] = call.args.as_slice() else {
        return None;
    };
    if left != arg1 || left != arg2 {
        return None;
    }
    Some(Stmt::Set(left.clone(), 0.into()))
}

pub fn simp(stmts: Vec<Stmt>) -> Vec<Stmt> {
    let mut out = vec![];
    for stmt in stmts {
        if let Some(s) = xor(&stmt) {
            out.push(s);
        } else {
            out.push(stmt);
        }
    }
    out
}
