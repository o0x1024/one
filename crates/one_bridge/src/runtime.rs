use one_core::OneResult;
use one_engine::{Engine, EngineBuilder, RuntimeLimits};
use serde_json::Value as JsonValue;

use crate::extensions::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub title: String,
    pub description: String,
    pub severity: String,
    pub category: String,
    pub evidence: Option<String>,
    pub url: Option<String>,
}

#[derive(Default)]
pub struct PluginState {
    pub plugin_id: String,
    pub plugin_name: String,
    pub findings: Vec<Finding>,
    pub logs: Vec<(String, String)>,
    pub last_result: Option<JsonValue>,
}

pub struct PluginRuntime {
    engine: Engine<PluginState>,
}

impl PluginRuntime {
    pub fn new(plugin_id: &str, plugin_name: &str) -> Self {
        let state = PluginState {
            plugin_id: plugin_id.to_string(),
            plugin_name: plugin_name.to_string(),
            ..Default::default()
        };

        let engine = EngineBuilder::<PluginState>::new()
            .limits(RuntimeLimits {
                max_operations: Some(10_000_000),
                max_call_depth: Some(64),
                ..Default::default()
            })
            .extension(SentinelCoreExtension::new())
            .extension(FetchExtension::new())
            .extension(FsExtension::new())
            .extension(NetworkExtension::new())
            .extension(DictionaryExtension::new())
            .extension(TlsExtension::new())
            .extension(MonitorExtension::new())
            .extension(AstExtension::new())
            .build_with_store(state);

        PluginRuntime { engine }
    }

    pub fn load_plugin(&mut self, source: &str) -> OneResult<()> {
        self.engine.eval(source)?;
        self.sync_from_globals();
        Ok(())
    }

    pub fn load_plugin_module(&mut self, source: &str, path: &str) -> OneResult<()> {
        self.engine.eval_module(source, path)?;
        self.sync_from_globals();
        Ok(())
    }

    pub fn call_function(&mut self, name: &str, args: &[JsonValue]) -> OneResult<JsonValue> {
        self.reset_runtime_state();

        for (i, arg) in args.iter().enumerate() {
            self.engine.set_json_global(&format!("__arg{i}__"), arg);
        }
        let arg_refs: Vec<String> = (0..args.len()).map(|i| format!("__arg{i}__")).collect();
        let call = format!("return {name}({});", arg_refs.join(", "));

        let result = self.engine.eval(&call)?;
        self.engine.run_event_loop()?;
        self.sync_from_globals();

        if self.engine.store().last_result.is_some() {
            return Ok(self.engine.store().last_result.clone().unwrap());
        }

        Ok(one_engine::js_to_json(self.engine.vm(), result))
    }

    pub fn scan_transaction(&mut self, transaction: &JsonValue) -> OneResult<Vec<Finding>> {
        self.reset_runtime_state();
        self.engine.set_json_global("__transaction__", transaction);

        let _ = self.engine.eval("scan_transaction(__transaction__);")?;
        self.engine.run_event_loop()?;
        self.sync_from_globals();

        Ok(std::mem::take(&mut self.engine.store_mut().findings))
    }

    pub fn findings(&self) -> &[Finding] {
        &self.engine.store().findings
    }

    pub fn logs(&self) -> &[(String, String)] {
        &self.engine.store().logs
    }

    pub fn last_result(&self) -> Option<&JsonValue> {
        self.engine.store().last_result.as_ref()
    }

    pub fn engine(&self) -> &Engine<PluginState> {
        &self.engine
    }

    pub fn engine_mut(&mut self) -> &mut Engine<PluginState> {
        &mut self.engine
    }

    fn reset_runtime_state(&mut self) {
        let vm = self.engine.vm_mut();
        crate::sentinel_api::init_storage(vm);
        self.engine.store_mut().findings.clear();
        self.engine.store_mut().logs.clear();
        self.engine.store_mut().last_result = None;
    }

    fn sync_from_globals(&mut self) {
        let findings_json = self.engine.get_json_global("__sentinel_findings__");
        let logs_json = self.engine.get_json_global("__sentinel_logs__");
        let last_result = self.engine.get_json_global("__sentinel_last_result__");

        let store = self.engine.store_mut();
        store.findings = parse_findings(&findings_json);
        store.logs = parse_logs(&logs_json);
        if !last_result.is_null() {
            store.last_result = Some(last_result);
        }
    }
}

