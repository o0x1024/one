use one_core::JsValue;

use crate::opcode::Instruction;

/// Compiled function/script bytecode
#[derive(Debug, Clone)]
pub struct CodeBlock {
    pub name: String,
    pub bytecode: Vec<Instruction>,
    pub constants: Vec<JsValue>,
    pub string_constants: Vec<String>, // separate pool for string values
    pub register_count: u16,
    pub param_count: u16,
    pub upvalue_count: u16,
    pub is_strict: bool,
    pub is_async: bool,
    pub is_generator: bool,
    pub inner_functions: Vec<CodeBlock>,
    pub source_map: Vec<SourceMapping>,
    pub exception_handlers: Vec<ExceptionHandler>,
}

#[derive(Debug, Clone)]
pub struct SourceMapping {
    pub bytecode_offset: u32,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone)]
pub struct ExceptionHandler {
    pub try_start: u32,
    pub try_end: u32,
    pub catch_start: u32,
    pub finally_start: Option<u32>,
    pub catch_register: u8,
}

impl CodeBlock {
    pub fn new(name: String) -> Self {
        CodeBlock {
            name,
            bytecode: Vec::new(),
            constants: Vec::new(),
            string_constants: Vec::new(),
            register_count: 0,
            param_count: 0,
            upvalue_count: 0,
            is_strict: false,
            is_async: false,
            is_generator: false,
            inner_functions: Vec::new(),
            source_map: Vec::new(),
            exception_handlers: Vec::new(),
        }
    }

    pub fn emit(&mut self, instruction: Instruction) -> u32 {
        let offset = self.bytecode.len() as u32;
        self.bytecode.push(instruction);
        offset
    }

    pub fn add_constant(&mut self, value: JsValue) -> u16 {
        let idx = self.constants.len();
        self.constants.push(value);
        idx as u16
    }

    pub fn add_string_constant(&mut self, s: String) -> u16 {
        // Dedup strings
        if let Some(idx) = self.string_constants.iter().position(|existing| existing == &s) {
            return idx as u16;
        }
        let idx = self.string_constants.len();
        self.string_constants.push(s);
        idx as u16
    }

    /// Patch a jump instruction's offset (for forward jumps resolved later)
    pub fn patch_jump(&mut self, instr_offset: u32, target: u32) {
        let delta = target as i32 - instr_offset as i32 - 1;
        let instr = &mut self.bytecode[instr_offset as usize];
        let op = instr.opcode();
        let a = instr.a();
        *instr = Instruction::asbx(op, a, delta as i16);
    }

    pub fn current_offset(&self) -> u32 {
        self.bytecode.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcode::{Instruction, Opcode};
    use one_core::JsValue;

    #[test]
    fn emit_and_read_back() {
        let mut cb = CodeBlock::new("test".into());
        cb.emit(Instruction::abx(Opcode::LoadConst, 0, 0));
        cb.emit(Instruction::abc(Opcode::Add, 0, 1, 2));
        cb.emit(Instruction::op_only(Opcode::ReturnUndef));
        assert_eq!(cb.bytecode.len(), 3);
        assert_eq!(cb.bytecode[0].opcode(), Opcode::LoadConst);
        assert_eq!(cb.bytecode[1].opcode(), Opcode::Add);
        assert_eq!(cb.bytecode[2].opcode(), Opcode::ReturnUndef);
    }

    #[test]
    fn constant_pool() {
        let mut cb = CodeBlock::new("test".into());
        let idx0 = cb.add_constant(JsValue::from_f64(3.14));
        let idx1 = cb.add_constant(JsValue::from_i32(42));
        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
        assert_eq!(cb.constants.len(), 2);
    }

    #[test]
    fn string_constant_dedup() {
        let mut cb = CodeBlock::new("test".into());
        let a = cb.add_string_constant("hello".into());
        let b = cb.add_string_constant("world".into());
        let c = cb.add_string_constant("hello".into());
        assert_eq!(a, 0);
        assert_eq!(b, 1);
        assert_eq!(c, 0); // deduped
        assert_eq!(cb.string_constants.len(), 2);
    }

    #[test]
    fn patch_forward_jump() {
        let mut cb = CodeBlock::new("test".into());
        let jump_pos = cb.emit(Instruction::asbx(Opcode::JumpIfFalse, 0, 0)); // placeholder
        cb.emit(Instruction::abx(Opcode::LoadConst, 0, 0));
        cb.emit(Instruction::abx(Opcode::LoadConst, 1, 1));
        let target = cb.current_offset();
        cb.patch_jump(jump_pos, target);
        // Jump should skip 2 instructions
        assert_eq!(cb.bytecode[jump_pos as usize].sbx(), 2);
    }
}
