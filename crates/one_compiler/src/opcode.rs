/// 32-bit instruction encoding:
///   ABC format:  [opcode:8][A:8][B:8][C:8]
///   ABx format:  [opcode:8][A:8][Bx:16]
///   AsBx format: [opcode:8][A:8][sBx:16] (signed)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    // Data movement
    LoadConst = 0, // ABx:  r[A] = constants[Bx]
    Move,          // AB:   r[A] = r[B]
    LoadUndef,     // A:    r[A] = undefined
    LoadNull,      // A:    r[A] = null
    LoadTrue,      // A:    r[A] = true
    LoadFalse,     // A:    r[A] = false
    LoadInt,       // AsBx: r[A] = sBx (small integer immediate)

    // Arithmetic
    Add, // ABC:  r[A] = r[B] + r[C]
    Sub, // ABC:  r[A] = r[B] - r[C]
    Mul, // ABC:  r[A] = r[B] * r[C]
    Div, // ABC:  r[A] = r[B] / r[C]
    Mod, // ABC:  r[A] = r[B] % r[C]
    Exp, // ABC:  r[A] = r[B] ** r[C]
    Neg, // AB:   r[A] = -r[B]

    // Bitwise
    BitAnd,  // ABC
    BitOr,   // ABC
    BitXor,  // ABC
    Shl,     // ABC
    Shr,     // ABC
    UShr,    // ABC
    BitNot,  // AB

    // Comparison
    Eq,       // ABC:  r[A] = r[B] == r[C]
    StrictEq, // ABC:  r[A] = r[B] === r[C]
    Lt,       // ABC:  r[A] = r[B] < r[C]
    LtEq,     // ABC:  r[A] = r[B] <= r[C]
    Gt,       // ABC:  r[A] = r[B] > r[C]
    GtEq,     // ABC:  r[A] = r[B] >= r[C]

    // Logical
    Not,        // AB:   r[A] = !r[B]
    TypeOf,     // AB:   r[A] = typeof r[B]
    InstanceOf, // ABC:  r[A] = r[B] instanceof r[C]
    In,         // ABC:  r[A] = r[B] in r[C]

    // Control flow
    Jump,          // sBx:  pc += sBx
    JumpIfTrue,    // AsBx: if r[A] then pc += sBx
    JumpIfFalse,   // AsBx: if !r[A] then pc += sBx
    JumpIfNullish, // AsBx: if r[A] is null/undefined then pc += sBx

    // Property access
    GetProp, // ABC:  r[A] = r[B].constants[C]  (named property)
    SetProp, // ABC:  r[A].constants[B] = r[C]
    GetElem, // ABC:  r[A] = r[B][r[C]]  (computed property)
    SetElem, // ABC:  r[A][r[B]] = r[C]

    // Function call
    Call,       // ABC:  r[A] = r[B](r[B+1]..r[B+C])  A=dest, B=func, C=argc
    CallMethod, // ABC:  r[A] = r[B].method(args)  (method call optimization)
    New,        // ABC:  r[A] = new r[B](r[B+1]..r[B+C])
    Return,     // A:    return r[A]
    ReturnUndef,  //       return undefined

    // Closure / Scope
    CreateClosure, // ABx:  r[A] = closure(inner_functions[Bx])
    GetUpvalue,    // AB:   r[A] = upvalues[B]
    SetUpvalue,    // AB:   upvalues[A] = r[B]
    CloseUpvalue,  // A:    close upvalue at register A

    // Object / Array
    CreateObject, // AB:   r[A] = new Object() with B initial capacity
    CreateArray,  // AB:   r[A] = new Array() with B initial capacity
    SetArrayElem, // ABC:  r[A][B] = r[C]  (B is immediate index)
    InitProp,     // ABC:  r[A].constants[B] = r[C]  (object literal init)
    Spread,       // AB:   spread r[B] into r[A]

    // Exception
    TryStart,  // Bx:   enter try block, handler at current_pc + Bx
    TryEnd,    //       leave try block
    Throw,     // A:    throw r[A]
    CatchBind, // A:    r[A] = caught exception

    // Iterators
    GetIterator,  // AB:   r[A] = r[B][Symbol.iterator]()
    IteratorNext, // AB:   r[A] = r[B].next()
    IteratorDone, // AB:   r[A] = r[B].done

    // Global access
    GetGlobal, // ABx:  r[A] = global[constants[Bx]]
    SetGlobal, // ABx:  global[constants[Bx]] = r[A]

    // Debug / special
    Debugger, //       debugger statement

    // Peephole / specialized
    Nop, //             no operation (peephole placeholder)
    Inc, // AB:        r[A] = r[B] + 1
    Dec, // AB:        r[A] = r[B] - 1

    // Placeholder for future opcodes
    Wide = 255, // Wide prefix for extended operands
}

