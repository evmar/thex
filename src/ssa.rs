use std::collections::HashMap;

use crate::{
    ast::{Call, Expr, Name, Stmt, StmtKind, Var, visit_stmt_expr_mut},
    simp::simp_stmt,
};

#[derive(Debug)]
pub struct Block {
    pub ip: u32,
    pub stmts: Vec<Stmt>,
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{:x}:", self.ip)?;
        for s in self.stmts.iter() {
            if f.alternate() {
                writeln!(f, "{:#}", s)?;
            } else {
                writeln!(f, "{}", s)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
pub fn fmt_blocks(blocks: &[Block], include_ip: bool) -> String {
    blocks
        .iter()
        .map(|b| {
            if include_ip {
                format!("{b:#}")
            } else {
                format!("{b}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

impl From<Vec<Stmt>> for Block {
    fn from(stmts: Vec<Stmt>) -> Self {
        Block {
            ip: stmts[0].ip[0],
            stmts,
        }
    }
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
                    if next.ip.iter().any(|ip| jmp_targets.contains(ip)) {
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
        ip: stmts[0].ip[0],
        stmts,
    })
    .collect()
}

fn ssa_names(block: &mut Block, next_var: &mut Var) -> (HashMap<Name, Var>, HashMap<Name, Var>) {
    let mut reads: HashMap<Name, Var> = HashMap::new();
    let mut writes: HashMap<Name, Var> = HashMap::new();

    for stmt in block.stmts.iter_mut() {
        // set => let
        // given (set x (some_expr x)) we want the inner x to refer to a previous x,
        // so convert to let here, then traverse the expression, then update the outer x.
        let pending = if let StmtKind::Set(Expr::Name(name), val) = &mut stmt.kind {
            let new_var = *next_var;
            *next_var = next_var.next();
            let name = name.clone();
            stmt.kind = StmtKind::Let(new_var, val.clone());
            Some((name, new_var))
        } else {
            None
        };

        visit_stmt_expr_mut(stmt, &mut |expr| {
            let Expr::Name(name) = expr else {
                return;
            };
            match writes.get(name) {
                Some(new_var) => *expr = Expr::Var(*new_var),
                None => {
                    // read of name we haven't written means it's a block input
                    let new_var = *next_var;
                    *next_var = next_var.next();
                    reads.insert(name.clone(), new_var); // never overwritten
                    writes.insert(name.clone(), new_var); // may be overwritten
                    *expr = Expr::Var(new_var);
                }
            }
        });

        if let Some((name, var)) = pending {
            writes.insert(name, var); // may be overwritten
        }
    }

    (reads, writes)
}

fn nexts(blocks: &[Block], block: usize) -> impl Iterator<Item = usize> {
    let mut fallthrough = if block + 1 < blocks.len() {
        Some(block + 1)
    } else {
        None
    };

    let last = blocks[block].stmts.last().unwrap();
    let jmp = if let StmtKind::Jmp(cond, dst) = &last.kind {
        if cond.func == "jmp" || cond.func == "ret" {
            fallthrough = None;
        }
        if let Expr::Val(addr) = dst {
            Some(blocks.iter().position(|b| b.ip == *addr).unwrap())
        } else {
            // uhoh
            None
        }
    } else {
        None
    };
    [fallthrough, jmp].into_iter().filter_map(|x| x)
}

pub fn ssa(stmts: Vec<Stmt>) -> Vec<Block> {
    let mut blocks = blocks(stmts);

    let mut block_preds = Vec::<Vec<usize>>::new();
    block_preds.resize_with(blocks.len(), Default::default);
    for src in 0..blocks.len() {
        for dst in nexts(&blocks, src) {
            block_preds[dst].push(src);
        }
    }

    let mut next_var = Var(1);
    let mut block_ins: Vec<HashMap<Name, Var>> = vec![];
    let mut block_outs: Vec<HashMap<Name, Var>> = vec![];
    for block in blocks.iter_mut() {
        let (ins, outs) = ssa_names(block, &mut next_var);
        block_ins.push(ins);
        block_outs.push(outs);
    }

    // If block A -> block B, and B uses some var not in A, add an entry to A's ins and outs.
    // Loop until stability.
    loop {
        let mut changed = false;
        for (cur, _) in blocks.iter().enumerate() {
            for &prev in block_preds[cur].iter() {
                if cur == prev {
                    continue;
                }
                let [ins, prev_ins] = block_ins.get_disjoint_mut([cur, prev]).unwrap();
                let prev_outs = &mut block_outs[prev];
                for var in ins.keys() {
                    if prev_outs.get(var).is_none() {
                        let new_var = next_var;
                        next_var = next_var.next();
                        prev_ins.insert(var.clone(), new_var.clone());
                        prev_outs.insert(var.clone(), new_var.clone());
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut phis: HashMap<Var, Vec<Expr>> = HashMap::new();
    for (name, var) in block_ins[0].iter() {
        phis.insert(*var, vec![format!("{name}_in").into()]);
    }

    for (cur, ins) in block_ins.iter().enumerate() {
        for (name, var) in ins.iter() {
            let phi = phis.entry(*var).or_insert_with(Default::default);
            for &prev in block_preds[cur].iter() {
                let out = block_outs[prev].get(name).unwrap();
                let new = Expr::Var(*out);
                phi.push(new);
            }
        }
    }

    for (cur, ins) in block_ins.iter().enumerate() {
        for (var, new) in ins.iter() {
            let phi = phis.get(new).unwrap();
            let val: Expr = match phi.len() {
                0 => Call {
                    func: "todo".into(), // var is input to whole program (or a bug)
                    args: vec![var.clone().into()],
                }
                .into(),
                1 => phi[0].clone().into(),
                _ => Call {
                    func: "phi".into(),
                    args: phi.iter().map(|var| var.clone().into()).collect(),
                }
                .into(),
            };

            let mut stmt = Stmt {
                ip: vec![],
                kind: StmtKind::Let(*new, val),
            };
            // simplify e.g (let x (phi x y))
            // this also happens when inlining, but only for statements affected by inlining
            simp_stmt(&mut stmt);
            blocks[cur].stmts.insert(0, stmt);
        }
    }

    blocks
}

/*
fn rename() {
    // The entry point block includes the original register values as inputs.
    for (var, (_, vars)) in block_ins[0].iter_mut() {
        vars.insert(var.clone());
    }

    // Union any vars used together into a set.
    let mut u = Union::new();
    for ins in block_ins.iter() {
        for (_, (new_var, vars)) in ins.iter() {
            for var in vars {
                u.join(new_var, var);
            }
        }
    }
    // println!("{:?}", u.sets());

    // Replace vars with their union representative.
    for block in blocks.iter_mut() {
        for stmt in block.stmts.iter_mut() {
            if let StmtKind::Set(var, _) = &mut stmt.kind {
                if let Expr::Var(var) = var {
                    if let Some(new_var) = u.lookup(var) {
                        *var = new_var.clone();
                    }
                }
            }
            visit_stmt_expr(stmt, &mut |expr| {
                if let Expr::Var(var) = expr {
                    if let Some(new_var) = u.lookup(var) {
                        *var = new_var.clone();
                    }
                }
            });
        }
    }

    blocks
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    fn ssa_names(instrs: &[iced_x86::Instruction]) -> String {
        let mut block: Block = instrs
            .into_iter()
            .map(Stmt::from)
            .collect::<Vec<_>>()
            .into();
        let mut next_sym = Var(1);
        super::ssa_names(&mut block, &mut next_sym);
        block
            .stmts
            .into_iter()
            .map(|stmt| format!("{}", stmt.kind))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn names() -> anyhow::Result<()> {
        use iced_x86::code_asm::*;
        let mut a = CodeAssembler::new(32)?;
        a.mov(eax, eax)?;
        a.mov(eax, ebx)?;
        a.mov(ebx, eax)?;
        a.mov(eax, edx)?;
        a.mov(ecx, eax)?;
        insta::assert_snapshot!(ssa_names(a.instructions()), @"
        (let v1 v2)
        (let v3 v4)
        (let v5 v3)
        (let v6 v7)
        (let v8 v6)
        ");
        Ok(())
    }

    fn ssa(instrs: &[iced_x86::Instruction]) -> anyhow::Result<String> {
        use std::fmt::Write;
        let block = instrs.into_iter().map(Stmt::from).collect::<Vec<_>>();
        let blocks = super::ssa(block);
        let mut out = String::new();
        for block in blocks {
            writeln!(&mut out, "{:x}:", block.ip)?;
            for stmt in block.stmts {
                writeln!(&mut out, "{}", stmt.kind)?;
            }
        }
        Ok(out)
    }

    #[test]
    fn simple() -> anyhow::Result<()> {
        let code = {
            use iced_x86::code_asm::*;
            let mut a = CodeAssembler::new(32)?;
            a.mov(ax, 0)?;
            let mut b1 = a.create_label();
            a.set_label(&mut b1)?;
            a.add(ax, 1)?;
            a.jmp(b1)?;
            a.take_instructions()
        };
        insta::assert_snapshot!(ssa(&code)?, @"
        0:
        (let v1 0)
        1:
        (let v3 (phi v1 v2))
        (let v2 (+ v3 1))
        (jmp (jmp) 1)
        ");
        Ok(())
    }

    #[test]
    fn branch() -> anyhow::Result<()> {
        let code = {
            use iced_x86::code_asm::*;
            let mut a = CodeAssembler::new(32)?;
            a.mov(ax, 0)?;

            let mut top = a.create_label();
            a.set_label(&mut top)?;
            let mut br_else = a.create_label();

            a.test(ax, 0)?;
            a.jnz(br_else)?;
            let mut br_out = a.create_label();

            a.cld()?;
            a.jmp(br_out)?;

            a.set_label(&mut br_else)?;
            a.cld()?;

            a.set_label(&mut br_out)?;
            a.add(ax, 1)?;
            a.jmp(top)?;
            a.take_instructions()
        };
        insta::assert_snapshot!(ssa(&code)?, @"
        0:
        (let v1 0)
        1:
        (let v2 (phi v1 v3))
        (test v2 0)
        (jmp (jne) 2)
        0:
        (let v5 v2)
        (cld)
        (jmp (jmp) 3)
        2:
        (let v6 v2)
        (cld)
        3:
        (let v4 (phi v5 v6))
        (let v3 (+ v4 1))
        (jmp (jmp) 1)
        ");
        Ok(())
    }
}
