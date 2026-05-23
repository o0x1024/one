pub mod dns;
pub mod fetch;
pub mod tcp;
pub mod tls;
pub mod ws;

use one_vm::Vm;

pub fn install_net(vm: &mut Vm) {
    fetch::install_fetch(vm);
    tcp::install_tcp(vm);
    ws::install_websocket(vm);
    tls::install_tls(vm);
    dns::install_dns(vm);
}
