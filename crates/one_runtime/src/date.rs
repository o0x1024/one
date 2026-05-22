use std::time::{SystemTime, UNIX_EPOCH};

use one_core::JsValue;
use one_vm::Vm;
use one_vm::object::JsObject;

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m - 1, d)
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let mut y = i64::from(year);
    let m = i64::from(month);
    let d = i64::from(day);
    y -= i64::from(u32::from(m <= 2));
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn date_parts(ms: f64) -> (i32, u32, u32, u32, u32, u32, u32) {
    let secs = (ms / 1000.0).trunc() as i64;
    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400) as u32;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    let millis = (ms.rem_euclid(1000.0)) as u32;
    let (year, month, day) = civil_from_days(days);
    (year, month, day, hours, minutes, seconds, millis)
}

fn utc_ms_from_components(
    year: i32,
    month: u32,
    day: u32,
    hours: u32,
    minutes: u32,
    seconds: u32,
    millis: u32,
) -> f64 {
    let days = days_from_civil(year, month + 1, day);
    let secs = days * 86_400
        + i64::from(hours) * 3600
        + i64::from(minutes) * 60
        + i64::from(seconds);
    secs as f64 * 1000.0 + f64::from(millis)
}

fn get_date_ms(vm: &Vm) -> Option<f64> {
    let this = vm.get_global("this");
    vm.date_ms(this)
}

fn pad2(n: u32) -> String {
    format!("{n:02}")
}

fn pad3(n: u32) -> String {
    format!("{n:03}")
}

fn month_name(month: u32) -> &'static str {
    match month {
        0 => "Jan",
        1 => "Feb",
        2 => "Mar",
        3 => "Apr",
        4 => "May",
        5 => "Jun",
        6 => "Jul",
        7 => "Aug",
        8 => "Sep",
        9 => "Oct",
        10 => "Nov",
        _ => "Dec",
    }
}

fn date_now(_vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    Ok(JsValue::from_f64(now_ms()))
}

fn date_constructor(vm: &mut Vm, args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = if args.is_empty() {
        now_ms()
    } else if args.len() == 1 {
        args[0].to_number()
    } else {
        let year = args[0].to_number() as i32;
        let month = args.get(1).map(|v| v.to_number() as u32).unwrap_or(0);
        let day = args.get(2).map(|v| v.to_number() as u32).unwrap_or(1);
        let hours = args.get(3).map(|v| v.to_number() as u32).unwrap_or(0);
        let minutes = args.get(4).map(|v| v.to_number() as u32).unwrap_or(0);
        let seconds = args.get(5).map(|v| v.to_number() as u32).unwrap_or(0);
        let millis = args.get(6).map(|v| v.to_number() as u32).unwrap_or(0);
        utc_ms_from_components(year, month, day, hours, minutes, seconds, millis)
    };
    Ok(vm.new_date(ms))
}

fn date_get_time(_vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    Ok(JsValue::from_f64(get_date_ms(_vm).unwrap_or(f64::NAN)))
}

fn date_get_full_year(_vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(_vm).unwrap_or(f64::NAN);
    let (year, _, _, _, _, _, _) = date_parts(ms);
    Ok(JsValue::from_f64(f64::from(year)))
}

fn date_get_month(_vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(_vm).unwrap_or(f64::NAN);
    let (_, month, _, _, _, _, _) = date_parts(ms);
    Ok(JsValue::from_f64(f64::from(month)))
}

fn date_get_date(_vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(_vm).unwrap_or(f64::NAN);
    let (_, _, day, _, _, _, _) = date_parts(ms);
    Ok(JsValue::from_f64(f64::from(day)))
}

fn date_get_hours(_vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(_vm).unwrap_or(f64::NAN);
    let (_, _, _, hours, _, _, _) = date_parts(ms);
    Ok(JsValue::from_f64(f64::from(hours)))
}

fn date_get_minutes(_vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(_vm).unwrap_or(f64::NAN);
    let (_, _, _, _, minutes, _, _) = date_parts(ms);
    Ok(JsValue::from_f64(f64::from(minutes)))
}

fn date_get_seconds(_vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(_vm).unwrap_or(f64::NAN);
    let (_, _, _, _, _, seconds, _) = date_parts(ms);
    Ok(JsValue::from_f64(f64::from(seconds)))
}

fn date_get_milliseconds(_vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(_vm).unwrap_or(f64::NAN);
    let (_, _, _, _, _, _, millis) = date_parts(ms);
    Ok(JsValue::from_f64(f64::from(millis)))
}

fn date_to_iso_string(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(vm).unwrap_or(f64::NAN);
    let (year, month, day, hours, minutes, seconds, millis) = date_parts(ms);
    let iso = format!(
        "{year:04}-{}-{}T{}:{}:{}.{}Z",
        pad2(month + 1),
        pad2(day),
        pad2(hours),
        pad2(minutes),
        pad2(seconds),
        pad3(millis)
    );
    Ok(vm.alloc_string(iso))
}

fn date_to_string(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(vm).unwrap_or(f64::NAN);
    let (year, month, day, hours, minutes, seconds, _) = date_parts(ms);
    let s = format!(
        "{} {} {} {} {}:{}:{} GMT+0000 (Coordinated Universal Time)",
        month_name(month),
        pad2(day),
        year,
        if hours >= 12 { "PM" } else { "AM" },
        pad2(hours % 12),
        pad2(minutes),
        pad2(seconds)
    );
    Ok(vm.alloc_string(s))
}

fn date_to_locale_date_string(vm: &mut Vm, _args: &[JsValue]) -> one_core::OneResult<JsValue> {
    let ms = get_date_ms(vm).unwrap_or(f64::NAN);
    let (year, month, day, _, _, _, _) = date_parts(ms);
    let s = format!("{}/{}/{}", month + 1, day, year);
    Ok(vm.alloc_string(s))
}

fn install_proto_method<F>(vm: &mut Vm, proto_val: JsValue, name: &str, func: F)
where
    F: Fn(&mut Vm, &[JsValue]) -> one_core::OneResult<JsValue> + 'static,
{
    let host_name = format!("Date.prototype.{name}");
    let sentinel = vm.register_host_fn_returning_sentinel(&host_name, func);
    if let Some(obj) = vm.get_object_mut(proto_val) {
        obj.set_property(name.to_string(), sentinel);
    }
}

pub fn install_date(vm: &mut Vm) {
    vm.register_host_fn("Date", date_constructor);
    vm.register_host_fn("Date.now", date_now);

    let proto = JsObject::new();
    let proto_val = vm.alloc_object(proto);

    install_proto_method(vm, proto_val, "getTime", date_get_time);
    install_proto_method(vm, proto_val, "getFullYear", date_get_full_year);
    install_proto_method(vm, proto_val, "getMonth", date_get_month);
    install_proto_method(vm, proto_val, "getDate", date_get_date);
    install_proto_method(vm, proto_val, "getHours", date_get_hours);
    install_proto_method(vm, proto_val, "getMinutes", date_get_minutes);
    install_proto_method(vm, proto_val, "getSeconds", date_get_seconds);
    install_proto_method(vm, proto_val, "getMilliseconds", date_get_milliseconds);
    install_proto_method(vm, proto_val, "toISOString", date_to_iso_string);
    install_proto_method(vm, proto_val, "toString", date_to_string);
    install_proto_method(vm, proto_val, "toLocaleDateString", date_to_locale_date_string);

    vm.set_date_prototype(proto_val);

    let date_global = vm.get_global("Date");
    if let Some(obj) = vm.get_object_mut(date_global) {
        obj.set_property("prototype".to_string(), proto_val);
    }
}
