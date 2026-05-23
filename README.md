# One

A 100% Rust JavaScript runtime targeting full ES2024+ compliance with built-in TypeScript support. Designed as an embeddable engine for Rust projects and as a standalone runtime.

[中文文档](#中文文档)

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

---

## 中文文档

# One

100% Rust 实现的通用 JavaScript 运行时，目标完整实现 ES2024+ 规范，内置 TypeScript 支持，可独立使用也可嵌入 Rust 项目。

## 设计目标

**四维极致原则：最高性能、最少代码、最多兼容、最简接入**

- 100% Rust，零 C/C++ 依赖
- 完整 ES2024+ 规范合规（通过 Test262 验证）
- 内置 TypeScript 类型擦除（解析阶段完成，零额外开销）
- 分代垃圾回收器（Generational GC），支持 WeakRef / FinalizationRegistry
- 寄存器式字节码 VM + 多态内联缓存
- NaN-boxing 值表示，所有 JS 值压缩到一个 `u64`
- 惰性解析 + 字节码缓存 + 堆快照：极致启动性能
- Cargo feature flags 按需裁剪，支持 wasm32 编译目标

## 技术亮点

| 特性 | 方案 |
|------|------|
| 值表示 | NaN-boxing (`u64`)，极小内存开销 |
| 字符串 | 双编码 Latin1 + UTF-16，O(1) 索引 |
| 对象模型 | Shape (Hidden Class) + 多态 IC + 内联属性 |
| 字节码 | 寄存器式 32 位定长 + 类型特化指令 |
| GC | 分代回收：16MB Nursery bump alloc + 增量标记-压缩 |
| 嵌入 API | `Store<T>` 宿主数据绑定 + Builder + Fuel 燃料控制 |
| TypeScript | 解析阶段集成类型擦除，一次解析完成 |

## 快速上手

```rust
use one_engine::OneEngine;

// 三行代码即可运行 JS
let mut engine = OneEngine::<()>::default();
let result = engine.eval("1 + 1")?;
println!("{}", result); // 2
```

## 构建

```bash
cargo build          # 构建
cargo run -p one_cli # 运行 REPL
cargo test           # 测试
```

需要 Rust 1.85+（edition 2024）。
