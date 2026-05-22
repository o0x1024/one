pub mod array;
pub mod boolean;
pub mod console;
pub mod globals;
pub mod number;
pub mod object;
pub mod promise;
pub mod string;

use one_vm::Vm;

/// Install all built-in runtime APIs
pub fn install_builtins(vm: &mut Vm) {
    console::install_console(vm);
    promise::install_promise(vm);
    globals::install_globals(vm);
    object::install_object(vm);
    array::install_array(vm);
    string::install_string(vm);
    number::install_number(vm);
    boolean::install_boolean(vm);
}
