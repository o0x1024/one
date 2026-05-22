# One JS Runtime — 设计规格文档

## 概述

**One** 是一个 100% Rust 实现的通用 JavaScript 运行时，目标是完整实现 ES2024+ 规范，内置 TypeScript 支持，可独立使用也可嵌入 Rust 项目。Sentinel AI 是其第一个集成用户，用于替换当前基于 Deno Core/V8 的插件运行时。

**项目位置：** `/Users/like/code/one`（项目名称 `one`）

## 设计目标

**四维极致原则：最高性能、最少代码、最多兼容、最简接入**

- 100% Rust，零 C/C++ 依赖（不依赖 V8/JSC/QuickJS 等现有引擎）
- 完整 ES2024+ 规范合规（通过 Test262 验证），含 Annex B Web 兼容
- 内置 TypeScript 类型擦除（解析阶段顺带完成，零额外开销）
- 分代垃圾回收器（Generational GC），支持 WeakRef/FinalizationRegistry
- 渐进式执行策略：字节码解释器 → 预留 JIT 接口（Cranelift）
- 惰性解析 + 字节码缓存 + 堆快照：极致启动性能
- Cargo feature flags 按需裁剪，最小嵌入体积
- 支持 wasm32 编译目标
- 独立通用定位，Sentinel AI 是第一个集成用户

## 技术借鉴

| 来源 | 借鉴内容 |
|------|----------|
| V8 | 分代 GC 架构（Scavenger + Mark-Compact）、内联缓存（IC）、隐藏类/Shape 系统、写屏障设计 |
| pipa | NaN-boxing 值表示、寄存器 VM 设计、16MB nursery + bump allocation、write barriers |
| Boa | CodeBlock 结构、字节码调度表（函数指针数组）、RuntimeLimits、ECMAScript 规范映射方式 |
| oxc | 区分 BindingIdentifier/IdentifierReference 的精确 AST 设计、arena 分配器模式 |
| QuickJS | 轻量级 Promise 调度模式 |
| Deno | op 系统的 JS-to-Rust FFI 设计（宿主 API 绑定） |

---

## 一、整体架构

### 执行流水线

```
源码 (JS/TS/JSX/TSX)
  │
  ▼
┌──────────────────────────┐
│  解析器 (one_parser)      │  含 Lexer + AST + TS 类型擦除
│  · 惰性解析：顶层立即解析  │  Arena 分配器管理 AST 节点
│  · 函数体延迟到首次调用    │  TS 语法在解析时直接跳过
│  · Arena bump alloc       │
└───────────┬──────────────┘
            │ AST (arena-allocated)
            ▼
┌──────────────────────────┐
│  字节码编译器             │  one_compiler
│  (Compiler)              │  含类型特化指令生成
│  · 支持 eval() 运行时编译 │
└───────────┬──────────────┘
            │ 字节码 (CodeBlock)          ┌──────────────────┐
            ▼                             │  字节码缓存       │
┌──────────────────────────┐              │  序列化/反序列化   │
│  寄存器虚拟机 (one_vm)    │◄────────────┤  避免重复编译      │
│  · 多态内联缓存 (Poly IC) │              └──────────────────┘
│  · 特化指令快速路径       │
│  · JIT 接口预留           │
└───────────┬──────────────┘
            │
     ┌──────┴──────┐
     ▼              ▼
┌────────┐   ┌───────────────┐
│ 分代 GC │   │ 运行时         │  one_gc / one_runtime
│(one_gc)│   │ 内置对象+宿主API│
│ 新生代  │   │ 事件循环       │
│ 老年代  │   │ 模块系统       │
│ WeakRef │   │ console/fetch  │
└────────┘   └───────────────┘
```

### Crate 组织（精简为 8 个）

合并关系紧密的 crate，减少跨 crate 样板代码：

```
one/                        (位于 /Users/like/code/one)
├── Cargo.toml              workspace 根
├── crates/
│   ├── one_core/           核心共享类型 (JsValue, GcPtr, OneError, InternId, 字符串实习池)
│   ├── one_parser/         解析器 (含 Lexer + AST + TS 类型擦除，Arena 分配)
│   ├── one_compiler/       AST → 字节码编译 (含 eval 运行时编译接口)
│   ├── one_gc/             分代 GC (含 derive(Trace) proc-macro)
│   ├── one_vm/             寄存器式字节码虚拟机 (含多态 IC)
│   ├── one_runtime/        内置对象 + 事件循环 + 模块系统 + 宿主 API
│   ├── one_engine/         统一嵌入 API (Builder 模式, Serde 集成)
│   └── one_cli/            REPL + CLI 入口
└── tests/                  集成测试 + Test262 运行器
```

合并策略说明：
- `one_lexer` + `one_ast` + `one_typescript` → 合入 `one_parser`：词法分析、AST 定义、TS 类型擦除都是解析层的内部实现，无需对外暴露独立 crate
- `one_interner` → 合入 `one_core`：`InternId` 已在 core 定义，实习池实现也应在此
- `one_host` → 合入 `one_runtime`：宿主 API 与内置对象紧密耦合，拆分反而增加公共接口

**`one_core` 的职责**：定义跨 crate 共享的核心类型——`JsValue`（NaN-boxing）、`GcPtr`（GC 指针原语）、`OneError`（错误类型层次结构）、`InternId`（字符串实习标识）、字符串实习池实现。

### 依赖关系

```
one_cli ──→ one_engine ──→ one_runtime ──→ one_vm ──→ one_compiler ──→ one_parser
                                │              │
                                ▼              ▼
                            one_gc         one_core (被所有 crate 共同依赖)
```

### 惰性解析 (Lazy Parsing)

大幅提升启动性能——大多数函数在首次运行前不会被调用：

```
首次加载：
  1. 完整解析顶层代码 + 所有函数签名
  2. 函数体只做"预扫描"（匹配括号、记录作用域信息），不构建 AST
  3. 编译并执行顶层代码

函数首次调用时：
  1. 完整解析该函数体 → 构建 AST
  2. 编译为字节码 → 替换占位 CodeBlock
  3. 执行
```

预扫描记录的信息：函数体的源码范围、是否包含 `eval`/`arguments`/`with`（影响作用域优化决策）。

### AST Arena 分配器

AST 节点使用 bump/arena 分配器（借鉴 oxc 的 `oxc_allocator`）：

```rust
struct AstArena {
    chunks: Vec<Vec<u8>>,
    current: *mut u8,
    end: *mut u8,
}

impl AstArena {
    fn alloc<T>(&self, value: T) -> &T;  // bump allocation，极快
    fn reset(&mut self);                  // 编译完成后一次性释放所有 AST 节点
}
```

AST 是短命的（解析后编译为字节码即丢弃），arena 分配避免了逐节点 alloc/dealloc 的开销，且整体释放零碎片。

---

## 二、值表示与对象模型

### NaN-boxing 值表示

所有 JS 值压缩到一个 `u64` 中，采用 NaN-boxing 方案：

```
64 位布局：
┌─────────────────────── 64 bits ───────────────────────┐
│  浮点数 (f64):  正常 IEEE 754 double                    │
│  整数 (i32):    NaN 标记 + 32 位有符号整数               │
│  布尔值:        NaN 标记 + 0/1                          │
│  null:          特殊 NaN 模式                           │
│  undefined:     特殊 NaN 模式                           │
│  对象指针:      NaN 标记 + 48 位指针                     │
│  字符串指针:    NaN 标记 + 48 位指针                     │
│  Symbol:        NaN 标记 + 48 位指针                     │
└────────────────────────────────────────────────────────┘

标记位分布（利用 NaN 的 quiet NaN 空间）：
- f64 的 quiet NaN: 指数位全1 + 最高有效位为1
- 可用空间: 51 位（除去 NaN 标记位）
- 类型标签占用 3-4 位，剩余 47-48 位用于指针/值
```

Rust 定义：

```rust
#[derive(Clone, Copy)]
struct JsValue(u64);

impl JsValue {
    fn is_number(&self) -> bool;
    fn is_object(&self) -> bool;
    fn is_string(&self) -> bool;
    fn is_boolean(&self) -> bool;
    fn is_null(&self) -> bool;
    fn is_undefined(&self) -> bool;
    fn is_symbol(&self) -> bool;

    fn as_f64(&self) -> f64;
    fn as_i32(&self) -> i32;
    fn as_object(&self) -> GcPtr<JsObject>;
    fn as_string(&self) -> GcPtr<JsString>;

    fn from_f64(v: f64) -> Self;
    fn from_i32(v: i32) -> Self;
    fn from_bool(v: bool) -> Self;
    fn null() -> Self;
    fn undefined() -> Self;
}
```

