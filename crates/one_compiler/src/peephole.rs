use crate::codeblock::CodeBlock;
use crate::opcode::{Instruction, Opcode};

pub fn optimize(code: &mut CodeBlock) {
    let bytecode = &mut code.bytecode;
    let len = bytecode.len();

    for i in 0..len {
        if i + 1 >= len {
            continue;
        }

        // Not + JumpIfTrue → JumpIfFalse (eliminate double negation)
        if bytecode[i].opcode() == Opcode::Not && bytecode[i + 1].opcode() == Opcode::JumpIfTrue {
            let not_dest = bytecode[i].a();
            let jump_cond = bytecode[i + 1].a();
            if not_dest == jump_cond {
                let src = bytecode[i].b();
                bytecode[i + 1] = Instruction::asbx(
                    Opcode::JumpIfFalse,
                    src,
                    bytecode[i + 1].sbx(),
                );
                bytecode[i] = Instruction::op_only(Opcode::Nop);
            }
            continue;
        }

        // Not + JumpIfFalse → JumpIfTrue
        if bytecode[i].opcode() == Opcode::Not && bytecode[i + 1].opcode() == Opcode::JumpIfFalse
        {
            let not_dest = bytecode[i].a();
            let jump_cond = bytecode[i + 1].a();
            if not_dest == jump_cond {
                let src = bytecode[i].b();
                bytecode[i + 1] = Instruction::asbx(
                    Opcode::JumpIfTrue,
                    src,
                    bytecode[i + 1].sbx(),
                );
                bytecode[i] = Instruction::op_only(Opcode::Nop);
            }
        }
    }
}

pub fn optimize_recursive(code: &mut CodeBlock) {
    optimize(code);
    for inner in &mut code.inner_functions {
        optimize_recursive(inner);
    }
}
