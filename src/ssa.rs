use std::collections::HashMap;

use crate::ast::{Call, Expr, Stmt, StmtKind, Var, visit_stmt_expr};

pub struct Block {
    pub ip: u32,
    pub stmts: Vec<Stmt>,
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

#[derive(Default)]
struct Syms(HashMap<Var, u8>);
impl Syms {
    fn next(&mut self, var: &str) -> Var {
        let next: u8 = self.0.get(var).copied().unwrap_or(0) + 1;
        self.0.insert(Var::new(var), next);
        Var::new(format!("{var}{next}"))
    }
}

fn ssa_names(block: &mut Block, syms: &mut Syms) -> (HashMap<Var, Var>, HashMap<Var, Var>) {
    let mut reads: HashMap<Var, Var> = HashMap::new();
    let mut writes: HashMap<Var, Var> = HashMap::new();
    for stmt in block.stmts.iter_mut() {
        visit_stmt_expr(stmt, &mut |expr| {
            let Expr::Var(var) = expr else {
                return;
            };
            match writes.get(var) {
                Some(new_var) => *var = new_var.clone(),
                None => {
                    // read of var we haven't written means it's a block input
                    let new_var = syms.next(var);
                    reads.insert(var.clone(), new_var.clone()); // never overwritten
                    writes.insert(var.clone(), new_var.clone()); // may be overwritten
                    *var = new_var;
                }
            }
        });
        if let StmtKind::Set(Expr::Var(var), _) = &mut stmt.kind {
            let new_var = syms.next(var);
            writes.insert(var.clone(), new_var.clone());
            *var = new_var;
        }
    }

    (reads, writes)
}

fn nexts(blocks: &[Block], block: usize) -> impl Iterator<Item = usize> {
    let mut fallthrough = Some(block + 1);

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

    let mut syms = Syms::default();
    let mut block_ins: Vec<HashMap<Var, Var>> = vec![];
    let mut block_outs: Vec<HashMap<Var, Var>> = vec![];
    for block in blocks.iter_mut() {
        let (ins, outs) = ssa_names(block, &mut syms);
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
                        let new_var = syms.next(var);
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

    let mut phis: HashMap<Var, Vec<Var>> = HashMap::new();
    for (var, new) in block_ins[0].iter() {
        phis.insert(new.clone(), vec![format!("{var}_in").into()]);
    }

    for (cur, ins) in block_ins.iter().enumerate() {
        for (var, new) in ins.iter() {
            let phi = phis.entry(new.clone()).or_insert_with(Default::default);
            for &prev in block_preds[cur].iter() {
                let out = block_outs[prev].get(var).unwrap();
                if out == new {
                    // if phi references itself, ignore it.
                    // this is the case where a block loops an unmodified input back to itself.
                    continue;
                }
                phi.push(out.clone());
            }
            //eprintln!("{new}: {phi:?}");
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

            blocks[cur].stmts.insert(
                0,
                Stmt {
                    ip: vec![],
                    kind: StmtKind::Set(new.clone().into(), val),
                },
            );
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
        let mut syms = Syms::default();
        super::ssa_names(&mut block, &mut syms);
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
        a.mov(eax, ebx)?;
        a.mov(ebx, eax)?;
        a.mov(eax, edx)?;
        a.mov(ecx, eax)?;
        insta::assert_snapshot!(ssa_names(a.instructions()), @"
        (set eax1 ebx1)
        (set ebx2 eax1)
        (set eax2 edx1)
        (set ecx1 eax2)
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
        (set ax1 0)
        1:
        (set ax2 (phi ax1 ax3))
        (set ax3 (+ ax2 1))
        (jmp (jmp) 1)
        ");
        Ok(())
    }

    #[test]
    fn two_blocks() -> anyhow::Result<()> {
        let code = {
            use iced_x86::code_asm::*;
            let mut a = CodeAssembler::new(32)?;
            a.mov(ax, 0)?;

            // ax gets forwarded through this block
            let mut b1 = a.create_label();
            a.set_label(&mut b1)?;
            a.cld()?;

            let mut fwd = a.create_label();
            a.jmp(fwd)?;
            a.set_label(&mut fwd)?;
            a.add(ax, 1)?;
            a.jmp(b1)?;
            a.take_instructions()
        };
        insta::assert_snapshot!(ssa(&code)?, @"
        0:
        (set ax1 0)
        1:
        (set ax4 (phi ax1 ax3))
        (cld)
        (jmp (jmp) 2)
        2:
        (set ax2 ax4)
        (set ax3 (+ ax2 1))
        (jmp (jmp) 1)
        ");
        Ok(())
    }
}
