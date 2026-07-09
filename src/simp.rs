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

/// Simplify (test x x) to (cmp x 0).
/// https://stackoverflow.com/questions/39556649/in-x86-whats-difference-between-test-eax-eax-and-cmp-eax-0
fn test_to_cmp(stmt: &StmtKind) -> Option<StmtKind> {
    let StmtKind::Set(left, Expr::Call(call)) = stmt else {
        return None;
    };
    let "test" = call.func.as_str() else {
        return None;
    };
    let [arg1, arg2] = call.args.as_slice() else {
        return None;
    };
    if arg1 != arg2 {
        return None;
    }
    Some(StmtKind::Set(
        left.clone(),
        Call {
            func: "cmp".into(),
            args: vec![arg1.clone(), 0.into()],
        }
        .into(),
    ))
}

/// Simplify a cmp followed by conditional jmp.
fn cmp_jmp(stmts: (&StmtKind, &StmtKind)) -> Option<StmtKind> {
    // First stmt looks like (set _ (cmp expr var))
    let StmtKind::Set(left, expr) = stmts.0 else {
        return None;
    };
    let Expr::Var(var) = left else { return None };
    let "_" = var.as_str() else { return None };

    // Expr is (cmp expr var)
    let Expr::Call(call) = expr else { return None };
    let "cmp" = call.func.as_str() else {
        return None;
    };
    let args = call.args.as_slice();

    let StmtKind::Jmp(cond, dst) = stmts.1 else {
        return None;
    };
    let jmp = cond.func.as_str();
    assert!(cond.args.is_empty());

    let func = match jmp {
        "je" => "=",
        "jne" => "!=",
        "jge" => ">=",
        "jl" => "<",
        "jle" => "<=",
        _ => return None,
    };

    Some(StmtKind::Jmp(
        Call {
            func: func.into(),
            args: args.to_vec(),
        }
        .into(),
        dst.clone(),
    ))
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
        if let Some(s) = test_to_cmp(&stmt) {
            stmts[i].kind = s;
        }
        let stmt = &stmts[i].kind;

        if i + 1 < stmts.len() {
            let next = &stmts[i + 1].kind;
            if let Some(s) = cmp_jmp((&stmt, &next)) {
                stmts[i].kind = s;
                stmts.remove(i + 1);
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
        let stmts = instrs.into_iter().map(Stmt::from).collect::<Vec<_>>();
        let stmts = super::simp(stmts);
        stmts
            .into_iter()
            .map(|stmt| format!("{}", stmt.kind))
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
        a.cmp(eax, ebx)?;
        a.jne(4)?;
        insta::assert_snapshot!(simp(a.instructions()), @"
        (jmp (= eax ebx) 0x4)
        (jmp (!= eax ebx) 0x4)
        ");
        Ok(())
    }

    #[test]
    fn test_jne() -> anyhow::Result<()> {
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