### 对象模型：Shape (Hidden Class) 系统 + 内联属性存储

```rust
struct JsObject {
    shape: GcPtr<Shape>,
    inline_properties: [JsValue; N],  // 前 N 个属性内联存储在对象头部（避免间接寻址）
    overflow_properties: Option<Box<Vec<JsValue>>>,  // 超出内联容量的属性
    elements: Option<Vec<JsValue>>,   // 整数索引属性（密集数组优化）
    prototype: Option<GcPtr<JsObject>>,
    extensible: bool,
    kind: ObjectKind,                 // Ordinary / Array / Function / ...
}

struct Shape {
    id: ShapeId,
    transitions: HashMap<InternId, GcPtr<Shape>>,  // 属性名 → 下一个 shape
    property_table: Vec<PropertyDescriptor>,        // 属性名 → offset 映射
    parent: Option<GcPtr<Shape>>,                   // 转换链父节点
    inline_capacity: u8,                            // 该 shape 链的内联属性容量
}

struct PropertyDescriptor {
    key: InternId,
    offset: u32,
    flags: PropertyFlags,  // writable, enumerable, configurable
}
```

**内联属性存储**（借鉴 V8 in-object properties）：前 N 个属性（默认 4-8 个，通过 slack tracking 动态调整）直接嵌入对象结构体中，属性访问只需 `对象指针 + 固定偏移`，省去一次堆指针解引用。超出内联容量的属性溢出到 `overflow_properties`。

**Slack tracking**：首次创建某 Shape 的对象时多分配若干内联槽位，观察实际使用后收缩到精确值，后续同 Shape 对象使用收缩后的容量。

Shape 转换链：相同属性添加顺序的对象共享 Shape 实例。

### 多态内联缓存（Polymorphic Inline Cache）

三级 IC 设计，覆盖从单态到巨态的全部场景：

```rust
enum InlineCache {
    Uninitialized,
    Monomorphic(MonoIC),          // 单一 shape — 最快路径
    Polymorphic(PolyIC),          // 2-4 种 shape — 常见多态
    Megamorphic,                  // >4 种 shape — 退化为哈希查找
}

struct MonoIC {
    shape_id: ShapeId,
    offset: u32,
}

struct PolyIC {
    entries: ArrayVec<MonoIC, 4>,  // 最多 4 个 shape 缓存
}
```

属性访问流程：
1. **单态 IC**：检查 shape_id 匹配 → 直接 offset 访问（1 次比较）
2. **多态 IC**：线性扫描 2-4 个 entry → 匹配则 offset 访问（2-4 次比较）
3. **巨态 IC**：退化为 Shape property_table 哈希查找
4. 未命中时根据当前 IC 状态决定升级路径：Uninit → Mono → Poly → Mega

### 字符串表示：双编码法（借鉴 V8）

JS 规范中字符串是 UTF-16 码元序列。使用 UTF-8 会导致 `charCodeAt(i)` 等索引操作为 O(n)。
采用**双编码法**：

```rust
enum JsString {
    Static(&'static str),           // 内置字符串常量
    Interned(InternId),             // 池化短字符串
    Latin1(GcPtr<Latin1String>),    // 纯 ASCII/Latin1 — 1 byte/char，O(1) 索引
    Utf16(GcPtr<Utf16String>),      // 含非 ASCII — 2 bytes/char，O(1) 索引
    Rope(GcPtr<RopeNode>),          // 大字符串拼接的延迟求值优化
}

struct Latin1String {
    data: Vec<u8>,     // 每个字节 = 一个字符，索引 O(1)
    hash: u32,
}

struct Utf16String {
    data: Vec<u16>,    // UTF-16 码元序列，完全符合 ES 规范语义
    hash: u32,
}

struct RopeNode {
    left: JsString,
    right: JsString,
    length: usize,     // 以 UTF-16 码元计的长度
}
```

**关键优势**：
- 实践中 >90% 的字符串是纯 ASCII，用 Latin1 编码内存占用与 UTF-8 相同
- `str[i]`、`charCodeAt(i)` 均为 O(1)（Latin1 和 UTF-16 都是定长编码）
- 完美支持 ES 规范的 UTF-16 语义，包括代理对和孤立代理对
- 字符串拼接时，如果双方都是 Latin1 且结果仍在 Latin1 范围内，保持 Latin1 编码

---

## 三、字节码与虚拟机

### 指令编码：定长 32 位

```
三种指令格式：

ABC 格式:   ┌─opcode(8)─┬──A(8)──┬──B(8)──┬──C(8)──┐
ABx 格式:   ┌─opcode(8)─┬──A(8)──┬──────Bx(16)─────┐
AsBx 格式:  ┌─opcode(8)─┬──A(8)──┬─────sBx(16)─────┐
```

- 定长 32 位解码简单，CPU 缓存友好
- 8 位 opcode（256 个操作码），足够覆盖完整 ES 规范
- A/B/C 字段可引用寄存器、常量池索引或立即数
- **Wide 指令前缀**：当 8 位寄存器编号（256）或 16 位操作数不够时，使用 `Wide` 前缀指令，后跟扩展编码的操作数。这允许在常见情况下保持 32 位紧凑编码，同时支持大函数/大常量池的极端场景

### 核心指令集

```
// === 数据移动 ===
LoadConst       r(A), const(Bx)     加载常量到寄存器
Move            r(A), r(B)          寄存器间移动
LoadUndef       r(A)                加载 undefined
LoadNull        r(A)                加载 null
LoadTrue        r(A)                加载 true
LoadFalse       r(A)                加载 false
LoadInt         r(A), imm(sBx)      加载小整数立即数

// === 算术运算 ===
Add             r(A), r(B), r(C)    A = B + C
Sub             r(A), r(B), r(C)    A = B - C
Mul             r(A), r(B), r(C)    A = B * C
Div             r(A), r(B), r(C)    A = B / C
Mod             r(A), r(B), r(C)    A = B % C
Exp             r(A), r(B), r(C)    A = B ** C
Neg             r(A), r(B)          A = -B

// === 位运算 ===
BitAnd          r(A), r(B), r(C)
BitOr           r(A), r(B), r(C)
BitXor          r(A), r(B), r(C)
Shl             r(A), r(B), r(C)
Shr             r(A), r(B), r(C)
UShr            r(A), r(B), r(C)
BitNot          r(A), r(B)

// === 比较 / 逻辑 ===
Eq              r(A), r(B), r(C)    A = (B == C)
StrictEq        r(A), r(B), r(C)    A = (B === C)
Lt              r(A), r(B), r(C)    A = (B < C)
Lte             r(A), r(B), r(C)    A = (B <= C)
Not             r(A), r(B)          A = !B
TypeOf          r(A), r(B)          A = typeof B
InstanceOf      r(A), r(B), r(C)    A = (B instanceof C)
In              r(A), r(B), r(C)    A = (B in C)

// === 控制流 ===
Jump            sBx                  无条件跳转
JumpIfTrue      r(A), sBx            条件跳转
JumpIfFalse     r(A), sBx
JumpIfNullish   r(A), sBx            可选链支持 (?.)

// === 属性访问（配合 IC） ===
GetProp         r(A), r(B), name(C)  A = B.name (IC slot)
SetProp         r(A), name(B), r(C)  A.name = C (IC slot)
GetElem         r(A), r(B), r(C)     A = B[C]
SetElem         r(A), r(B), r(C)     A[B] = C
DeleteProp      r(A), r(B), name(C)  delete B.name

// === 函数调用 ===
Call            r(A), r(func), argc   A = func(args...)
CallMethod      r(A), r(obj), name, argc
New             r(A), r(ctor), argc   A = new ctor(args...)
Return          r(A)
TailCall        r(func), argc

// === 闭包 / 作用域 ===
CreateClosure   r(A), func(Bx)       创建闭包
GetUpvalue      r(A), upval(B)        读取上值
SetUpvalue      upval(A), r(B)        设置上值
CloseUpvalue    upval(A)              关闭上值

// === 对象 / 数组 ===
CreateObject    r(A), size(B)
CreateArray     r(A), size(B)
SetArrayElem    r(A), index(B), r(C)
Spread          r(A), r(B)           展开操作

// === 异常处理 ===
TryStart        handler(Bx)          进入 try 块
TryEnd                               离开 try 块
Throw           r(A)                 抛出异常
CatchBind       r(A)                 绑定 catch 参数

// === 异步 / 生成器 ===
Await           r(A), r(B)           A = await B
Yield           r(A), r(B)           生成器 yield
YieldStar       r(A), r(B)           yield* 委托
CreateAsyncGen  r(A), func(Bx)       创建异步生成器

// === 迭代器 ===
GetIterator     r(A), r(B)           A = B[Symbol.iterator]()
IteratorNext    r(A), r(B)           A = B.next()
IteratorDone    r(A), r(B)           A = B.done
IteratorValue   r(A), r(B)           A = B.value
ForIn           r(A), r(B)           for-in 枚举

// === 解构 ===
Destructure     r(A), r(B), pattern(C)

// === 类 ===
DefineClass     r(A), super(B), body(C)
DefineMethod    r(A), name(B), func(C)
GetSuper        r(A)
SuperCall       r(A), argc

// === 模块 ===
ImportModule    r(A), specifier(Bx)
ExportBinding   name(A), r(B)

// === 类型特化指令（快速路径）===
AddInt          r(A), r(B), r(C)    整数快速加法（跳过类型检查）
AddNum          r(A), r(B), r(C)    数字快速加法（直接 f64）
AddStr          r(A), r(B), r(C)    字符串快速拼接
EqInt           r(A), r(B), r(C)    整数快速比较
LtInt           r(A), r(B), r(C)    整数快速小于
GetPropCached   r(A), r(B), ic(C)   IC 命中时的属性快速访问
```

