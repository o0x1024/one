# One

A 100% Rust JavaScript runtime targeting full ES2024+ compliance with built-in TypeScript support. Designed as an embeddable engine for Rust projects and as a standalone runtime.

[中文文档](README_CN.md)

## Highlights

- **Pure Rust** — zero C/C++ dependencies (no V8, JSC, or QuickJS)
- **ES2024+ compliant** — validated against Test262
- **Built-in TypeScript** — type stripping at parse time with zero overhead
- **Generational GC** — nursery bump-allocation + incremental mark-compact old generation
- **Register-based VM** — 32-bit fixed-width bytecode with polymorphic inline caches
- **NaN-boxing** — all JS values fit in a single `u64`
- **Embeddable** — three lines of code to create an engine, run JS, and get results
- **Sandboxed** — bare engine mode with fuel-based execution control
- **wasm32 ready** — compiles to WebAssembly

## Architecture

```
Source (JS/TS/JSX/TSX)
  │
  ▼
┌────────────────────────┐
│  Parser (one_parser)   │  Lexer + AST + TS type stripping
│  · Lazy parsing        │  Arena-allocated AST nodes
│  · Arena bump alloc    │
└──────────┬─────────────┘
           │ AST
           ▼
┌────────────────────────┐
│  Compiler              │  one_compiler
│  (one_compiler)        │  Constant folding + peephole optimizer
└──────────┬─────────────┘
           │ Bytecode (CodeBlock)
           ▼
┌────────────────────────┐
│  Register VM (one_vm)  │  Polymorphic inline caches
│  · Shape / Hidden Class│  Type-specialized fast paths
│  · Inline properties   │
└──────────┬─────────────┘
           │
    ┌──────┴──────┐
    ▼              ▼
┌────────┐  ┌────────────────┐
│ Gen GC │  │ Runtime        │  one_gc / one_runtime
│(one_gc)│  │ Builtins + API │
│ Young  │  │ Event loop     │
│ Old    │  │ Module system  │
└────────┘  └────────────────┘
```

## Crate Structure

| Crate | Description |
|-------|-------------|
| `one_core` | Shared types — `JsValue` (NaN-boxing), `GcPtr`, `OneError`, `InternId`, string interner |
| `one_parser` | Lexer + AST + Pratt parser + TS type stripping + lazy parsing |
| `one_compiler` | AST → register bytecode compiler with constant folding |
| `one_gc` | Generational GC — nursery scavenger + mark-compact + `derive(Trace)` |
| `one_vm` | Register-based bytecode VM with Shape system and inline caches |
| `one_runtime` | Builtin objects + event loop + module system + host APIs |
| `one_engine` | Embedding API — `OneEngine<T>`, Builder, Presets, `FromJs`/`IntoJs` |
| `one_bridge` | Sentinel AI adapter layer |
| `one_cli` | REPL + CLI entry point |

## Quick Start

### As an Embedded Engine

```rust
use one_engine::OneEngine;

let mut engine = OneEngine::<()>::default();
let result = engine.eval("1 + 1")?;
println!("{}", result); // 2
```

### With Host Data

```rust
use one_engine::{OneEngine, Preset};

struct MyApp {
    request_count: u64,
}

let mut engine = OneEngine::<MyApp>::builder()
    .host_data(MyApp { request_count: 0 })
    .preset(Preset::Standard)
    .enable_typescript(true)
    .fuel(100_000)
    .build();

engine.eval("console.log('Hello from One!')")?;
```

### Sandbox Mode

```rust
let mut engine = OneEngine::<()>::builder()
    .bare(true)
    .fuel(10_000)
    .build();

engine.install::<ObjectBuiltin>();
engine.install::<ArrayBuiltin>();
engine.install::<MathBuiltin>();
// No I/O available — fully sandboxed
```

## Building

```bash
# Build the workspace
cargo build

# Run the CLI / REPL
cargo run -p one_cli

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