fn parse_findings(value: &JsonValue) -> Vec<Finding> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };

    items.iter().map(json_to_finding).collect()
}

fn json_to_finding(value: &JsonValue) -> Finding {
    Finding {
        title: json_string_field(value, "title"),
        description: json_string_field(value, "description"),
        severity: value
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("medium")
            .to_string(),
        category: value
            .get("category")
            .and_then(|v| v.as_str())
            .or_else(|| value.get("vuln_type").and_then(|v| v.as_str()))
            .unwrap_or("unknown")
            .to_string(),
        evidence: value
            .get("evidence")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        url: value
            .get("url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    }
}

fn parse_logs(value: &JsonValue) -> Vec<(String, String)> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };

    items
        .iter()
        .filter_map(|entry| {
            let level = entry.get("level")?.as_str()?.to_string();
            let message = entry.get("message")?.as_str()?.to_string();
            Some((level, message))
        })
        .collect()
}

fn json_string_field(value: &JsonValue, field: &str) -> String {
    value
        .get(field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_runtime_basic() {
        let mut rt = PluginRuntime::new("test-1", "Test Plugin");
        rt.load_plugin(
            r#"
            function scan_transaction(tx) {
                if (tx.url.includes("admin")) {
                    Sentinel.emitFinding({
                        title: "Admin Access",
                        severity: "high",
                        description: "Direct admin access detected"
                    });
                }
            }
        "#,
        )
        .unwrap();

        let tx = serde_json::json!({
            "url": "https://example.com/admin",
            "method": "GET"
        });

        let findings = rt.scan_transaction(&tx).unwrap();
        assert!(!findings.is_empty());
        assert_eq!(findings[0].title, "Admin Access");
        assert_eq!(findings[0].severity, "high");
    }

    #[test]
    fn plugin_runtime_logging() {
        let mut rt = PluginRuntime::new("test-2", "Log Plugin");
        rt.load_plugin(
            r#"
            Sentinel.log("info", "plugin started");
            Sentinel.log("debug", "processing data");
        "#,
        )
        .unwrap();

        let logs = rt.logs();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0], ("info".to_string(), "plugin started".to_string()));
        assert_eq!(logs[1], ("debug".to_string(), "processing data".to_string()));
    }

    #[test]
    fn plugin_runtime_json_args() {
        let mut rt = PluginRuntime::new("test-3", "JSON Plugin");
        rt.load_plugin(
            r#"
            function analyze(input) {
                return { success: true, result: input.value * 2 };
            }
        "#,
        )
        .unwrap();

        let result = rt
            .call_function("analyze", &[serde_json::json!({"value": 21})])
            .unwrap();
        assert_eq!(result["success"], true);
        assert_eq!(result["result"].as_f64().unwrap(), 42.0);
    }

    #[test]
    fn plugin_runtime_console_log() {
        let mut rt = PluginRuntime::new("test-4", "Console Plugin");
        rt.load_plugin(
            r#"
            console.log("Hello from plugin");
            let x = 1 + 2;
        "#,
        )
        .unwrap();
    }

    #[test]
    fn plugin_runtime_module() {
        let mut rt = PluginRuntime::new("test-5", "Module Plugin");
        rt.engine_mut().register_module(
            "utils",
            r#"
            export function double(x) { return x * 2; }
        "#,
        );
        rt.load_plugin_module(
            r#"
            import { double } from "utils";
            function analyze(input) { return double(input); }
        "#,
            "plugin.js",
        )
        .unwrap();

        let result = rt.call_function("analyze", &[serde_json::json!(21)]).unwrap();
        assert_eq!(result.as_f64().unwrap(), 42.0);
    }

    #[test]
    fn plugin_runtime_typescript() {
        let mut rt = PluginRuntime::new("test-6", "TS Plugin");
        rt.load_plugin(
            r#"
            function analyze(input: any): number {
                let x: number = input.value;
                return x * 2;
            }
        "#,
        )
        .unwrap();

        let result = rt
            .call_function("analyze", &[serde_json::json!({"value": 10})])
            .unwrap();
        assert_eq!(result.as_f64().unwrap(), 20.0);
    }
}