编译器通过静态类型推断或运行时 profiling 反馈决定使用通用指令还是特化指令。
特化指令在类型守卫失败时 fallback 到通用指令路径。

### CodeBlock 结构

```rust
struct CodeBlock {
    name: InternId,                          // 函数/脚本名
    bytecode: Vec<u32>,                      // 32 位指令流
    constants: Vec<JsValue>,                 // 常量池
    register_count: u16,                     // 所需寄存器数量
    param_count: u16,                        // 形式参数数量
    upvalue_count: u16,                      // 上值数量
    rest_param: bool,                        // 是否有 rest 参数
    is_strict: bool,                         // 严格模式
    is_async: bool,                          // async 函数
    is_generator: bool,                      // generator 函数
    exception_handlers: Vec<ExceptionHandler>,
    source_map: Vec<SourceMapping>,          // 字节码偏移 ↔ 源码位置
    inline_caches: Vec<InlineCacheEntry>,    // IC 槽位
    inner_functions: Vec<GcPtr<CodeBlock>>,  // 嵌套函数的 CodeBlock
}

struct ExceptionHandler {
    try_start: u32,      // try 块起始 PC
    try_end: u32,        // try 块结束 PC
    catch_start: u32,    // catch 处理器 PC
    finally_start: Option<u32>,  // finally 处理器 PC
    catch_register: u8,  // catch 参数绑定的寄存器
}

struct SourceMapping {
    bytecode_offset: u32,
    line: u32,
    column: u32,
}
```

### VM 调度机制

```rust
type OpcodeHandler = fn(&mut Vm) -> ControlFlow;

static HANDLERS: [OpcodeHandler; 256] = [
    handle_load_const,    // 0x00
    handle_move,          // 0x01
    handle_add,           // 0x02
    // ... 每个 opcode 一个处理函数
];

struct Vm {
    heap: Heap,
    stack: Vec<JsValue>,          // 共享值栈（所有调用帧的寄存器）
    frames: Vec<CallFrame>,       // 调用帧栈
    global_object: GcPtr<JsObject>,
    runtime_limits: RuntimeLimits,
}

struct CallFrame {
    code: GcPtr<CodeBlock>,
    pc: usize,                    // 程序计数器
    base: usize,                  // 栈中寄存器基址
    return_register: u8,          // 返回值写入调用者的哪个寄存器
    flags: CallFrameFlags,        // EXIT_EARLY, CONSTRUCT, etc.
}

struct RuntimeLimits {
    max_recursion_depth: usize,   // 默认 512
    max_stack_size: usize,        // 默认 1024
    max_loop_iterations: u64,     // 默认无限制
    execution_timeout: Option<Duration>,
}

// 主执行循环
impl Vm {
    fn run(&mut self) -> Result<JsValue, OneError> {
        loop {
            let frame = self.current_frame();
            let instruction = frame.fetch();
            let opcode = instruction.opcode();
            
            match HANDLERS[opcode as usize](self) {
                ControlFlow::Continue => {},
                ControlFlow::Return(val) => {
                    if self.frames.len() <= 1 {
                        return Ok(val);
                    }
                    self.pop_frame(val);
                },
                ControlFlow::Exception(err) => {
                    if !self.unwind_exception(err)? {
                        return Err(err);
                    }
                },
                ControlFlow::Yield(val) => {
                    self.suspend_frame(val);
                    return Ok(val);
                },
            }
        }
    }
}
```

### 运行时编译：eval() / new Function()

`eval()` 需要在运行时完成完整的"源码 → 解析 → 编译 → 执行"流水线。VM 持有编译器的引用以支持此场景：

```rust
impl Vm {
    fn eval_direct(&mut self, source: &str, scope: &Scope) -> Result<JsValue> {
        // 直接 eval：在调用者的作用域中解析和编译
        // 可访问调用者的局部变量
        let code = self.compiler.compile_eval(source, scope)?;
        self.execute(code)
    }

    fn eval_indirect(&mut self, source: &str) -> Result<JsValue> {
        // 间接 eval / new Function()：在全局作用域中编译
        let code = self.compiler.compile_eval(source, &self.global_scope)?;
        self.execute(code)
    }
}
```

含 `eval` 的函数会禁用某些优化（如变量的寄存器分配需要回退到作用域对象查找），预扫描阶段会标记函数是否包含 `eval` 调用。

### Strict Mode vs Sloppy Mode

两种模式的行为差异需要编译器和 VM 多处配合：

| 差异点 | Strict Mode | Sloppy Mode |
|--------|-------------|-------------|
| `this` 绑定 | 函数调用中 `this` 为 `undefined` | `this` 自动绑定到全局对象 |
| `arguments` | 形参与 arguments 独立（拷贝语义） | 形参与 arguments 映射（修改同步） |
| 变量声明 | 未声明变量赋值抛 ReferenceError | 隐式创建全局变量 |
| `with` 语句 | SyntaxError | 创建动态作用域（影响变量查找） |
| `delete` | 删除变量/函数抛 SyntaxError | 静默返回 false |
| 八进制字面量 | `0123` 为 SyntaxError | `0123` 解释为八进制 83 |
| 重复参数名 | SyntaxError | 允许（后者覆盖前者） |

`CodeBlock.is_strict` 标志在编译期确定，影响编译器的代码生成和 VM 的运行时行为。`with` 语句会禁用该作用域内的静态变量查找优化。

### 字节码缓存与堆快照

**字节码缓存**：将编译好的 CodeBlock 序列化到磁盘，后续加载同一模块时跳过解析和编译：

```rust
impl CodeBlock {
    fn serialize(&self, writer: &mut impl Write) -> Result<()>;
    fn deserialize(reader: &mut impl Read) -> Result<Self>;
}
```

**堆快照 (Snapshot)**：将已初始化的内置对象/全局状态序列化为二进制快照：
- 启动时反序列化快照直接还原堆状态，跳过数百个内置方法的运行时注册
- V8 的快照机制使 Node.js 启动时间从 ~100ms 降至 ~30ms

---

## 四、分代垃圾回收器

### 堆内存布局

```
One Heap
├── Young Generation (新生代)
│   ├── Nursery (16MB) — bump allocation
│   └── Survivor Space — Minor GC 后存活对象暂存
│
├── Old Generation (老年代)
│   ├── Old Space — Mark-Compact 管理
│   └── Large Object Space — >256KB 独立分配
│
└── Metadata
    ├── Remembered Set — 记录 old→young 引用
    └── Mark Bitmap — 标记位图
```

