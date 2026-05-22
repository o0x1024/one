pub mod console;

use one_vm::Vm;

/// Install all built-in runtime APIs
pub fn install_builtins(vm: &mut Vm) {
    console::install_console(vm);
}
