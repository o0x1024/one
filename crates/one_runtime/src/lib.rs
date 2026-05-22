pub mod console;
pub mod promise;

use one_vm::Vm;

/// Install all built-in runtime APIs
pub fn install_builtins(vm: &mut Vm) {
    console::install_console(vm);
    promise::install_promise(vm);
}