### 新生代：Scavenger

- **Nursery (16MB)**：bump allocation，分配开销仅为指针递增
- **Minor GC (Scavenge)**：Cheney 半空间拷贝算法
  - 只扫描根集 + 写屏障记录的 old→young 引用（Remembered Set）
  - 存活对象拷贝到 Survivor Space
  - 未引用对象随 Nursery 整体回收（零开销）
- **晋升策略**：对象存活超过 2 次 Minor GC 后晋升到老年代
- **触发条件**：Nursery 空间耗尽时触发

### 老年代：增量标记-压缩

- **三色标记法**（白/灰/黑）
  - 白：未访问
  - 灰：已发现但子引用未完全扫描
  - 黑：已扫描完毕
- **增量标记**：每次标记一个时间片（如 5ms），与 VM 执行交替，避免长停顿
- **压缩阶段**：移动存活对象消除碎片，更新所有引用
- **触发条件**：老年代使用率超过 75%

### 写屏障 (Write Barrier)

```rust
fn write_barrier(host: &GcPtr, field_slot: &mut GcPtr, value: GcPtr) {
    *field_slot = value;
    
    // 分代屏障：记录 old→young 引用
    if host.is_old_gen() && value.is_young_gen() {
        remembered_set.record(field_slot);
    }
    
    // 增量标记屏障：防止漏标（黑→白）
    if marking_in_progress() && host.is_black() && value.is_white() {
        mark_gray(value);
    }
}
```

### GC 安全点 (Safepoint)

- 字节码循环回边（back-edge）和函数调用点为安全点
- 安全点检查 GC 标志位，决定是否触发回收
- 安全点也用于检查执行超时和终止请求

### GC API + derive(Trace) 宏

```rust
// 所有 GC 管理的对象实现 Trace trait
trait Trace {
    fn trace(&self, tracer: &mut Tracer);
}

// derive 宏自动生成 Trace 实现，大幅减少样板代码
#[derive(Trace)]
struct JsObject {
    shape: Gc<Shape>,               // 自动追踪
    properties: Vec<JsValue>,       // 自动追踪内部 GcPtr
    #[no_trace]
    extensible: bool,               // 跳过非 GC 字段
    #[no_trace]
    kind: ObjectKind,
}

// GC 智能指针
#[derive(Clone)]
struct Gc<T: Trace> {
    ptr: NonNull<GcHeader>,
    _marker: PhantomData<T>,
}

// GC 根引用（自动注册/注销）
struct GcRoot<T: Trace> {
    inner: Gc<T>,
    root_id: RootId,
}

// 弱引用指针（支持 WeakRef/FinalizationRegistry）
struct GcWeak<T: Trace> {
    ptr: NonNull<GcHeader>,
    _marker: PhantomData<T>,
}

impl<T: Trace> GcWeak<T> {
    fn upgrade(&self) -> Option<Gc<T>>;  // 对象已回收则返回 None
}

// 堆管理接口
struct Heap {
    nursery: Nursery,
    old_space: OldSpace,
    large_object_space: LargeObjectSpace,
    remembered_set: RememberedSet,
    weak_refs: Vec<GcWeak<dyn Trace>>,           // 注册的弱引用
    finalization_queue: Vec<FinalizationEntry>,   // 待执行的回收回调
    
    fn alloc<T: Trace>(&mut self, value: T) -> Gc<T>;
    fn alloc_weak<T: Trace>(&mut self, target: &Gc<T>) -> GcWeak<T>;
    fn register_finalizer(&mut self, target: &Gc<dyn Trace>, callback: Gc<JsFunction>);
    fn collect_minor(&mut self);
    fn collect_major(&mut self);
    fn drain_finalization_queue(&mut self) -> Vec<FinalizationEntry>;
    fn stats(&self) -> HeapStats;
}

struct HeapStats {
    nursery_used: usize,
    nursery_capacity: usize,
    old_space_used: usize,
    old_space_capacity: usize,
    total_gc_count: u64,
    total_gc_time: Duration,
}
```

`derive(Trace)` 由 `one_gc` crate 内含的 proc-macro 子 crate 提供，自动遍历结构体字段生成 trace 方法。`#[no_trace]` 标记非 GC 字段，避免手写大量样板代码（借鉴 Boa 的 `boa_gc` 设计）。

### WeakRef / FinalizationRegistry 集成

GC 回收阶段的弱引用处理流程：

```
Mark 阶段结束后：
  1. 扫描 weak_refs 列表
  2. 如果弱引用的目标对象未被标记（已死亡）→ 将弱引用置为 None
  3. 如果目标对象注册了 FinalizationRegistry → 加入 finalization_queue
  4. Sweep/Compact 正常回收

GC 完成后（在 VM 的安全点）：
  5. 清空 finalization_queue，在 JS 微任务中执行回调
```

`GcHeader` 中增加 `weak_ref_count: u16` 和 `has_finalizer: bool` 标记，只有注册了弱引用或 finalizer 的对象才参与上述流程，零开销于普通对象。

### 与 VM 的集成

- VM 的寄存器栈、调用帧中的 JsValue 通过 GcRoot 注册为根集
- 闭包的上值（upvalue）通过 Gc<T> 管理
- CodeBlock、Shape、JsObject、JsString 等堆对象均参与 GC 追踪
- 全局对象、模块缓存等持久引用通过 GcRoot 注册
- WeakRef 通过 GcWeak<T> 实现，FinalizationRegistry 回调在 GC 后作为微任务执行

---

## 五、运行时

### 事件循环 (Event Loop)

```
每一轮 tick：
1. 执行到期的定时器回调
2. 处理 I/O 完成回调
3. 清空微任务队列（Promise then/catch/finally）
4. 执行 setImmediate 回调
5. 处理 close 回调（socket close 等）
6. 检查是否还有待处理事件 → 是则继续，否则退出

关键原则：
- 微任务在每个阶段结束后立即全部执行
- I/O 多路复用基于 mio (通过 tokio 的底层)
- async/await 映射到 Promise + 字节码 Await/Yield 指令
```

实现：

```rust
struct EventLoop {
    microtask_queue: VecDeque<Gc<JsFunction>>,
    timer_heap: BinaryHeap<TimerEntry>,
    io_poller: Poller,  // 基于 mio
    pending_ops: Vec<PendingAsyncOp>,
    
    fn run_until_complete(&mut self, vm: &mut Vm) -> Result<()>;
    fn enqueue_microtask(&mut self, task: Gc<JsFunction>);
    fn set_timer(&mut self, callback: Gc<JsFunction>, delay: Duration, repeat: bool) -> TimerId;
    fn cancel_timer(&mut self, id: TimerId);
}
```

### 内置对象实现优先级

**第一优先级（MVP 必需）：**

| 对象 | 关键方法 |
|------|----------|
| Object | keys, values, entries, assign, create, defineProperty, freeze, seal |
| Array | push, pop, map, filter, reduce, forEach, find, includes, from, isArray, flat, sort |
| String | slice, substring, indexOf, replace, split, trim, padStart, padEnd, startsWith, endsWith |
| Number | parseInt, parseFloat, isNaN, isFinite, toFixed, toPrecision |
| Boolean | (基本包装) |
| Symbol | for, keyFor, iterator, asyncIterator, hasInstance, toPrimitive |
| Function | call, apply, bind, length, name |
| Error + 子类 | TypeError, RangeError, ReferenceError, SyntaxError, URIError, EvalError |
| Math | random, floor, ceil, round, abs, max, min, pow, sqrt, log |
| JSON | parse, stringify |
| Date | now, parse, getTime, toISOString, toLocaleString |
| Promise | then, catch, finally, all, allSettled, race, any, resolve, reject |
| Map / Set | get, set, has, delete, forEach, keys, values, entries, size |
| WeakMap / WeakSet | get, set, has, delete |
| RegExp | test, exec, match, replace, search, split (flags: g, i, m, s, u, y) |
| ArrayBuffer | byteLength, slice |
| TypedArray | Int8~Float64Array, buffer, byteOffset, byteLength, set, subarray |
| DataView | getInt8~getFloat64, setInt8~setFloat64 |
| Proxy / Reflect | 全部 13 个 trap |

**第二优先级：**

