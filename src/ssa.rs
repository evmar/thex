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

pub fn ssa(stmts: Vec<Stmt>) -> Vec<Block> {
    let blocks = blocks(stmts);

    blocks
}
