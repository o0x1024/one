# One

100% Rust 实现的通用 JavaScript 引擎，专为嵌入式场景设计。完整 ES2024+ 规范支持，内置 TypeScript，2.9 MB 二进制，零 C/C++ 依赖。

[English](README.md)

## 特性

- **纯 Rust** — 零 C/C++ 依赖（不依赖 V8/JSC/QuickJS）
- **ES2024+** — 闭包、迭代器、async/await、Promise、模块系统
- **内置 TypeScript** — 解析阶段类型擦除，零额外开销
- **分代 GC** — 新生代 bump-alloc + 增量标记-压缩老年代
- **寄存器式 VM** — 32 位定长字节码 + NaN-boxing（`u64` 值表示）
- **极简嵌入** — `Engine::new()` + `engine.eval("...")` 两行代码运行 JS
- **Extension 扩展** — 可插拔的宿主函数、状态和引导 JS
- **模块系统** — 可组合的 resolver 链：本地文件、URL、内存、自定义
- **内建网络栈** — fetch、TCP、WebSocket、TLS、DNS（通过 `net` feature 按需启用）
- **安全沙箱** — 燃料限制、调用深度控制、裸引擎模式

## 架构

```
源码 (JS/TS)
  │
  ▼
┌──────────────────────────┐
│  解析器 (one_parser)      │  词法分析 + Pratt 解析器 + TS 类型擦除
└───────────┬──────────────┘
            │ AST
            ▼
┌──────────────────────────┐
│  编译器 (one_compiler)    │  AST → 寄存器字节码
│                          │  自由变量分析 + upvalue 捕获
└───────────┬──────────────┘
            │ CodeBlock（字节码）
            ▼
┌──────────────────────────┐
│  寄存器虚拟机 (one_vm)    │  NaN-boxed 值，内联缓存
│  · 闭包 / Upvalue        │  迭代器协议
│  · async/await           │  Promise 微任务队列
└───────────┬──────────────┘
            │
     ┌──────┴──────┐
     ▼              ▼
┌────────┐   ┌───────────────┐
│ 分代 GC │   │ 运行时         │  one_gc / one_runtime
│(one_gc)│   │ 内置对象 + API │
│ 新生代  │   │ 事件循环       │
│ 老年代  │   │ 网络栈（可选） │
└────────┘   └───────────────┘
```

## Crate 结构

| Crate | 说明 |
|-------|------|
| `one_core` | 核心共享类型 — `JsValue`（NaN-boxing）、`OneError`、字符串实习池 |
| `one_parser` | 词法分析 + AST + Pratt 解析器 + TS 类型擦除 |
| `one_compiler` | AST → 寄存器字节码编译器，含常量折叠和 upvalue 分析 |
| `one_gc` | 分代 GC — 新生代 Scavenger + 标记-压缩老年代 |
| `one_vm` | 寄存器式字节码虚拟机，含闭包、迭代器、async/await |
| `one_runtime` | 内置对象（Array、Map、Promise、RegExp…）+ 可选 `net` 模块 |
| `one_engine` | 嵌入 API — `Engine`、`EngineBuilder`、`Extension`、`ModuleResolver` |
| `one_bridge` | Sentinel AI 适配层 |
| `one_cli` | REPL + CLI 入口 |

## 快速上手

### 作为嵌入引擎

```rust
use one_engine::Engine;

let mut engine = Engine::new();
let result = engine.eval("1 + 1").unwrap();
assert_eq!(result.to_number(), 2.0);
```

### 使用 EngineBuilder

```rust
use one_engine::{EngineBuilder, Preset, RuntimeLimits};

let mut engine = EngineBuilder::new()
    .preset(Preset::Sandbox)
    .limits(RuntimeLimits {
        max_operations: Some(100_000),
        max_call_depth: Some(64),
        ..Default::default()
    })
    .with_module("utils", "export function double(x) { return x * 2; }")
    .build();

engine.eval("console.log('Hello from One!')").unwrap();
```

### Extension 扩展系统

```rust
use one_engine::{EngineBuilder, Extension, HostFnDescriptor, host_fn};
use one_core::JsValue;

struct MyExtension;
impl Extension for MyExtension {
    fn name(&self) -> &str { "my_ext" }
    fn host_functions(&self) -> Vec<HostFnDescriptor> {
        vec![host_fn("myAdd", |_vm, args| {
            let a = args.get(0).map(|v| v.to_number()).unwrap_or(0.0);
            let b = args.get(1).map(|v| v.to_number()).unwrap_or(0.0);
            Ok(JsValue::from_f64(a + b))
        })]
    }
}

let mut engine = EngineBuilder::new().extension(MyExtension).build();
let result = engine.eval("myAdd(40, 2)").unwrap();
assert_eq!(result.to_number(), 42.0);
```

### 模块系统

One 使用可组合的 resolver 链（Static → File → URL）：

```javascript
// main.js
import { double } from "./math.js";                    // 本地文件
import greet from "https://example.com/greet.mjs";      // URL（自动缓存到磁盘）

console.log(double(21));  // 42
console.log(greet("One"));
```

```bash
one main.js
```

自定义 resolver — 实现 `ModuleResolver` trait 即可：

```rust
use one_engine::{EngineBuilder, FileModuleResolver, ModuleResolverChain,
                 StaticModuleResolver, UrlModuleResolver};

let chain = ModuleResolverChain::new()
    .push(StaticModuleResolver::new())
    .push(FileModuleResolver::new("./src"))
    .push(UrlModuleResolver::with_default_cache());

let engine = EngineBuilder::new().module_resolver(chain).build();
```

### 内建网络（按需启用）

`one_cli` 默认启用。通过 `--no-default-features` 关闭。

```javascript
// HTTP
let resp = fetch("https://httpbin.org/get");
console.log(resp["status"]);  // 200

// DNS
let ip = dns.lookup("example.com");

// TLS 证书检查
let cert = tls.getCertificate("github.com");
console.log(cert["subject"]);  // CN=github.com

// TCP
let conn = net.connect("93.184.216.34:80", 3000);
net.write(conn["handle"], "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n");
let data = net.read(conn["handle"], 4096);
net.close(conn["handle"]);
```

二进制体积对比：

| 构建方式 | 大小 |
|---------|------|
| 不含 `net` feature | **2.9 MB** |
| 含 `net` feature（默认） | **6.3 MB** |

## CLI 使用

```bash
# 执行脚本
one script.js

# 内联执行
one -e 'console.log(1 + 2)'

# 交互式 REPL
one
```

## 构建

```bash
# 构建工作区
cargo build

# 构建 release CLI（含网络）
cargo build --release -p one_cli

# 构建精简版（不含网络）
cargo build --release -p one_cli --no-default-features

# 运行测试
cargo test
```

需要 Rust 1.85+（edition 2024）。

## 许可证

本项目双许可，任选其一：

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) 或 <http://opensource.org/licenses/MIT>)
