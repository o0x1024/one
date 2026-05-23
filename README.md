# One

A 100% Rust JavaScript engine designed for embedding. Full ES2024+ compliance, built-in TypeScript support, and a 2.9 MB binary with zero C/C++ dependencies.

[中文文档](README_CN.md)

## Highlights

- **Pure Rust** — zero C/C++ dependencies (no V8, JSC, or QuickJS)
- **ES2024+ compliant** — closures, iterators, async/await, Promises, modules
- **Built-in TypeScript** — type stripping at parse time with zero overhead
- **Generational GC** — nursery bump-allocation + incremental mark-compact old generation
- **Register-based VM** — 32-bit fixed-width bytecode with NaN-boxing (`u64` values)
- **Embeddable** — `Engine::new()` + `engine.eval("...")` — two lines to run JS from Rust
- **Extension system** — pluggable host functions, state, and bootstrap JS via `Extension` trait
- **Module system** — composable resolver chain: local files, URLs, in-memory, or custom
- **Built-in networking** — fetch, TCP, WebSocket, TLS, DNS (opt-in via `net` feature flag)
- **Sandboxed** — fuel-based execution limits, call depth control, bare engine mode

## Architecture

```
Source (JS/TS)
  │
  ▼
┌────────────────────────┐
│  Parser (one_parser)   │  Lexer + Pratt parser + TS type stripping
└──────────┬─────────────┘
           │ AST
           ▼
┌────────────────────────┐
│  Compiler              │  AST → register bytecode
│  (one_compiler)        │  Free variable analysis + upvalue capture
└──────────┬─────────────┘
           │ CodeBlock (bytecode)
           ▼
┌────────────────────────┐
│  Register VM (one_vm)  │  NaN-boxed values, inline caches
│  · Closures / Upvalues │  Iterator protocol
│  · async/await         │  Promise microtask queue
└──────────┬─────────────┘
           │
    ┌──────┴──────┐
    ▼              ▼
┌────────┐  ┌────────────────┐
│ Gen GC │  │ Runtime        │  one_gc / one_runtime
│(one_gc)│  │ Builtins + API │
│ Young  │  │ Event loop     │
│ Old    │  │ Net (optional) │
└────────┘  └────────────────┘
```

## Crate Structure

| Crate | Description |
|-------|-------------|
| `one_core` | Shared types — `JsValue` (NaN-boxing), `OneError`, string interner |
| `one_parser` | Lexer + AST + Pratt parser + TS type stripping |
| `one_compiler` | AST → register bytecode with constant folding and upvalue analysis |
| `one_gc` | Generational GC — nursery scavenger + mark-compact old generation |
| `one_vm` | Register-based bytecode VM with closures, iterators, async/await |
| `one_runtime` | Builtins (Array, Map, Promise, RegExp, ...) + optional `net` module |
| `one_engine` | Embedding API — `Engine`, `EngineBuilder`, `Extension`, `ModuleResolver` |
| `one_bridge` | Sentinel AI adapter layer |
| `one_cli` | REPL + CLI entry point |

## Quick Start

### As an Embedded Engine

```rust
use one_engine::Engine;

let mut engine = Engine::new();
let result = engine.eval("1 + 1").unwrap();
assert_eq!(result.to_number(), 2.0);
```

### With EngineBuilder

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

### Extension System

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

### Module System

One uses a composable resolver chain (Static → File → URL):

```javascript
// main.js
import { double } from "./math.js";                    // local file
import greet from "https://example.com/greet.mjs";      // URL (cached to disk)

console.log(double(21));  // 42
console.log(greet("One"));
```

```bash
one main.js
```

Custom resolvers can be plugged in by implementing the `ModuleResolver` trait:

```rust
use one_engine::{EngineBuilder, FileModuleResolver, ModuleResolverChain,
                 StaticModuleResolver, UrlModuleResolver};

let chain = ModuleResolverChain::new()
    .push(StaticModuleResolver::new())
    .push(FileModuleResolver::new("./src"))
    .push(UrlModuleResolver::with_default_cache());

let engine = EngineBuilder::new().module_resolver(chain).build();
```

### Built-in Networking (opt-in)

Enabled by default in `one_cli`. Disable with `--no-default-features`.

```javascript
// HTTP
let resp = fetch("https://httpbin.org/get");
console.log(resp["status"]);  // 200

// DNS
let ip = dns.lookup("example.com");

// TLS certificate inspection
let cert = tls.getCertificate("github.com");
console.log(cert["subject"]);  // CN=github.com

// TCP
let conn = net.connect("93.184.216.34:80", 3000);
net.write(conn["handle"], "GET / HTTP/1.1\r\nHost: example.com\r\n\r\n");
let data = net.read(conn["handle"], 4096);
net.close(conn["handle"]);
```

Binary size comparison:

| Build | Size |
|-------|------|
| Without `net` feature | **2.9 MB** |
| With `net` feature (default) | **6.3 MB** |

## CLI

```bash
# Run a script
one script.js

# Inline execution
one -e 'console.log(1 + 2)'

# Interactive REPL
one
```

## Building

```bash
# Build the workspace
cargo build

# Build release CLI (with networking)
cargo build --release -p one_cli

# Build minimal (no networking)
cargo build --release -p one_cli --no-default-features

# Run tests
cargo test
```

### Requirements

- Rust 1.85+ (edition 2024)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.
