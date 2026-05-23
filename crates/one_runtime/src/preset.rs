use one_vm::Vm;

use crate::{
    array, boolean, collections, console, date, error, globals, json, math, number, object,
    promise, regexp, string, symbol, timers,
};
#[cfg(feature = "net")]
use crate::net;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Preset {
    #[default]
    Full,
    Minimal,
    Sandbox,
    Custom(Vec<BuiltinModule>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinModule {
    Console,
    Math,
    JSON,
    Promise,
    Timers,
    Object,
    Array,
    String,
    Number,
    Date,
    RegExp,
    Map,
    Set,
    Symbol,
    Error,
    Globals,
    #[cfg(feature = "net")]
    Net,
}

/// Default instruction fuel for [`Preset::Sandbox`] when no explicit limit is set.
pub const SANDBOX_DEFAULT_FUEL: u64 = 1_000_000;

pub fn install_preset(vm: &mut Vm, preset: &Preset) {
    match preset {
        Preset::Full => crate::install_builtins(vm),
        Preset::Minimal => {
            object::install_object(vm);
            error::install_error(vm);
        }
        Preset::Sandbox => {
            console::install_console(vm);
            promise::install_promise(vm);
            object::install_object(vm);
            array::install_array(vm);
            collections::install_collections(vm);
            string::install_string(vm);
            number::install_number(vm);
            boolean::install_boolean(vm);
            math::install_math(vm);
            json::install_json(vm);
            error::install_error(vm);
            date::install_date(vm);
            symbol::install_symbol(vm);
            regexp::install_regexp(vm);
        }
        Preset::Custom(modules) => {
            let mut collections_installed = false;
            for module in modules {
                match module {
                    BuiltinModule::Console => console::install_console(vm),
                    BuiltinModule::Math => math::install_math(vm),
                    BuiltinModule::JSON => json::install_json(vm),
                    BuiltinModule::Promise => promise::install_promise(vm),
                    BuiltinModule::Timers => timers::install_timers(vm),
                    BuiltinModule::Object => object::install_object(vm),
                    BuiltinModule::Array => array::install_array(vm),
                    BuiltinModule::String => string::install_string(vm),
                    BuiltinModule::Number => number::install_number(vm),
                    BuiltinModule::Date => date::install_date(vm),
                    BuiltinModule::RegExp => regexp::install_regexp(vm),
                    BuiltinModule::Map | BuiltinModule::Set => {
                        if !collections_installed {
                            collections::install_collections(vm);
                            collections_installed = true;
                        }
                    }
                    BuiltinModule::Symbol => symbol::install_symbol(vm),
                    BuiltinModule::Error => error::install_error(vm),
                    BuiltinModule::Globals => globals::install_globals(vm),
                    #[cfg(feature = "net")]
                    BuiltinModule::Net => net::install_net(vm),
                }
            }
        }
    }
}