| 对象 | 说明 |
|------|------|
| BigInt | 任意精度整数 |
| SharedArrayBuffer | 共享内存 |
| Atomics | 原子操作 |
| WeakRef | 弱引用（需 GC 弱引用支持，见第四节） |
| FinalizationRegistry | 垃圾回收注册（需 GC finalization 支持） |
| Intl.* | 国际化 API（可选 feature flag） |
| Iterator / AsyncIterator | 迭代器助手方法 |

### 声明式宏注册内置方法

内置对象的方法注册代码高度重复。使用声明式宏减少样板：

```rust
builtin_methods!(Array, prototype, [
    ("push",      array_push,      1),  // (名称, Rust 函数, 参数数量)
    ("pop",       array_pop,       0),
    ("map",       array_map,       1),
    ("filter",    array_filter,    1),
    ("reduce",    array_reduce,    1),
    ("forEach",   array_for_each,  1),
    ("find",      array_find,      1),
    ("includes",  array_includes,  1),
    ("indexOf",   array_index_of,  1),
    ("flat",      array_flat,      0),
    ("sort",      array_sort,      1),
]);

builtin_static_methods!(Array, [
    ("isArray",   array_is_array,  1),
    ("from",      array_from,      1),
    ("of",        array_of,        0),
]);
```

宏自动生成：属性注册到 Shape、函数对象创建、length/name 属性设置。

### RegExp 实现策略

从零实现完整 ES 规范 RegExp 引擎工作量巨大（Unicode property escapes、lookbehind、named groups 等）。采用**适配层包装**策略：

- 内部使用 Rust 生态的 `regex` crate（或 `fancy-regex` 用于支持 lookbehind/lookahead）
- 适配层将 ES RegExp 语法转换为 Rust regex 语法
- ES 特有语义（lastIndex、全局/粘性标志状态、exec 迭代行为）在适配层实现
- 通过 feature flag `regexp` 控制，`minimal` 构建不含 RegExp

### Annex B（Web 兼容性）

ES 规范 Annex B 定义了浏览器历史遗留行为。One 选择**部分实现**：

| 特性 | 是否实现 | 理由 |
|------|----------|------|
| `__proto__` 属性 | 是 | 广泛使用，Node.js/Deno 均支持 |
| `escape()` / `unescape()` | 是 | 仍有使用，实现简单 |
| HTML 注释 `<!-- -->` | 是 | 实际 JS 文件中偶尔出现 |
| `String.prototype.substr` | 是 | 广泛使用的遗留方法 |
| Block-scoped function in sloppy mode | 是 | 规范要求的 Web 兼容行为 |
| RegExp legacy static properties | 否 | 极少使用，增加复杂度 |

通过 feature flag `annex-b`（默认开启）控制。

### 模块系统

```rust
enum ModuleType {
    ESM,   // import/export — 原生支持
    CJS,   // require/module.exports — 兼容层
}

struct ModuleLoader {
    cache: HashMap<ModulePath, GcPtr<Module>>,
    resolvers: Vec<Box<dyn ModuleResolver>>,
    
    fn resolve(&self, specifier: &str, referrer: &str) -> Result<ModulePath>;
    fn load(&self, path: &ModulePath) -> Result<ModuleSource>;
    fn evaluate(&mut self, vm: &mut Vm, module: &Module) -> Result<JsValue>;
}

trait ModuleResolver {
    fn resolve(&self, specifier: &str, referrer: &str) -> Option<ModulePath>;
    fn load(&self, path: &ModulePath) -> Option<String>;
}

struct Module {
    source: ModuleSource,
    code_block: GcPtr<CodeBlock>,
    namespace: GcPtr<JsObject>,
    status: ModuleStatus, // Unlinked, Linking, Linked, Evaluating, Evaluated, Error
    import_entries: Vec<ImportEntry>,
    export_entries: Vec<ExportEntry>,
}
```

ESM 为原生实现，CJS 通过包装为 ESM 语义兼容。模块解析器（resolver）可由宿主自定义，嵌入时可以实现自定义的模块加载逻辑（如从数据库加载插件代码）。

---

## 六、宿主 API 绑定机制

### Op 宏系统

```rust
// 同步 op
#[one_op]
fn op_console_log(args: Vec<JsValue>) -> Result<()> {
    // ...
}

// 异步 op
#[one_op(async)]
async fn op_fetch(url: String, options: FetchOptions) -> Result<Response> {
    // ...
}

// 注册到运行时
let mut runtime = OneRuntime::new();
runtime.register_op("console_log", op_console_log);
runtime.register_op("fetch", op_fetch);
```

宏自动生成：
- JS 参数到 Rust 类型的反序列化
- Rust 返回值到 JS 值的序列化
- 异步 op 到 Promise 的桥接

### 内置宿主 API (one_host)

| API | 实现方式 |
|-----|----------|
| `console.log/warn/error/info/debug` | 同步 op → Rust stdout/stderr |
| `setTimeout/setInterval/clearTimeout/clearInterval` | 事件循环定时器 |
| `fetch` | 异步 op → reqwest/hyper |
| `TextEncoder/TextDecoder` | 同步 op → Rust 编码转换 |
| `URL/URLSearchParams` | 纯 JS 或 Rust op 混合 |
| `atob/btoa` | 同步 op → base64 |
| `crypto.getRandomValues/SubtleCrypto` | 同步/异步 op → ring/rustls |
| 文件 I/O (`readFile/writeFile/mkdir`) | 异步 op → tokio::fs |
| 网络 (`connect/listen`) | 异步 op → tokio::net |

---

## 七、TypeScript 支持（解析阶段集成）

TS 类型擦除**合入解析器**（one_parser），不作为独立 crate。解析器在构建 AST 时直接识别并跳过 TS 语法节点，一次解析完成 JS+TS，零额外开销：

```rust
impl Parser {
    fn parse(&mut self, source: &str, config: ParserConfig) -> Result<Program> {
        // config.typescript == true 时，解析器识别 TS 语法
        // 类型注解、接口、类型别名等在 AST 构建时直接跳过
        // 无需二次遍历或源码级文本替换
    }
}

struct ParserConfig {
    typescript: bool,    // 是否识别 TS 语法
    jsx: bool,           // 是否识别 JSX 语法
    strict_mode: bool,   // 默认严格模式
}
```

解析阶段处理的 TS 特性：
- 类型注解跳过（`: Type`、`as Type`、`<T>` 泛型参数）
- 接口/类型别名声明跳过（`interface`、`type`）
- 枚举声明转换为等价的对象字面量 + 变量声明 AST 节点
- 装饰器保留（装饰器是运行时语义，需生成对应 AST）
- namespace 转换为等价的 IIFE AST 节点

不包含类型检查——仅做语法层面的类型信息剥离。

**相比独立 crate 的优势**：
- 避免解析两次（先解析 TS，再解析 JS）
- 共享词法分析器和 AST 分配器
- 减少一个 crate 的维护成本

---

## 八、嵌入 API (one_engine)

**核心定位：One 的首要目标是作为可嵌入引擎被集成到其他 Rust 项目中。** 独立运行（CLI/REPL）是次要场景。因此嵌入 API 是整个项目最重要的公共接口。

设计原则：
- **三行代码可用**：零配置创建引擎、执行代码、获取结果
- **宿主完全掌控**：JS 代码只能做宿主明确允许的事情
- **零开销互操作**：Rust ↔ JS 类型转换尽可能在编译期完成
- **优雅的资源控制**：内存、CPU、执行时间均可精确限制

### 8.1 Store\<T\> 宿主数据绑定（核心机制）

每个嵌入场景都需要在 host function 中访问宿主状态。借鉴 wasmtime 的 `Store<T>` 模式，引擎携带宿主数据的类型参数：

```rust
// 嵌入者定义宿主数据
struct MyApp {
    db: DatabasePool,
    config: AppConfig,
    user_id: String,
    request_count: u64,
}

// 引擎携带宿主数据
let mut engine = OneEngine::<MyApp>::builder()
    .host_data(MyApp { db, config, user_id, request_count: 0 })
    .build();

// 所有 host function 都可以访问宿主数据
#[one_op]
fn op_query(store: &mut Store<MyApp>, sql: String) -> Result<Vec<Row>> {
    store.data_mut().request_count += 1;
    store.data().db.query(&sql).await
}

// 三行代码最简用法（无宿主数据时用 () 类型）
let mut engine = OneEngine::<()>::default();
engine.eval("1 + 1")?;
```

