use std::sync::atomic::{AtomicU64, Ordering};

use one_core::JsValue;
use one_vm::Vm;

static RNG_STATE: AtomicU64 = AtomicU64::new(0x853c49e6748fea9b);

fn math_random() -> f64 {
    let mut state = RNG_STATE.load(Ordering::Relaxed);
    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
    RNG_STATE.store(state, Ordering::Relaxed);
    (state >> 11) as f64 / ((1u64 << 53) as f64)
}

fn num_arg(args: &[JsValue], idx: usize) -> f64 {
    args.get(idx).map(|v| v.to_number()).unwrap_or(f64::NAN)
}

fn set_math_constants(vm: &mut Vm) {
    let math_val = vm.get_global("Math");
    if let Some(obj) = vm.get_object_mut(math_val) {
        obj.set_property("PI".to_string(), JsValue::from_f64(std::f64::consts::PI));
        obj.set_property("E".to_string(), JsValue::from_f64(std::f64::consts::E));
        obj.set_property("LN2".to_string(), JsValue::from_f64(std::f64::consts::LN_2));
        obj.set_property("LN10".to_string(), JsValue::from_f64(std::f64::consts::LN_10));
        obj.set_property(
            "LOG2E".to_string(),
            JsValue::from_f64(std::f64::consts::LOG2_E),
        );
        obj.set_property(
            "LOG10E".to_string(),
            JsValue::from_f64(std::f64::consts::LOG10_E),
        );
        obj.set_property("SQRT2".to_string(), JsValue::from_f64(std::f64::consts::SQRT_2));
    }
}

pub fn install_math(vm: &mut Vm) {
    vm.register_host_fn("Math.abs", |_vm, args| Ok(JsValue::from_f64(num_arg(args, 0).abs())));

    vm.register_host_fn("Math.floor", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).floor()))
    });

    vm.register_host_fn("Math.ceil", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).ceil()))
    });

    vm.register_host_fn("Math.round", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).round()))
    });

    vm.register_host_fn("Math.max", |_vm, args| {
        if args.is_empty() {
            Ok(JsValue::from_f64(f64::NEG_INFINITY))
        } else {
            let max = args.iter().map(|v| v.to_number()).fold(f64::NEG_INFINITY, f64::max);
            Ok(JsValue::from_f64(max))
        }
    });

    vm.register_host_fn("Math.min", |_vm, args| {
        if args.is_empty() {
            Ok(JsValue::from_f64(f64::INFINITY))
        } else {
            let min = args.iter().map(|v| v.to_number()).fold(f64::INFINITY, f64::min);
            Ok(JsValue::from_f64(min))
        }
    });

    vm.register_host_fn("Math.pow", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).powf(num_arg(args, 1))))
    });

    vm.register_host_fn("Math.sqrt", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).sqrt()))
    });

    vm.register_host_fn("Math.random", |_vm, _args| Ok(JsValue::from_f64(math_random())));

    vm.register_host_fn("Math.sign", |_vm, args| {
        let n = num_arg(args, 0);
        let sign = if n.is_nan() {
            f64::NAN
        } else if n > 0.0 {
            1.0
        } else if n < 0.0 {
            -1.0
        } else {
            0.0
        };
        Ok(JsValue::from_f64(sign))
    });

    vm.register_host_fn("Math.trunc", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).trunc()))
    });

    vm.register_host_fn("Math.log", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).ln()))
    });

    vm.register_host_fn("Math.log2", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).log2()))
    });

    vm.register_host_fn("Math.log10", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).log10()))
    });

    vm.register_host_fn("Math.sin", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).sin()))
    });

    vm.register_host_fn("Math.cos", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).cos()))
    });

    vm.register_host_fn("Math.tan", |_vm, args| {
        Ok(JsValue::from_f64(num_arg(args, 0).tan()))
    });

    set_math_constants(vm);
}
