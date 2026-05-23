# One

100% Rust 实现的通用 JavaScript 运行时，目标完整实现 ES2024+ 规范，内置 TypeScript 支持，可独立使用也可嵌入 Rust 项目。

[English](README.md)

## 设计目标

**四维极致原则：最高性能、最少代码、最多兼容、最简接入**

- 100% Rust，零 C/C++ 依赖（不依赖 V8/JSC/QuickJS 等现有引擎）
- 完整 ES2024+ 规范合规（通过 Test262 验证）
- 内置 TypeScript 类型擦除（解析阶段完成，零额外开销）
- 分代垃圾回收器（Generational GC），支持 WeakRef / FinalizationRegistry
- 寄存器式字节码 VM + 多态内联缓存
- NaN-boxing 值表示，所有 JS 值压缩到一个 `u64`
- 惰性解析 + 字节码缓存 + 堆快照：极致启动性能
- Cargo feature flags 按需裁剪，支持 wasm32 编译目标

## 架构

```
源码 (JS/TS/JSX/TSX)
  │
  ▼
┌──────────────────────────┐
│  解析器 (one_parser)      │  词法分析 + AST + TS 类型擦除
│  · 惰性解析              │  Arena 分配器管理 AST 节点
│  · Arena bump alloc      │
└───────────┬──────────────┘
            │ AST
            ▼
┌──────────────────────────┐
│  编译器 (one_compiler)    │  常量折叠 + 窥孔优化
└───────────┬──────────────┘
            │ 字节码 (CodeBlock)
            ▼
┌──────────────────────────┐
│  寄存器虚拟机 (one_vm)    │  多态内联缓存
│  · Shape / 隐藏类         │  类型特化快速路径
│  · 内联属性存储           │
└───────────┬──────────────┘
            │
     ┌──────┴──────┐
     ▼              ▼
┌────────┐   ┌───────────────┐
│ 分代 GC │   │ 运行时         │  one_gc / one_runtime
│(one_gc)│   │ 内置对象+宿主API│
│ 新生代  │   │ 事件循环       │
│ 老年代  │   │ 模块系统       │
└────────┘   └───────────────┘
```

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

## Crate 结构

| Crate | 说明 |
|-------|------|
| `one_core` | 核心共享类型 — `JsValue` (NaN-boxing)、`GcPtr`、`OneError`、`InternId`、字符串实习池 |
| `one_parser` | 词法分析 + AST + Pratt 解析器 + TS 类型擦除 + 惰性解析 |
| `one_compiler` | AST → 寄存器字节码编译器，含常量折叠 |
| `one_gc` | 分代 GC — 新生代 Scavenger + 标记-压缩 + `derive(Trace)` |
| `one_vm` | 寄存器式字节码虚拟机，含 Shape 系统与内联缓存 |
| `one_runtime` | 内置对象 + 事件循环 + 模块系统 + 宿主 API |
| `one_engine` | 嵌入 API — `OneEngine<T>`、Builder、Preset、`FromJs`/`IntoJs` |
| `one_bridge` | Sentinel AI 适配层 |
| `one_cli` | REPL + CLI 入口 |

## 快速上手

### 作为嵌入引擎

```rust
use one_engine::OneEngine;

// 三行代码即可运行 JS
let mut engine = OneEngine::<()>::default();
let result = engine.eval("1 + 1")?;
println!("{}", result); // 2
```

### 携带宿主数据

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

### 沙箱模式

```rust
let mut engine = OneEngine::<()>::builder()
    .bare(true)
    .fuel(10_000)
    .build();

engine.install::<ObjectBuiltin>();
engine.install::<ArrayBuiltin>();
engine.install::<MathBuiltin>();
// 无 I/O 能力 — 完全沙箱化
```

## 构建

```bash
cargo build          # 构建
cargo run -p one_cli # 运行 REPL
cargo test           # 测试
```

需要 Rust 1.85+（edition 2024）。

## 许可证

本项目双许可，任选其一：

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT License ([LICENSE-MIT](LICENSE-MIT) 或 <http://opensource.org/licenses/MIT>)