Sentinel 集成示例——`PluginContext` 作为宿主数据：

```rust
struct PluginHost {
    plugin_id: String,
    category: PluginMainCategory,
    findings: Vec<Finding>,
    finding_sink: Option<Sender<Finding>>,
    run_id: Option<String>,
}

let mut engine = OneEngine::<PluginHost>::builder()
    .host_data(PluginHost { plugin_id, category, findings: vec![], ... })
    .preset(Preset::Standard)
    .enable_typescript(true)
    .build();
```

### 8.2 Builder 模式与预置配置

```rust
let engine = OneEngine::<T>::builder()
    // 宿主数据
    .host_data(my_data)
    // 沙箱模式
    .bare(true)                           // 裸引擎，不注册任何默认全局对象
    .preset(Preset::Safe)                 // 或使用预置配置
    // 资源控制
    .heap_size(64 * 1024 * 1024)
    .nursery_size(16 * 1024 * 1024)
    .fuel(100_000)                        // 初始燃料
    .runtime_limits(RuntimeLimits { .. })
    // 功能开关
    .enable_typescript(true)
    .strict_mode(true)
    // 注册宿主函数和类
    .register_function("add", |a: f64, b: f64| a + b)
    .register_class::<HttpClient>()
    .register_module_resolver(my_resolver)
    // 执行钩子
    .on_before_call(my_call_hook)
    .on_gc(my_gc_hook)
    .build();

// 三种预置配置
enum Preset {
    Bare,      // 空白 globalThis，只有语言原语（undefined/null/true/false）
    Safe,      // 纯计算内置对象（Object/Array/String/Math/JSON/Promise...），无 I/O
    Standard,  // Safe + console + 定时器，无文件/网络
    Full,      // 全部能力，含 fetch/fs/net/crypto
}
```

### 8.3 类型安全函数注册（FromJs/IntoJs trait）

零 Serde 开销的类型安全绑定，借鉴 mlua 的 `FromLua/IntoLua`：

```rust
// 自动类型推断，编译期类型检查
engine.register_function("add", |a: f64, b: f64| -> f64 { a + b });
engine.register_function("greet", |name: String| -> String {
    format!("Hello, {}!", name)
});

// 异步函数
engine.register_async_function("fetch_data", |url: String| async move {
    reqwest::get(&url).await?.text().await
});

// 可访问宿主数据的函数
engine.register_host_function("get_user", |store: &Store<MyApp>| -> String {
    store.data().user_id.clone()
});

// 底层 trait（为基本类型自动实现）
trait FromJs: Sized {
    fn from_js(value: JsValue, engine: &OneEngine<T>) -> Result<Self>;
}
trait IntoJs {
    fn into_js(self, engine: &mut OneEngine<T>) -> Result<JsValue>;
}
// 自动实现：f64, i32, u32, bool, String, &str, Vec<T>, HashMap<K,V>,
// Option<T>, Result<T,E>, (T1, T2, ...) 元组
```

高频函数（如 Sentinel 的 `scan_transaction`）使用 `FromJs/IntoJs` 零开销转换；低频或复杂结构使用 Serde 做自动 JSON 桥接。

### 8.4 Rust 结构体 → JS 类映射（JsClass trait）

借鉴 mlua 的 `UserData` 和 Boa 的 `register_global_class`：

```rust
struct HttpClient {
    base_url: String,
    timeout: Duration,
}

impl JsClass for HttpClient {
    fn class_name() -> &'static str { "HttpClient" }

    fn constructor(args: &Args) -> Result<Self> {
        Ok(HttpClient {
            base_url: args.get::<String>(0)?,
            timeout: Duration::from_secs(args.get_or::<u64>(1, 30)?),
        })
    }

    fn methods(m: &mut MethodRegistry<Self>) {
        m.add_method("get", |this, path: String| async move {
            let url = format!("{}{}", this.base_url, path);
            Ok(reqwest::get(&url).await?.text().await?)
        });
        m.add_method_mut("setTimeout", |this, ms: u64| {
            this.timeout = Duration::from_millis(ms);
            Ok(())
        });
    }

    fn properties(p: &mut PropertyRegistry<Self>) {
        p.add_getter("baseUrl", |this| Ok(this.base_url.clone()));
        p.add_getter("timeoutMs", |this| Ok(this.timeout.as_millis() as u64));
    }
}

engine.register_class::<HttpClient>();
// JS 侧：
// const client = new HttpClient("https://api.example.com", 60);
// const data = await client.get("/users");
// console.log(client.baseUrl);
```

也支持 derive 宏简化简单场景：

```rust
#[derive(JsClass)]
#[js(name = "Point")]
struct Point {
    #[js(getter, setter)]
    x: f64,
    #[js(getter, setter)]
    y: f64,
}

#[js_methods]
impl Point {
    #[js(constructor)]
    fn new(x: f64, y: f64) -> Self { Point { x, y } }

    fn distance(&self, other: &Point) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}
```

### 8.5 Fuel 燃料式执行控制（借鉴 wasmtime）

确定性的执行限制，比超时更安全更可控：

```rust
// 设置初始燃料
engine.set_fuel(10_000);

// 执行——每条字节码指令消耗一定燃料
let result = engine.call_function("compute", &args);

match result {
    Ok(val) => { /* 正常完成 */ },
    Err(OneError::OutOfFuel { consumed }) => {
        // 燃料耗尽，安全中断
        // 可以补充燃料后从中断点恢复
        engine.add_fuel(50_000);
        engine.resume()?;
    },
    Err(e) => { /* 其他错误 */ },
}

// 查询剩余燃料（可用于计费、配额管理）
let remaining = engine.get_fuel();

// 也支持 epoch 式中断（低开销、非确定性，适用于超时场景）
engine.set_epoch_deadline(1);
// 从其他线程触发中断：
let epoch_handle = engine.epoch_handle();  // EpochHandle: Send + Sync
std::thread::spawn(move || {
    std::thread::sleep(Duration::from_secs(5));
    epoch_handle.increment();  // 触发引擎中断
});
```

fuel 与 epoch 的对比：

| 特性 | Fuel | Epoch |
|------|------|-------|
| 确定性 | 完全确定（同一程序同一燃料 = 同一中断点） | 非确定性（依赖墙钟时间） |
| 开销 | 每条指令 ~2-5% 性能损失 | ~1% 性能损失 |
| 精度 | 指令级 | 取决于检查间隔 |
| 可计费 | 是（消耗量 = 计算量） | 否 |
| 跨线程触发 | 不需要 | 需要（通过 EpochHandle） |

### 8.6 裸引擎与沙箱模式

安全敏感嵌入的核心——JS 代码只能做宿主明确允许的事情：

```rust
// 裸引擎：globalThis 上只有语言原语
let mut engine = OneEngine::<()>::builder()
    .bare(true)
    .fuel(10_000)
    .build();

// 选择性安装内置对象
engine.install::<ObjectBuiltin>();
engine.install::<ArrayBuiltin>();
engine.install::<StringBuiltin>();
engine.install::<JsonBuiltin>();
engine.install::<MathBuiltin>();
engine.install::<PromiseBuiltin>();
// 不安装 console/fetch/fs/net → JS 代码完全无法访问 I/O

// 确定性执行（可选）
engine.install::<MathBuiltin>().with_config(MathConfig {
    deterministic: true,  // Math.random() 使用固定种子
});
```

沙箱保证：
- 裸引擎无法执行任何 I/O（无 fetch、无 fs、无 net、无 console）
- 无法访问系统信息（无 Date.now、无 navigator、无 process）
- 可选禁止 eval()（阻止运行时代码生成）
- Fuel 限制防止无限循环和资源耗尽
- 内存限制防止 OOM

### 8.7 多上下文（Multi-Context）

单引擎多上下文——共享编译结果，隔离运行状态（借鉴 V8 Context / ES Realm）：

```rust
let engine = OneEngine::<()>::new(&config);

// 创建隔离的执行上下文
let mut ctx_a = engine.create_context();  // 独立 globalThis
let mut ctx_b = engine.create_context();  // 独立 globalThis

ctx_a.eval("var x = 1")?;
ctx_b.eval("typeof x === 'undefined'")?;  // true — 完全隔离

// 共享：编译后的 CodeBlock、Shape 树、内置对象原型
// 隔离：全局变量、模块缓存、宿主数据

// 每个 Context 可以有自己的宿主数据
struct TenantData { tenant_id: String, quota: u64 }
let ctx = engine.create_context_with_data(TenantData { ... });
```

