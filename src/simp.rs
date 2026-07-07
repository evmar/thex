use crate::ast::{Expr, Stmt};

/// Simplify `xor eax, eax` => setting eax to 0.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn simp(instrs: &[iced_x86::Instruction]) -> String {
        let stmts = instrs.into_iter().map(Stmt::from).collect::<Vec<_>>();
        let stmts = super::simp(stmts);
        stmts
            .into_iter()
            .map(|i| format!("{i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn xor() -> anyhow::Result<()> {
        use iced_x86::code_asm::*;
        let mut a = CodeAssembler::new(32)?;
        a.xor(eax, ebx)?; // no simp
        a.xor(ebx, ebx)?; // simp
        insta::assert_snapshot!(simp(a.instructions()), @"
        (set eax (^ eax ebx))
        (set ebx 0x0)
        ");
        Ok(())
    }
}
