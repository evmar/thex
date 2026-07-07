use crate::ast::{Call, Expr, Stmt, StmtKind};

/// Simplify `xor eax, eax` => setting eax to 0.
fn xor(stmt: &StmtKind) -> Option<StmtKind> {
    let StmtKind::Set(left, Expr::Call(call)) = stmt else {
        return None;
    };
    let "^" = call.func.as_str() else { return None };
    let [arg1, arg2] = call.args.as_slice() else {
        return None;
    };
    if left != arg1 || left != arg2 {
        return None;
    }
    Some(StmtKind::Set(left.clone(), 0.into()))
}

/// Simplify a test followed by je.
fn test_je(stmts: (&StmtKind, &StmtKind)) -> Option<StmtKind> {
    // First stmt looks like (set _ (- expr var))
    let StmtKind::Set(left, expr) = stmts.0 else {
        return None;
    };
    let Expr::Var(var) = left else { return None };
    let "_" = var.as_str() else { return None };

    // Expr is (- expr var)
    let Expr::Call(call) = expr else { return None };
    let test = call.func.as_str();
    let args = call.args.as_slice();

    let StmtKind::Jmp(cond, dst) = stmts.1 else {
        return None;
    };
    let Expr::Call(call) = cond else {
        return None;
    };
    let jmp = call.func.as_str();
    assert!(call.args.is_empty());

    let cond = match (test, jmp) {
        ("cmp", "je") => Call {
            func: "=".into(),
            args: args.to_vec(),
        },
        ("test", "jne") => {
            let [left, right] = args else {
                unreachable!();
            };
            if left != right {
                return None;
            }
            Call {
                func: "!=".into(),
                args: vec![args[0].clone(), 0.into()],
            }
        }
        _ => return None,
    };

    Some(StmtKind::Jmp(cond.into(), dst.clone()))
}

pub fn simp(stmts: Vec<Stmt>) -> Vec<Stmt> {
    let mut stmts = stmts;
    let mut i = 0;
    while i < stmts.len() {
        let stmt = &stmts[i].kind;
        if let Some(s) = xor(&stmt) {
            stmts[i].kind = s;
            continue;
        }

        if i + 1 < stmts.len() {
            let next = &stmts[i + 1].kind;
            if let Some(s) = test_je((&stmt, &next)) {
                stmts.remove(i);
                stmts[i].kind = s;
                continue;
            }
        }

        i += 1;
    }

    stmts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simp(instrs: &[iced_x86::Instruction]) -> String {
        let stmts = instrs
            .into_iter()
            .enumerate()
            .map(|(i, instr)| Stmt {
                instr: i,
                kind: StmtKind::from(instr),
            })
            .collect::<Vec<_>>();
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

    #[test]
    fn cmp_je() -> anyhow::Result<()> {
        use iced_x86::code_asm::*;
        let mut a = CodeAssembler::new(32)?;
        a.cmp(eax, ebx)?;
        a.je(4)?;
        insta::assert_snapshot!(simp(a.instructions()), @"(jmp (= eax ebx) 0x4)");
        Ok(())
    }

    #[test]
    fn test_jmp() -> anyhow::Result<()> {
        use iced_x86::code_asm::*;
        let mut a = CodeAssembler::new(32)?;
        a.test(eax, ebx)?;
        a.jne(4)?;
        a.test(eax, eax)?;
        a.jne(4)?;
        insta::assert_snapshot!(simp(a.instructions()), @"
        (set _ (test eax ebx))
        (jmp (jne) 0x4)
        (jmp (!= eax 0x0) 0x4)
        ");
        Ok(())
    }
}