适用场景：
- Sentinel 同时运行多个插件（共享引擎实例，每个插件一个 Context）
- 多租户 SaaS 平台（每个租户一个隔离的 JS 环境）
- 测试（每个测试用例一个干净的 Context）

### 8.8 引擎 Fork / Clone

初始化一次，按需克隆——服务端高并发场景的关键能力：

```rust
// 初始化模板引擎（加载公共代码、注册全局函数）
let template = OneEngine::<()>::new(&config);
template.eval("function handler(req) { return { status: 200 }; }")?;
template.eval("const utils = { sanitize(s) { ... } }")?;

// 创建快照（包含已编译代码 + 已初始化的全局状态）
let snapshot = template.snapshot()?;

// 每个请求从快照恢复（微秒级，远快于重新初始化）
let mut worker = OneEngine::restore(&snapshot)?;
let result = worker.call_function("handler", &request)?;
drop(worker);  // 丢弃，不影响 template

// 内存级 fork（比快照更快，共享不可变页面 + COW 可变页面）
let mut worker = template.fork();  // Copy-on-Write 语义
```

### 8.9 执行钩子与可观测性

嵌入者需要观测和拦截 JS 执行——安全审计、性能分析、调试：

```rust
engine.on_before_call(|ctx, func_name, args| -> HookAction {
    log::debug!("Calling {} with {} args", func_name, args.len());
    if func_name == "eval" { return HookAction::Block; }
    HookAction::Continue
});

engine.on_property_access(|obj_desc, prop_name, access| -> HookAction {
    audit_log.record(obj_desc, prop_name, access);
    HookAction::Continue
});

engine.on_gc(|event| {
    metrics.record("gc_pause_ms", event.pause_time.as_millis());
    metrics.record("gc_freed_bytes", event.freed_bytes);
});

engine.on_module_load(|specifier| -> HookAction {
    if !allowed_modules.contains(specifier) {
        return HookAction::Block;
    }
    HookAction::Continue
});

enum HookAction {
    Continue,                // 允许执行
    Block,                   // 阻止执行（抛出 JS 异常）
    Replace(JsValue),        // 替换返回值（用于 mock/stub）
}
```

### 8.10 线程安全模型

```rust
// 默认：OneEngine 是 !Send — 单线程使用，最佳性能（无锁开销）
let engine = OneEngine::<()>::new(&config);

// 跨线程场景：通过 Handle 提交工作
let handle = engine.handle();  // Handle: Send + Sync + Clone
std::thread::spawn(move || {
    // 通过 channel 提交 JS 执行请求
    let result = handle.call("process", &data).await;
});

// 多引擎并发：每个线程一个独立引擎实例（无共享状态）
let engines: Vec<_> = (0..num_cpus::get())
    .map(|_| OneEngine::restore(&snapshot).unwrap())
    .collect();
// 通过线程池分发请求到不同引擎
```

### 8.11 Serde 集成

复杂类型通过 Serde 自动桥接（适用于低频调用和复杂数据结构）：

```rust
#[derive(Serialize, Deserialize)]
struct PluginResult { success: bool, findings: Vec<Finding> }

engine.set_global("config", &my_config)?;
let result: PluginResult = engine.call_function("analyze", &input)?;
```

### 8.12 Feature Flags 按需裁剪

```toml
[features]
default = ["typescript", "regexp", "annex-b"]
typescript = []               # TS 类型擦除
regexp = ["fancy-regex"]      # RegExp
annex-b = []                  # Web 兼容性
fetch = ["reqwest"]           # fetch API
crypto = ["ring"]             # Web Crypto
snapshot = []                 # 堆快照 / fork
bytecode-cache = []           # 字节码缓存
fuel = []                     # Fuel 执行控制（默认只有 epoch）
hooks = []                    # 执行钩子
multi-context = []            # 多上下文支持
send = []                     # 使 OneEngine: Send（增加 Send bound）
serde = ["serde", "serde_json"]  # Serde 自动桥接
full = ["typescript", "regexp", "annex-b", "fetch", "crypto",
        "snapshot", "bytecode-cache", "fuel", "hooks",
        "multi-context", "serde"]
minimal = []                  # 纯引擎核心：eval + 基本类型
```

体积预估：

| 配置 | 预估体积 | 适用场景 |
|------|----------|----------|
| minimal | ~1.5 MB | 纯计算嵌入、wasm 环境 |
| default | ~3 MB | 通用嵌入 |
| full | ~6 MB | 功能完整的运行时 |

### 8.13 wasm32 编译目标

事件循环层通过 trait 抽象，wasm 目标使用不同实现：

```rust
trait EventLoopDriver {
    fn poll(&mut self, timeout: Option<Duration>) -> Vec<Event>;
    fn register_timer(&mut self, delay: Duration) -> TimerId;
}

struct MioDriver { /* 原生平台，基于 mio */ }
struct WasmDriver { /* wasm 平台，无 I/O */ }
```

wasm 约束：文件 I/O 和网络 API 在 wasm 下编译时排除；GC 使用 wasm `memory.grow`。

### 8.14 Sentinel 集成示例（完整）

展示 One 嵌入 API 如何替代当前 sentinel-plugins 的 Deno 运行时：

```rust
// 定义 Sentinel 插件宿主数据
struct SentinelPluginHost {
    plugin_id: String,
    category: PluginMainCategory,
    findings: Vec<Finding>,
    finding_sink: Option<Sender<Finding>>,
    run_id: Option<String>,
    runtime_settings: PluginRuntimeSettings,
}

// 创建插件引擎
let mut engine = OneEngine::<SentinelPluginHost>::builder()
    .host_data(host)
    .preset(Preset::Standard)          // 标准内置对象
    .enable_typescript(true)
    .fuel(100_000)                     // 执行限额
    .register_class::<SentinelApi>()   // 注册 Sentinel.* API
    .register_host_function("emitFinding", |store: &mut Store<SentinelPluginHost>, finding: Finding| {
        if let Some(sink) = &store.data().finding_sink {
            sink.send(finding.clone()).ok();
        }
        store.data_mut().findings.push(finding);
        Ok(())
    })
    .build();

// 加载并执行插件
engine.eval_module(plugin_code, &format!("sentinel://plugin_{}", plugin_id))?;

// 流量扫描
let findings: Vec<Finding> = engine.call_function("scan_transaction", &transaction)?;

// 引擎复用（重置状态但保留已编译代码）
engine.reset();
engine.set_fuel(100_000);
```

---

## 九、错误处理（面向嵌入者的结构化错误）

嵌入者需要精确区分错误类型并做出不同的恢复决策：

```rust
enum OneError {
    // === JS 层异常（可恢复：引擎状态仍然有效） ===
    JsException(JsException),       // JS throw 的异常

    // === 编译错误（可恢复：引擎状态不变） ===
    CompileError(CompileError),      // 语法错误 / 编译失败

    // === 资源限制（可恢复：补充资源后可继续） ===
    OutOfFuel { consumed: u64 },     // 燃料耗尽
    OutOfMemory {                    // 内存超限
        requested: usize,
        limit: usize,
    },
    StackOverflow { depth: usize },  // 栈溢出
    ExecutionTimeout {               // epoch 超时
        elapsed: Duration,
    },

    // === 引擎故障（不可恢复：需要 reset 或丢弃） ===
    InternalError(String),

    // === 钩子拦截 ===
    Blocked {                        // 执行被钩子阻止
        operation: String,
        reason: String,
    },
}

struct JsException {
    message: String,
    name: String,                    // "TypeError", "RangeError", etc.
    stack_trace: Vec<StackFrame>,
    value: JsValue,                  // 原始的 JS 异常对象
}

struct CompileError {
    message: String,
    file: Option<String>,
    line: u32,
    column: u32,
}

struct StackFrame {
    function_name: Option<String>,
    file_name: Option<String>,
    line: u32,
    column: u32,
}

impl OneError {
    /// 该错误后引擎是否仍可继续使用
    fn is_recoverable(&self) -> bool {
        !matches!(self, OneError::InternalError(_))
    }

    /// 该错误是否可以通过补充资源后恢复执行
    fn is_resumable(&self) -> bool {
        matches!(self, OneError::OutOfFuel { .. })
    }

    /// 获取 JS 层的调用栈（如果有）
    fn js_stack_trace(&self) -> Option<&[StackFrame]> {
        match self {
            OneError::JsException(e) => Some(&e.stack_trace),
            _ => None,
        }
    }
}

// 实现标准库 trait
impl std::fmt::Display for OneError { ... }
impl std::error::Error for OneError { ... }
impl From<OneError> for anyhow::Error { ... }  // anyhow 兼容
```

