use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use one_core::{JsValue, OneResult};
use one_vm::Vm;
use tungstenite::{connect, Message, WebSocket};
use tungstenite::stream::MaybeTlsStream;

type WsStream = WebSocket<MaybeTlsStream<std::net::TcpStream>>;
type WsMap = Arc<Mutex<HashMap<u32, WsStream>>>;

static WS_NEXT_HANDLE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

fn ws_map() -> &'static WsMap {
    use std::sync::OnceLock;
    static INSTANCE: OnceLock<WsMap> = OnceLock::new();
    INSTANCE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

fn ws_next_handle() -> u32 {
    WS_NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub fn install_websocket(vm: &mut Vm) {
    vm.register_host_fn("ws.connect", ws_connect);
    vm.register_host_fn("ws.send", ws_send);
    vm.register_host_fn("ws.receive", ws_receive);
    vm.register_host_fn("ws.close", ws_close);
    vm.register_host_fn("ws.ping", ws_ping);
    vm.register_host_fn("ws.state", ws_state);
}

fn ws_connect(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let url = args
        .first()
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();

    match connect(&url) {
        Ok((socket, _response)) => {
            let handle = ws_next_handle();
            ws_map().lock().unwrap().insert(handle, socket);
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

fn ws_send(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let data = args
        .get(1)
        .map(|v| vm.value_to_string(*v))
        .unwrap_or_default();

    let mut map = ws_map().lock().unwrap();
    let Some(ws) = map.get_mut(&handle) else {
        return Ok(JsValue::from_bool(false));
    };

    match ws.send(Message::Text(data.into())) {
        Ok(_) => Ok(JsValue::from_bool(true)),
        Err(_) => Ok(JsValue::from_bool(false)),
    }
}

fn ws_receive(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);

    let mut map = ws_map().lock().unwrap();
    let Some(ws) = map.get_mut(&handle) else {
        return Ok(JsValue::null());
    };

    match ws.read() {
        Ok(msg) => {
            let (msg_type, data): (&str, String) = match msg {
                Message::Text(t) => ("text", t.to_string()),
                Message::Binary(b) => ("binary", format!("[{} bytes]", b.len())),
                Message::Ping(_) => ("ping", String::new()),
                Message::Pong(_) => ("pong", String::new()),
                Message::Close(_) => ("close", String::new()),
                Message::Frame(_) => ("frame", String::new()),
            };
            let type_val = vm.alloc_string(msg_type.to_string());
            let data_val = vm.alloc_string(data);
            let result = vm.create_object_from_pairs(&[
                ("type".into(), type_val),
                ("data".into(), data_val),
            ]);
            Ok(result)
        }
        Err(_) => Ok(JsValue::null()),
    }
}

fn ws_close(_vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let mut map = ws_map().lock().unwrap();
    if let Some(mut ws) = map.remove(&handle) {
        let _ = ws.close(None);
        Ok(JsValue::from_bool(true))
    } else {
        Ok(JsValue::from_bool(false))
    }
}

fn ws_ping(_vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let mut map = ws_map().lock().unwrap();
    let Some(ws) = map.get_mut(&handle) else {
        return Ok(JsValue::from_bool(false));
    };
    match ws.send(Message::Ping(Vec::new().into())) {
        Ok(_) => Ok(JsValue::from_bool(true)),
        Err(_) => Ok(JsValue::from_bool(false)),
    }
}

fn ws_state(vm: &mut Vm, args: &[JsValue]) -> OneResult<JsValue> {
    let handle = args.first().map(|v| v.to_number() as u32).unwrap_or(0);
    let map = ws_map().lock().unwrap();
    if map.contains_key(&handle) {
        Ok(vm.alloc_string("open".into()))
    } else {
        Ok(vm.alloc_string("closed".into()))
    }
}
