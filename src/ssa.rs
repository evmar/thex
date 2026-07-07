use crate::ast::Stmt;

struct Block {
    stmts: Vec<Stmt>,
}

fn blocks(stmts: Vec<Stmt>) -> Vec<Block> {
    let mut blocks: Vec<Block> = vec![];
    let mut block_stmts: Vec<Stmt> = vec![];
    for stmt in stmts {
        let stmt = block_stmts.push_mut(stmt);
        if let Stmt::Jmp(_, _) = &stmt {
            blocks.push(Block { stmts: block_stmts });
            block_stmts = vec![];
        }
    }
    assert!(block_stmts.is_empty());
    blocks
}

pub fn ssa(stmts: Vec<Stmt>) -> Vec<Stmt> {
    let blocks = blocks(stmts);
    for block in blocks {
        println!("block:");
        for stmt in block.stmts {
            println!("  {}", stmt);
        }
        println!();
    }
    todo!();
}