嵌入者的错误处理模式：

```rust
match engine.call_function("handler", &input) {
    Ok(result) => process(result),

    // JS 逻辑异常 — 记录日志，引擎可继续使用
    Err(OneError::JsException(e)) => {
        log::warn!("Plugin error: {} at {:?}", e.message, e.stack_trace);
    },

    // 燃料耗尽 — 补充后可恢复执行
    Err(OneError::OutOfFuel { consumed }) => {
        log::info!("Used {} fuel, adding more", consumed);
        engine.add_fuel(50_000);
        engine.resume()?;
    },

    // 内存超限 — 丢弃引擎
    Err(OneError::OutOfMemory { .. }) => {
        engine.reset();
    },

    // 不可恢复错误 — 必须重建引擎
    Err(e) if !e.is_recoverable() => {
        engine = OneEngine::restore(&snapshot)?;
    },

    Err(e) => return Err(e.into()),
}
```

---

## 十、测试策略

| 测试类型 | 工具/方法 | 覆盖范围 |
|----------|----------|----------|
| 规范合规 | Test262 测试套件 | ES 规范每个特性 |
| 单元测试 | Rust `#[test]` | 每个 crate 的内部逻辑 |
| 集成测试 | 自定义 JS 测试用例 | 端到端执行正确性 |
| 性能基准 | V8 Benchmark Suite + SunSpider + Octane | 与 QuickJS/Boa 对比 |
| GC 压力 | 专用压力测试 | 内存泄漏、碎片化、停顿时间 |
| 模糊测试 | cargo-fuzz + libfuzzer | 解析器/VM 健壮性 |
| Sentinel 集成测试 | 现有 sentinel-plugins 测试用例 | 插件兼容性 |

合规度追踪指标：
- Test262 通过率（按特性分组）
- Sentinel 现有插件通过率

---

## 十一、开发阶段

### Phase 1：基础设施（~8 周）
- one_core：JsValue (NaN-boxing)、InternId、字符串实习池、OneError
- one_parser：词法分析器 + AST 定义 + 解析器（表达式 + 语句 + 声明）
- Arena 分配器用于 AST 节点
- 惰性解析基础（函数签名解析 + 函数体预扫描）

### Phase 2：编译 + VM + GC（~9 周）
- one_compiler：字节码定义 + AST → 字节码编译器
- one_vm：寄存器 VM 基础 + 函数指针调度表
- one_gc：Nursery (bump allocation) + Minor GC (Scavenge) + derive(Trace) 宏
- **里程碑 M1**：能执行 `console.log("Hello World")`

### Phase 3：核心语言特性（~14 周）
- 闭包 / 作用域链 / 上值
- 类 / 原型链 / `new`
- 异常处理 (try/catch/finally)
- 解构 / 展开运算符
- 迭代器 / 生成器 / for-of
- Promise / async-await + 微任务队列
- GC 老年代（Mark-Compact + 写屏障 + 增量标记）
- Strict Mode / Sloppy Mode 差异处理
- eval() / new Function() 运行时编译

### Phase 4：内置对象（~8 周）
- Object / Array / String（双编码） / Number / Boolean / Symbol
- Function / Error 层次结构
- RegExp（regex crate 适配层）
- Map / Set / WeakMap / WeakSet / TypedArray / ArrayBuffer / DataView
- Proxy / Reflect（全部 13 个 trap）
- Math / JSON / Date
- 声明式宏批量注册内置方法

### Phase 5：运行时完善（~8 周）
- 完整事件循环（trait 抽象，支持原生 + wasm 驱动）
- 模块系统 (ESM 原生 + CJS 兼容层)
- TS 语法支持合入 one_parser
- 宿主 API (console/fetch/fs/net) + Op 宏系统
- Annex B Web 兼容特性
- CLI / REPL (one_cli)

### Phase 6：嵌入 API + Sentinel 集成（~8 周）
- one_engine 核心：Store\<T\> 宿主数据绑定 + Builder 模式 + Preset 预置配置
- 类型安全绑定：FromJs/IntoJs trait + JsClass 宏 + Serde 桥接
- 执行控制：Fuel 燃料系统 + Epoch 中断
- 安全沙箱：裸引擎模式 + 选择性内置对象安装
- 多上下文 + Fork/Clone + 堆快照
- 执行钩子 / 可观测性
- 结构化错误体系 + 恢复机制
- Feature flags 裁剪 + wasm32 编译验证
- sentinel-plugins 适配层
- **里程碑 M2**：替换 Deno Core 运行 Sentinel 插件

### Phase 7：优化与扩展（持续）
- 多态 IC / Shape 转换链优化
- 内联属性存储 + Slack tracking
- 字节码窥孔优化 + 特化指令
- 字节码缓存优化
- Cranelift JIT 原型
- WeakRef / FinalizationRegistry
- BigInt / Intl / SharedArrayBuffer
- wasm32 深度优化（体积、启动时间）
- 嵌入 API 人体工学持续改进（基于使用反馈）

**预估总工期**：~55 周（Phase 1-6），Phase 7 持续进行

---

## 关键设计决策总结

| 决策 | 选择 | 优化维度 | 理由 |
|------|------|----------|------|
| 语言 | 100% Rust | 性能/接入 | 内存安全、与 Sentinel 技术栈一致、零 C FFI 开销 |
| 引擎 | 完全自研 | 性能/兼容 | 最大控制权、深度定制能力 |
| 值表示 | NaN-boxing (u64) | 性能 | 极小内存开销、快速类型检查、避免装箱 |
| 字符串 | 双编码 Latin1 + UTF-16 | 性能/兼容 | O(1) 索引访问、完美符合 ES 规范 UTF-16 语义 |
| 对象模型 | Shape + 多态 IC + 内联属性 | 性能 | 属性访问 O(1)、减少间接寻址、覆盖多态场景 |
| 字节码 | 寄存器式 32 位定长 + 特化指令 | 性能 | 指令数少、解码快、热路径跳过类型检查 |
| VM 调度 | 函数指针表 | 性能 | 可预测的分支、优于 match 大表达式 |
| 解析策略 | 惰性解析 + Arena 分配 | 性能/代码量 | 极快启动、AST 零碎片释放 |
| GC | 分代 + WeakRef + derive(Trace) | 性能/代码量/兼容 | 短命对象快速回收、自动生成 Trace 代码、完整规范支持 |
| 事件循环 | trait 抽象 (mio/wasm) | 接入 | 跨平台、支持 wasm32 编译目标 |
| TS 支持 | 解析阶段集成擦除 | 代码量/性能 | 一次解析完成、零额外开销 |
| RegExp | regex crate 适配层 | 代码量 | 避免从零实现 RegExp 引擎的巨大工作量 |
| 嵌入 API | Store\<T\> + Builder + Preset + FromJs/IntoJs + JsClass | 接入/性能 | 三行代码可用、宿主完全掌控、零开销互操作 |
| 资源控制 | Fuel 燃料 + Epoch 中断 + 裸引擎沙箱 | 安全/接入 | 确定性限制、可恢复中断、宿主完全掌控 |
| 多上下文 | Engine → Context 分离 + Fork/Clone | 性能/接入 | 共享编译结果、微秒级实例化、高并发复用 |
| 错误体系 | 结构化 OneError（可恢复/可恢复执行/不可恢复） | 接入 | 嵌入者精确区分错误类型并决定恢复策略 |
| 可观测性 | 执行钩子 + HookAction（Continue/Block/Replace） | 安全/接入 | 安全审计、性能分析、mock/stub 能力 |
| 兼容性 | Annex B 部分实现 + eval 支持 | 兼容 | 覆盖实际使用的遗留特性 |
| Crate 组织 | 8 个 crate（从 13 个精简） | 代码量 | 减少跨 crate 样板、加速编译 |
