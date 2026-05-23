use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpStream, TcpListener, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use one_core::{JsValue, OneResult};
use one_vm::Vm;

static NEXT_HANDLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

type StreamMap = Arc<Mutex<HashMap<u32, TcpStream>>>;
type ListenerMap = Arc<Mutex<HashMap<u32, TcpListener>>>;

fn streams() -> &'static StreamMap {
    use std::sync::OnceLock;
    static INSTANCE: OnceLock<StreamMap> = OnceLock::new();
    INSTANCE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

fn listeners() -> &'static ListenerMap {
    use std::sync::OnceLock;
    static INSTANCE: OnceLock<ListenerMap> = OnceLock::new();
    INSTANCE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

fn next_handle() -> u32 {
    NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub fn install_tcp(vm: &mut Vm) {
    vm.register_host_fn("net.connect", net_connect);
    vm.register_host_fn("net.write", net_write);
    vm.register_host_fn("net.read", net_read);
    vm.register_host_fn("net.close", net_close);
    vm.register_host_fn("net.listen", net_listen);
    vm.register_host_fn("net.accept", net_accept);
    vm.register_host_fn("net.closeListen", net_close_listen);
    vm.register_host_fn("net.localAddr", net_local_addr);
    vm.register_host_fn("net.remoteAddr", net_remote_addr);
    vm.register_host_fn("net.setReadTimeout", net_set_read_timeout);
    vm.register_host_fn("net.setWriteTimeout", net_set_write_timeout);
}

fn net_connect(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let addr = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();
    let timeout_ms = args.get(1).map(|v| v.to_number() as u64).unwrap_or(5000);

    let socket_addr: SocketAddr = match addr.parse() {
        Ok(a) => a,
        Err(e) => {
            let err = vm.alloc_string(format!("invalid address: {e}"));
            let result = vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("error".into(), err),
            ]);
            return Ok(result);
        }
    };

    match TcpStream::connect_timeout(&socket_addr, Duration::from_millis(timeout_ms)) {
        Ok(stream) => {
            let handle = next_handle();
            streams().lock().unwrap().insert(handle, stream);
            let result = vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(true)),
                ("handle".into(), JsValue::from_i32(handle as i32)),
            ]);
            Ok(result)
        }
        Err(e) => {
            let err = vm.alloc_string(e.to_string());
            let result = vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("error".into(), err),
            ]);
            Ok(result)
        }
    }
}

fn net_write(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let data = args
        .get(1)
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();

    let mut map = streams().lock().unwrap();
    let Some(stream) = map.get_mut(&handle) else {
        return Ok(JsValue::from_i32(-1));
    };

    match stream.write_all(data.as_bytes()) {
        Ok(_) => Ok(JsValue::from_i32(data.len() as i32)),
        Err(_) => Ok(JsValue::from_i32(-1)),
    }
}

fn net_read(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let max_bytes = args.get(1).map(|v| v.to_number() as usize).unwrap_or(4096);

    let mut map = streams().lock().unwrap();
    let Some(stream) = map.get_mut(&handle) else {
        return Ok(JsValue::null());
    };

    let mut buf = vec![0u8; max_bytes];
    match stream.read(&mut buf) {
        Ok(0) => Ok(JsValue::null()),
        Ok(n) => {
            buf.truncate(n);
            let s = String::from_utf8_lossy(&buf).into_owned();
            Ok(vm.alloc_string(s))
        }
        Err(_) => Ok(JsValue::null()),
    }
}

fn net_close(_vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let removed = streams().lock().unwrap().remove(&handle).is_some();
    Ok(JsValue::from_bool(removed))
}

fn net_listen(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let addr = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_else(|| "127.0.0.1:0".into());

    match TcpListener::bind(&addr) {
        Ok(listener) => {
            let local = listener.local_addr().ok();
            let handle = next_handle();
            let port = local.map(|a| a.port()).unwrap_or(0);
            listeners().lock().unwrap().insert(handle, listener);
            let result = vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(true)),
                ("handle".into(), JsValue::from_i32(handle as i32)),
                ("port".into(), JsValue::from_i32(port as i32)),
            ]);
            Ok(result)
        }
        Err(e) => {
            let err = vm.alloc_string(e.to_string());
            let result = vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("error".into(), err),
            ]);
            Ok(result)
        }
    }
}

fn net_accept(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);

    let map = listeners().lock().unwrap();
    let Some(listener) = map.get(&handle) else {
        let err = vm.alloc_string("invalid listener handle".into());
        return Ok(vm.create_object_from_pairs(&[
            ("ok".into(), JsValue::from_bool(false)),
            ("error".into(), err),
        ]));
    };

    match listener.accept() {
        Ok((stream, addr)) => {
            let conn_handle = next_handle();
            drop(map);
            streams().lock().unwrap().insert(conn_handle, stream);
            let addr_str = vm.alloc_string(addr.to_string());
            let result = vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(true)),
                ("handle".into(), JsValue::from_i32(conn_handle as i32)),
                ("remoteAddr".into(), addr_str),
            ]);
            Ok(result)
        }
        Err(e) => {
            let err = vm.alloc_string(e.to_string());
            Ok(vm.create_object_from_pairs(&[
                ("ok".into(), JsValue::from_bool(false)),
                ("error".into(), err),
            ]))
        }
    }
}

fn net_close_listen(_vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let removed = listeners().lock().unwrap().remove(&handle).is_some();
    Ok(JsValue::from_bool(removed))
}

fn net_local_addr(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let map = streams().lock().unwrap();
    let Some(stream) = map.get(&handle) else {
        return Ok(JsValue::null());
    };
    match stream.local_addr() {
        Ok(addr) => Ok(vm.alloc_string(addr.to_string())),
        Err(_) => Ok(JsValue::null()),
    }
}

fn net_remote_addr(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let map = streams().lock().unwrap();
    let Some(stream) = map.get(&handle) else {
        return Ok(JsValue::null());
    };
    match stream.peer_addr() {
        Ok(addr) => Ok(vm.alloc_string(addr.to_string())),
        Err(_) => Ok(JsValue::null()),
    }
}

fn net_set_read_timeout(_vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let ms = args.get(1).map(|v| v.to_number() as u64).unwrap_or(0);

    let map = streams().lock().unwrap();
    let Some(stream) = map.get(&handle) else {
        return Ok(JsValue::from_bool(false));
    };
    let timeout = if ms > 0 {
        Some(Duration::from_millis(ms))
    } else {
        None
    };
    let ok = stream.set_read_timeout(timeout).is_ok();
    Ok(JsValue::from_bool(ok))
}

fn net_set_write_timeout(_vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let ms = args.get(1).map(|v| v.to_number() as u64).unwrap_or(0);

    let map = streams().lock().unwrap();
    let Some(stream) = map.get(&handle) else {
        return Ok(JsValue::from_bool(false));
    };
    let timeout = if ms > 0 {
        Some(Duration::from_millis(ms))
    } else {
        None
    };
    let ok = stream.set_write_timeout(timeout).is_ok();
    Ok(JsValue::from_bool(ok))
}