impl Opcode {
    pub fn from_u8(v: u8) -> Option<Self> {
        if v <= Self::Dec as u8 || v == 255 {
            Some(unsafe { std::mem::transmute::<u8, Opcode>(v) })
        } else {
            None
        }
    }
}

/// A single 32-bit instruction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instruction(pub u32);

impl Instruction {
    pub fn opcode(self) -> Opcode {
        Opcode::from_u8((self.0 & 0xFF) as u8).unwrap_or(Opcode::Wide)
    }
    pub fn a(self) -> u8 {
        ((self.0 >> 8) & 0xFF) as u8
    }
    pub fn b(self) -> u8 {
        ((self.0 >> 16) & 0xFF) as u8
    }
    pub fn c(self) -> u8 {
        ((self.0 >> 24) & 0xFF) as u8
    }
    pub fn bx(self) -> u16 {
        (self.0 >> 16) as u16
    }
    pub fn sbx(self) -> i16 {
        (self.0 >> 16) as i16
    }

    // Constructors
    pub fn abc(op: Opcode, a: u8, b: u8, c: u8) -> Self {
        Instruction((op as u32) | ((a as u32) << 8) | ((b as u32) << 16) | ((c as u32) << 24))
    }
    pub fn abx(op: Opcode, a: u8, bx: u16) -> Self {
        Instruction((op as u32) | ((a as u32) << 8) | ((bx as u32) << 16))
    }
    pub fn asbx(op: Opcode, a: u8, sbx: i16) -> Self {
        Instruction((op as u32) | ((a as u32) << 8) | ((sbx as u16 as u32) << 16))
    }
    pub fn op_only(op: Opcode) -> Self {
        Instruction(op as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_abc_round_trip() {
        let instr = Instruction::abc(Opcode::Add, 1, 2, 3);
        assert_eq!(instr.opcode(), Opcode::Add);
        assert_eq!(instr.a(), 1);
        assert_eq!(instr.b(), 2);
        assert_eq!(instr.c(), 3);
    }

    #[test]
    fn instruction_abx_round_trip() {
        let instr = Instruction::abx(Opcode::LoadConst, 5, 1000);
        assert_eq!(instr.opcode(), Opcode::LoadConst);
        assert_eq!(instr.a(), 5);
        assert_eq!(instr.bx(), 1000);
    }

    #[test]
    fn instruction_asbx_positive() {
        let instr = Instruction::asbx(Opcode::Jump, 0, 42);
        assert_eq!(instr.opcode(), Opcode::Jump);
        assert_eq!(instr.sbx(), 42);
    }

    #[test]
    fn instruction_asbx_negative() {
        let instr = Instruction::asbx(Opcode::Jump, 0, -10);
        assert_eq!(instr.opcode(), Opcode::Jump);
        assert_eq!(instr.sbx(), -10);
    }

    #[test]
    fn opcode_from_u8() {
        assert_eq!(Opcode::from_u8(0), Some(Opcode::LoadConst));
        assert_eq!(Opcode::from_u8(Opcode::Return as u8), Some(Opcode::Return));
        assert_eq!(Opcode::from_u8(255), Some(Opcode::Wide));
        assert_eq!(Opcode::from_u8(200), None);
    }

    #[test]
    fn instruction_max_values() {
        let instr = Instruction::abc(Opcode::Add, 255, 255, 255);
        assert_eq!(instr.a(), 255);
        assert_eq!(instr.b(), 255);
        assert_eq!(instr.c(), 255);
    }
}
