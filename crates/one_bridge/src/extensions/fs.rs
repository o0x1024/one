use one_core::{JsValue, OneResult};
use one_engine::extension::{host_fn, Extension, HostFnDescriptor};
use one_vm::Vm;

pub struct FsExtension;

impl FsExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Extension for FsExtension {
    fn name(&self) -> &str {
        "sentinel_fs"
    }

    fn host_functions(&self) -> Vec<HostFnDescriptor> {
        vec![
            host_fn("__fs_read_text_file", fs_read_text_file),
            host_fn("__fs_write_text_file", fs_write_text_file),
            host_fn("__fs_read_file", fs_read_file),
            host_fn("__fs_write_file", fs_write_file),
            host_fn("__fs_mkdir", fs_mkdir),
            host_fn("__fs_read_dir", fs_read_dir),
            host_fn("__fs_stat", fs_stat),
            host_fn("__fs_copy_file", fs_copy_file),
            host_fn("__fs_remove", fs_remove),
            host_fn("__fs_make_temp_file", fs_make_temp_file),
        ]
    }
}

fn fs_read_text_file(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let path = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
    match std::fs::read_to_string(&path) {
        Ok(content) => Ok(vm.alloc_string(content)),
        Err(e) => Err(one_core::OneError::InternalError(format!(
            "Failed to read file '{path}': {e}"
        ))),
    }
}

fn fs_write_text_file(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let path = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
    let content = args.get(1).map(|v| vm.value_to_string(*v)).unwrap_or_default();
    std::fs::write(&path, &content).map_err(|e| {
        one_core::OneError::InternalError(format!("Failed to write file '{path}': {e}"))
    })?;
    Ok(JsValue::undefined())
}

fn fs_read_file(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let path = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
    match std::fs::read(&path) {
        Ok(bytes) => {
            let len = bytes.len() as u32;
            let arr = vm.new_array(len);
            if let Some(obj) = vm.get_object_mut(arr) {
                for (i, byte) in bytes.iter().enumerate() {
                    obj.set_property(i.to_string(), JsValue::from_i32(*byte as i32));
                }
            }
            Ok(arr)
        }
        Err(e) => Err(one_core::OneError::InternalError(format!(
            "Failed to read file '{path}': {e}"
        ))),
    }
}

fn fs_write_file(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let path = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
    let data_val = args.get(1).copied().unwrap_or(JsValue::undefined());

    let mut bytes = Vec::new();
    if let Some(obj) = vm.get_object(data_val) {
        if let one_vm::ObjectKind::Array { length } = obj.kind() {
            for i in 0..*length {
                if let Some(v) = obj.get_property(&i.to_string()) {
                    bytes.push(v.to_number() as u8);
                }
            }
        }
    }

    std::fs::write(&path, &bytes).map_err(|e| {
        one_core::OneError::InternalError(format!("Failed to write file '{path}': {e}"))
    })?;
    Ok(JsValue::undefined())
}

fn fs_mkdir(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let path = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
    let recursive = args
        .get(1)
        .map(|v| !v.is_undefined() && !v.is_null())
        .unwrap_or(false);
    if recursive {
        std::fs::create_dir_all(&path)
    } else {
        std::fs::create_dir(&path)
    }
    .map_err(|e| {
        one_core::OneError::InternalError(format!("Failed to create directory '{path}': {e}"))
    })?;
    Ok(JsValue::undefined())
}

fn fs_read_dir(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let path = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
    let entries: Vec<String> = std::fs::read_dir(&path)
        .map_err(|e| {
            one_core::OneError::InternalError(format!("Failed to read directory '{path}': {e}"))
        })?
        .filter_map(|entry| entry.ok().map(|e| e.file_name().to_string_lossy().to_string()))
        .collect();

    let len = entries.len() as u32;
    let str_vals: Vec<JsValue> = entries.iter().map(|name| vm.alloc_string(name.clone())).collect();
    let arr = vm.new_array(len);
    if let Some(obj) = vm.get_object_mut(arr) {
        for (i, val) in str_vals.into_iter().enumerate() {
            obj.set_property(i.to_string(), val);
        }
    }
    Ok(arr)
}

fn fs_stat(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let path = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
    let meta = std::fs::metadata(&path).map_err(|e| {
        one_core::OneError::InternalError(format!("Failed to stat '{path}': {e}"))
    })?;

    let result = vm.create_object_from_pairs(&[
        ("isFile".to_string(), JsValue::from_bool(meta.is_file())),
        ("isDirectory".to_string(), JsValue::from_bool(meta.is_dir())),
        ("size".to_string(), JsValue::from_f64(meta.len() as f64)),
    ]);
    Ok(result)
}

fn fs_copy_file(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let src = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
    let dst = args.get(1).map(|v| vm.value_to_string(*v)).unwrap_or_default();
    std::fs::copy(&src, &dst).map_err(|e| {
        one_core::OneError::InternalError(format!("Failed to copy '{src}' to '{dst}': {e}"))
    })?;
    Ok(JsValue::undefined())
}

fn fs_remove(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let path = args.first().map(|v| vm.value_to_string(*v)).unwrap_or_default();
    let recursive = args
        .get(1)
        .map(|v| !v.is_undefined() && !v.is_null())
        .unwrap_or(false);
    let meta = std::fs::metadata(&path).map_err(|e| {
        one_core::OneError::InternalError(format!("Failed to remove '{path}': {e}"))
    })?;
    if meta.is_dir() && recursive {
        std::fs::remove_dir_all(&path)
    } else if meta.is_dir() {
        std::fs::remove_dir(&path)
    } else {
        std::fs::remove_file(&path)
    }
    .map_err(|e| {
        one_core::OneError::InternalError(format!("Failed to remove '{path}': {e}"))
    })?;
    Ok(JsValue::undefined())
}

fn fs_make_temp_file(vm: &mut Vm, _args: &[JsValue]) -> OneResult<JsValue> {
    let dir = std::env::temp_dir();
    let name = format!("one_tmp_{}", std::process::id());
    let path = dir.join(&name);
    std::fs::write(&path, "").map_err(|e| {
        one_core::OneError::InternalError(format!("Failed to create temp file: {e}"))
    })?;
    Ok(vm.alloc_string(path.to_string_lossy().to_string()))
}
