pub mod codeblock;
pub mod compiler;
pub mod opcode;
pub mod peephole;

pub use codeblock::{CodeBlock, Constant, ImportSpec, ModuleExport, ModuleImport, ModuleInfo};
pub use compiler::Compiler;
pub use opcode::{Instruction, Opcode};
pub use peephole::{optimize, optimize_recursive};
