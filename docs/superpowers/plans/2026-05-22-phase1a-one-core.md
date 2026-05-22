# Phase 1a: 项目骨架 + one_core 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 初始化 One JS Runtime 的 Cargo workspace，实现 `one_core` crate 的全部核心类型——JsValue (NaN-boxing)、InternId + 字符串实习池、GcPtr 占位类型、OneError 错误体系。

**Architecture:** 创建 8 个 crate 的 workspace 骨架，`one_core` 作为所有 crate 的共享基础，提供零开销的值表示（NaN-boxing 编码到 u64）、字符串实习（哈希去重 + 整数 ID 索引）、GC 指针占位（Phase 2 替换为真实 GC）和结构化错误类型。

**Tech Stack:** Rust 2024 edition, Cargo workspace

---

## 文件结构

```
one/
├── Cargo.toml                        # workspace 根配置
├── crates/
│   ├── one_core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs                # 模块声明 + re-exports
│   │       ├── value.rs              # JsValue NaN-boxing
│   │       ├── intern.rs             # InternId + StringInterner
│   │       ├── error.rs              # OneError 类型层次
│   │       └── gc_ptr.rs             # GcPtr<T> 占位实现
│   ├── one_parser/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                # stub
│   ├── one_compiler/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                # stub
│   ├── one_gc/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                # stub
│   ├── one_vm/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                # stub
│   ├── one_runtime/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                # stub
│   ├── one_engine/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs                # stub
│   └── one_cli/
│       ├── Cargo.toml
│       └── src/main.rs               # stub
└── tests/                            # 集成测试目录（后续 Phase 使用）
```

---

### Task 1: 初始化 Cargo Workspace

**Files:**
- Create: `Cargo.toml`
- Create: `crates/one_core/Cargo.toml`, `crates/one_core/src/lib.rs`
- Create: `crates/one_parser/Cargo.toml`, `crates/one_parser/src/lib.rs`
- Create: `crates/one_compiler/Cargo.toml`, `crates/one_compiler/src/lib.rs`
- Create: `crates/one_gc/Cargo.toml`, `crates/one_gc/src/lib.rs`
- Create: `crates/one_vm/Cargo.toml`, `crates/one_vm/src/lib.rs`
- Create: `crates/one_runtime/Cargo.toml`, `crates/one_runtime/src/lib.rs`
- Create: `crates/one_engine/Cargo.toml`, `crates/one_engine/src/lib.rs`
- Create: `crates/one_cli/Cargo.toml`, `crates/one_cli/src/main.rs`

- [ ] **Step 1: 创建 workspace 根 Cargo.toml**

```toml
# one/Cargo.toml
[workspace]
resolver = "2"
members = [
    "crates/one_core",
    "crates/one_parser",
    "crates/one_compiler",
    "crates/one_gc",
    "crates/one_vm",
    "crates/one_runtime",
    "crates/one_engine",
    "crates/one_cli",
]

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "MIT OR Apache-2.0"
repository = "https://github.com/user/one"

[workspace.dependencies]
one_core = { path = "crates/one_core" }
one_parser = { path = "crates/one_parser" }
one_compiler = { path = "crates/one_compiler" }
one_gc = { path = "crates/one_gc" }
one_vm = { path = "crates/one_vm" }
one_runtime = { path = "crates/one_runtime" }
one_engine = { path = "crates/one_engine" }
```

- [ ] **Step 2: 创建 one_core crate**

```toml
# crates/one_core/Cargo.toml
[package]
name = "one_core"
version = "0.1.0"
edition.workspace = true

[dependencies]
```

```rust
// crates/one_core/src/lib.rs
```

- [ ] **Step 3: 创建其余 7 个 crate stubs**

每个 crate 的 Cargo.toml 同 one_core 格式（仅改 name）。`one_cli` 使用 `src/main.rs` 而非 `src/lib.rs`：

```rust
// crates/one_cli/src/main.rs
fn main() {
    println!("one - JavaScript Runtime");
}
```

其余 crate 的 `src/lib.rs` 为空文件。

- [ ] **Step 4: 验证构建**

Run: `cd /Users/like/code/one && cargo build`
Expected: 编译成功，无错误

- [ ] **Step 5: 提交**

```bash
git init
git add .
git commit -m "chore: initialize cargo workspace with 8 crate stubs"
```

---

### Task 2: JsValue NaN-boxing — f64 编码

**Files:**
- Create: `crates/one_core/src/value.rs`
- Modify: `crates/one_core/src/lib.rs`

- [ ] **Step 1: 编写 f64 编码的测试**

```rust
// crates/one_core/src/value.rs 底部
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_round_trip() {
        let cases = [0.0, -0.0, 1.0, -1.0, 3.14, f64::INFINITY, f64::NEG_INFINITY,
                      f64::MAX, f64::MIN, f64::MIN_POSITIVE, f64::EPSILON];
        for v in cases {
            let js = JsValue::from_f64(v);
            assert!(js.is_number(), "should be number: {v}");
            assert!(js.is_float64(), "should be float64: {v}");
            assert_eq!(js.as_f64().unwrap().to_bits(), v.to_bits(), "round trip failed: {v}");
        }
    }

    #[test]
    fn nan_is_canonicalized() {
        let nan1 = JsValue::from_f64(f64::NAN);
        let nan2 = JsValue::from_f64(-f64::NAN);
        assert!(nan1.is_number());
        assert!(nan1.as_f64().unwrap().is_nan());
        assert_eq!(nan1.0, nan2.0, "all NaN values should canonicalize to same bits");
    }

    #[test]
    fn negative_zero_preserved() {
        let nz = JsValue::from_f64(-0.0);
        let pz = JsValue::from_f64(0.0);
        assert!(nz.as_f64().unwrap().is_sign_negative());
        assert!(pz.as_f64().unwrap().is_sign_positive());
        assert_ne!(nz.0, pz.0, "-0 and +0 have different bit patterns");
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 编译失败 — `JsValue` 未定义

- [ ] **Step 3: 实现 JsValue f64 编码**

```rust
// crates/one_core/src/value.rs

/// NaN-boxing 值表示
///
/// 布局：
/// - 若 (bits >> 48) < 0xFFF8 → f64（原始 IEEE 754 位模式）
/// - 若 (bits >> 48) >= 0xFFF8 → 带标签的非 f64 值
///
/// 标签分配（高 16 位）：
///   0xFFF9 = undefined
///   0xFFFA = null
///   0xFFFB = boolean (bit 0 = true/false)
///   0xFFFC = i32 (低 32 位)
///   0xFFFD = symbol
///   0xFFFE = string pointer (低 48 位)
///   0xFFFF = object pointer (低 48 位)
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct JsValue(u64);

const CANON_NAN_BITS: u64 = 0x7FF8_0000_0000_0000;
const TAG_THRESHOLD: u64 = 0xFFF8;

const TAG_UNDEFINED: u64 = 0xFFF9_0000_0000_0000;
const TAG_NULL: u64      = 0xFFFA_0000_0000_0000;
const TAG_BOOL: u64      = 0xFFFB_0000_0000_0000;
const TAG_INT32: u64     = 0xFFFC_0000_0000_0000;
const TAG_SYMBOL: u64    = 0xFFFD_0000_0000_0000;
const TAG_STRING: u64    = 0xFFFE_0000_0000_0000;
const TAG_OBJECT: u64    = 0xFFFF_0000_0000_0000;

const TAG_MASK: u64 = 0xFFFF_0000_0000_0000;
const PTR_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

impl JsValue {
    /// 从 f64 创建。NaN 值被规范化为唯一的 canonical NaN。
    #[inline]
    pub fn from_f64(v: f64) -> Self {
        if v.is_nan() {
            JsValue(CANON_NAN_BITS)
        } else {
            JsValue(v.to_bits())
        }
    }

    /// 是否为 f64（含 NaN/Inf/±0）
    #[inline]
    pub fn is_float64(&self) -> bool {
        (self.0 >> 48) < TAG_THRESHOLD
    }

    /// 是否为数字类型（f64 或 i32）
    #[inline]
    pub fn is_number(&self) -> bool {
        self.is_float64() || self.is_int32()
    }

    /// 是否为 i32（此处尚未实现，先返回 false）
    #[inline]
    pub fn is_int32(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_INT32
    }

    /// 尝试提取 f64 值。
    #[inline]
    pub fn as_f64(&self) -> Option<f64> {
        if self.is_float64() {
            Some(f64::from_bits(self.0))
        } else {
            None
        }
    }
}
```

- [ ] **Step 4: 在 lib.rs 中声明模块**

```rust
// crates/one_core/src/lib.rs
pub mod value;

pub use value::JsValue;
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 3 tests passed

- [ ] **Step 6: 提交**

```bash
git add -A && git commit -m "feat(core): implement JsValue NaN-boxing with f64 encoding"
```

---

### Task 3: JsValue — i32、boolean、null、undefined 编码

**Files:**
- Modify: `crates/one_core/src/value.rs`

- [ ] **Step 1: 编写 i32/bool/null/undefined 测试**

```rust
// 添加到 value.rs 的 tests 模块中

#[test]
fn i32_round_trip() {
    let cases = [0, 1, -1, 42, i32::MAX, i32::MIN, 1000000];
    for v in cases {
        let js = JsValue::from_i32(v);
        assert!(js.is_int32(), "should be int32: {v}");
        assert!(js.is_number(), "should be number: {v}");
        assert!(!js.is_float64(), "should not be float64: {v}");
        assert_eq!(js.as_i32().unwrap(), v, "round trip failed: {v}");
    }
}

#[test]
fn i32_to_f64_conversion() {
    let js = JsValue::from_i32(42);
    assert_eq!(js.to_number(), 42.0);
}

#[test]
fn boolean_encoding() {
    let t = JsValue::from_bool(true);
    let f = JsValue::from_bool(false);
    assert!(t.is_boolean());
    assert!(f.is_boolean());
    assert_eq!(t.as_bool().unwrap(), true);
    assert_eq!(f.as_bool().unwrap(), false);
    assert!(!t.is_number());
    assert!(!t.is_null());
    assert_ne!(t.0, f.0);
}

#[test]
fn null_and_undefined() {
    let n = JsValue::null();
    let u = JsValue::undefined();
    assert!(n.is_null());
    assert!(u.is_undefined());
    assert!(n.is_nullish());
    assert!(u.is_nullish());
    assert!(!n.is_undefined());
    assert!(!u.is_null());
    assert!(!n.is_number());
    assert!(!u.is_boolean());
    assert_ne!(n.0, u.0);
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 编译失败 — `from_i32`, `from_bool`, `null`, `undefined` 等方法未定义

- [ ] **Step 3: 实现 i32/bool/null/undefined 编码**

```rust
// 添加到 value.rs 的 impl JsValue 块中

    #[inline]
    pub fn from_i32(v: i32) -> Self {
        JsValue(TAG_INT32 | (v as u32 as u64))
    }

    #[inline]
    pub fn as_i32(&self) -> Option<i32> {
        if self.is_int32() {
            Some(self.0 as u32 as i32)
        } else {
            None
        }
    }

    #[inline]
    pub fn from_bool(v: bool) -> Self {
        JsValue(TAG_BOOL | (v as u64))
    }

    #[inline]
    pub fn is_boolean(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_BOOL
    }

    #[inline]
    pub fn as_bool(&self) -> Option<bool> {
        if self.is_boolean() {
            Some((self.0 & 1) != 0)
        } else {
            None
        }
    }

    #[inline]
    pub const fn null() -> Self {
        JsValue(TAG_NULL)
    }

    #[inline]
    pub const fn undefined() -> Self {
        JsValue(TAG_UNDEFINED)
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0 == TAG_NULL
    }

    #[inline]
    pub fn is_undefined(&self) -> bool {
        self.0 == TAG_UNDEFINED
    }

    #[inline]
    pub fn is_nullish(&self) -> bool {
        self.is_null() || self.is_undefined()
    }

    /// 转换为 f64 数字（i32 会转为 f64）
    #[inline]
    pub fn to_number(&self) -> f64 {
        if self.is_float64() {
            f64::from_bits(self.0)
        } else if self.is_int32() {
            (self.0 as u32 as i32) as f64
        } else {
            f64::NAN
        }
    }
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 全部通过（前 3 个 + 新增 4 个 = 7 tests）

- [ ] **Step 5: 提交**

```bash
git add -A && git commit -m "feat(core): add i32/bool/null/undefined encoding to JsValue"
```

---

### Task 4: JsValue — 指针类型 + type_of + Display/Debug

**Files:**
- Modify: `crates/one_core/src/value.rs`

- [ ] **Step 1: 编写指针类型和 type_of 测试**

```rust
// 添加到 value.rs 的 tests 模块中

#[test]
fn object_pointer_round_trip() {
    let fake_ptr: u64 = 0x0000_1234_5678_9AB0;
    let js = JsValue::from_object_raw(fake_ptr);
    assert!(js.is_object());
    assert!(!js.is_string());
    assert!(!js.is_number());
    assert_eq!(js.as_object_raw().unwrap(), fake_ptr);
}

#[test]
fn string_pointer_round_trip() {
    let fake_ptr: u64 = 0x0000_ABCD_EF01_2340;
    let js = JsValue::from_string_raw(fake_ptr);
    assert!(js.is_string());
    assert!(!js.is_object());
    assert_eq!(js.as_string_raw().unwrap(), fake_ptr);
}

#[test]
fn type_of_all_types() {
    assert_eq!(JsValue::from_f64(1.0).type_of(), "number");
    assert_eq!(JsValue::from_i32(1).type_of(), "number");
    assert_eq!(JsValue::from_bool(true).type_of(), "boolean");
    assert_eq!(JsValue::null().type_of(), "object");
    assert_eq!(JsValue::undefined().type_of(), "undefined");
    assert_eq!(JsValue::from_string_raw(0x1000).type_of(), "string");
    assert_eq!(JsValue::from_object_raw(0x2000).type_of(), "object");
    assert_eq!(JsValue::from_symbol_raw(42).type_of(), "symbol");
}

#[test]
fn display_formatting() {
    assert_eq!(format!("{}", JsValue::from_f64(3.14)), "3.14");
    assert_eq!(format!("{}", JsValue::from_i32(42)), "42");
    assert_eq!(format!("{}", JsValue::from_bool(true)), "true");
    assert_eq!(format!("{}", JsValue::null()), "null");
    assert_eq!(format!("{}", JsValue::undefined()), "undefined");
}

#[test]
fn all_types_are_distinct() {
    let values = [
        JsValue::from_f64(0.0),
        JsValue::from_i32(0),
        JsValue::from_bool(false),
        JsValue::null(),
        JsValue::undefined(),
    ];
    for (i, a) in values.iter().enumerate() {
        for (j, b) in values.iter().enumerate() {
            if i != j {
                assert_ne!(a.0, b.0, "type {i} and {j} should have different bit patterns");
            }
        }
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 编译失败

- [ ] **Step 3: 实现指针类型 + type_of + Display/Debug**

```rust
// 添加到 value.rs 的 impl JsValue 块中

    #[inline]
    pub fn from_object_raw(ptr: u64) -> Self {
        debug_assert!(ptr & !PTR_MASK == 0, "pointer exceeds 48 bits");
        JsValue(TAG_OBJECT | (ptr & PTR_MASK))
    }

    #[inline]
    pub fn is_object(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_OBJECT
    }

    #[inline]
    pub fn as_object_raw(&self) -> Option<u64> {
        if self.is_object() { Some(self.0 & PTR_MASK) } else { None }
    }

    #[inline]
    pub fn from_string_raw(ptr: u64) -> Self {
        debug_assert!(ptr & !PTR_MASK == 0, "pointer exceeds 48 bits");
        JsValue(TAG_STRING | (ptr & PTR_MASK))
    }

    #[inline]
    pub fn is_string(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_STRING
    }

    #[inline]
    pub fn as_string_raw(&self) -> Option<u64> {
        if self.is_string() { Some(self.0 & PTR_MASK) } else { None }
    }

    #[inline]
    pub fn from_symbol_raw(id: u32) -> Self {
        JsValue(TAG_SYMBOL | (id as u64))
    }

    #[inline]
    pub fn is_symbol(&self) -> bool {
        (self.0 & TAG_MASK) == TAG_SYMBOL
    }

    #[inline]
    pub fn as_symbol_raw(&self) -> Option<u32> {
        if self.is_symbol() { Some(self.0 as u32) } else { None }
    }

    /// ES `typeof` 运算符
    #[inline]
    pub fn type_of(&self) -> &'static str {
        if self.is_float64() || self.is_int32() {
            "number"
        } else if self.is_boolean() {
            "boolean"
        } else if self.is_string() {
            "string"
        } else if self.is_symbol() {
            "symbol"
        } else if self.is_undefined() {
            "undefined"
        } else {
            "object"
        }
    }

    /// 内部标签（用于调试）
    fn tag(&self) -> u16 {
        (self.0 >> 48) as u16
    }
```

```rust
// 在 value.rs 中添加 trait 实现

impl std::fmt::Display for JsValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_float64() {
            let v = f64::from_bits(self.0);
            if v.is_nan() {
                write!(f, "NaN")
            } else if v == f64::INFINITY {
                write!(f, "Infinity")
            } else if v == f64::NEG_INFINITY {
                write!(f, "-Infinity")
            } else {
                write!(f, "{v}")
            }
        } else if self.is_int32() {
            write!(f, "{}", self.0 as u32 as i32)
        } else if self.is_boolean() {
            write!(f, "{}", if (self.0 & 1) != 0 { "true" } else { "false" })
        } else if self.is_null() {
            write!(f, "null")
        } else if self.is_undefined() {
            write!(f, "undefined")
        } else if self.is_string() {
            write!(f, "[string@{:#x}]", self.0 & PTR_MASK)
        } else if self.is_object() {
            write!(f, "[object@{:#x}]", self.0 & PTR_MASK)
        } else if self.is_symbol() {
            write!(f, "Symbol({})", self.0 as u32)
        } else {
            write!(f, "[unknown:{:#018x}]", self.0)
        }
    }
}

impl std::fmt::Debug for JsValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsValue({} tag={:#06x} bits={:#018x})", self, self.tag(), self.0)
    }
}

impl PartialEq for JsValue {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for JsValue {}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 全部通过（12 tests）

- [ ] **Step 5: 提交**

```bash
git add -A && git commit -m "feat(core): add pointer types, type_of, Display/Debug to JsValue"
```

---

### Task 5: InternId + StringInterner

**Files:**
- Create: `crates/one_core/src/intern.rs`
- Modify: `crates/one_core/src/lib.rs`

- [ ] **Step 1: 编写 StringInterner 测试**

```rust
// crates/one_core/src/intern.rs 底部
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_returns_same_id() {
        let mut interner = StringInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("hello");
        assert_eq!(a, b);
    }

    #[test]
    fn different_strings_get_different_ids() {
        let mut interner = StringInterner::new();
        let a = interner.intern("hello");
        let b = interner.intern("world");
        assert_ne!(a, b);
    }

    #[test]
    fn resolve_returns_original_string() {
        let mut interner = StringInterner::new();
        let id = interner.intern("hello");
        assert_eq!(interner.resolve(id), Some("hello"));
    }

    #[test]
    fn resolve_unknown_returns_none() {
        let interner = StringInterner::new();
        assert_eq!(interner.resolve(InternId(9999)), None);
    }

    #[test]
    fn well_known_strings_pre_interned() {
        let interner = StringInterner::with_well_known();
        assert!(interner.resolve(WELL_KNOWN_UNDEFINED).is_some());
        assert_eq!(interner.resolve(WELL_KNOWN_UNDEFINED), Some("undefined"));
        assert_eq!(interner.resolve(WELL_KNOWN_NULL), Some("null"));
        assert_eq!(interner.resolve(WELL_KNOWN_TRUE), Some("true"));
        assert_eq!(interner.resolve(WELL_KNOWN_FALSE), Some("false"));
        assert_eq!(interner.resolve(WELL_KNOWN_LENGTH), Some("length"));
        assert_eq!(interner.resolve(WELL_KNOWN_PROTOTYPE), Some("prototype"));
        assert_eq!(interner.resolve(WELL_KNOWN_CONSTRUCTOR), Some("constructor"));
    }

    #[test]
    fn intern_returns_well_known_id_for_known_strings() {
        let mut interner = StringInterner::with_well_known();
        let id = interner.intern("length");
        assert_eq!(id, WELL_KNOWN_LENGTH);
    }

    #[test]
    fn many_strings() {
        let mut interner = StringInterner::new();
        let ids: Vec<_> = (0..1000).map(|i| interner.intern(&format!("str_{i}"))).collect();
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(interner.resolve(*id), Some(format!("str_{i}").as_str()));
        }
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 编译失败

- [ ] **Step 3: 实现 InternId + StringInterner**

```rust
// crates/one_core/src/intern.rs

use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct InternId(pub u32);

pub struct StringInterner {
    map: HashMap<String, InternId>,
    strings: Vec<String>,
}

pub const WELL_KNOWN_UNDEFINED: InternId   = InternId(0);
pub const WELL_KNOWN_NULL: InternId        = InternId(1);
pub const WELL_KNOWN_TRUE: InternId        = InternId(2);
pub const WELL_KNOWN_FALSE: InternId       = InternId(3);
pub const WELL_KNOWN_LENGTH: InternId      = InternId(4);
pub const WELL_KNOWN_PROTOTYPE: InternId   = InternId(5);
pub const WELL_KNOWN_CONSTRUCTOR: InternId = InternId(6);
pub const WELL_KNOWN___PROTO__: InternId   = InternId(7);
pub const WELL_KNOWN_TO_STRING: InternId   = InternId(8);
pub const WELL_KNOWN_VALUE_OF: InternId    = InternId(9);
pub const WELL_KNOWN_HAS_INSTANCE: InternId = InternId(10);
pub const WELL_KNOWN_ITERATOR: InternId    = InternId(11);

const WELL_KNOWN_STRINGS: &[&str] = &[
    "undefined", "null", "true", "false",
    "length", "prototype", "constructor", "__proto__",
    "toString", "valueOf", "hasInstance", "iterator",
];

impl StringInterner {
    pub fn new() -> Self {
        StringInterner {
            map: HashMap::new(),
            strings: Vec::new(),
        }
    }

    pub fn with_well_known() -> Self {
        let mut interner = Self::new();
        for s in WELL_KNOWN_STRINGS {
            interner.intern(s);
        }
        interner
    }

    pub fn intern(&mut self, s: &str) -> InternId {
        if let Some(&id) = self.map.get(s) {
            return id;
        }
        let id = InternId(self.strings.len() as u32);
        self.strings.push(s.to_owned());
        self.map.insert(s.to_owned(), id);
        id
    }

    pub fn resolve(&self, id: InternId) -> Option<&str> {
        self.strings.get(id.0 as usize).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

impl Default for StringInterner {
    fn default() -> Self {
        Self::with_well_known()
    }
}
```

- [ ] **Step 4: 在 lib.rs 中导出**

```rust
// crates/one_core/src/lib.rs
pub mod value;
pub mod intern;

pub use value::JsValue;
pub use intern::{InternId, StringInterner};
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 全部通过

- [ ] **Step 6: 提交**

```bash
git add -A && git commit -m "feat(core): add InternId and StringInterner with well-known strings"
```

---

### Task 6: OneError 错误类型

**Files:**
- Create: `crates/one_core/src/error.rs`
- Modify: `crates/one_core/src/lib.rs`

- [ ] **Step 1: 编写错误类型测试**

```rust
// crates/one_core/src/error.rs 底部
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_exception_is_recoverable() {
        let err = OneError::js_exception("TypeError", "x is not a function");
        assert!(err.is_recoverable());
        assert!(!err.is_resumable());
    }

    #[test]
    fn out_of_fuel_is_resumable() {
        let err = OneError::OutOfFuel { consumed: 1000 };
        assert!(err.is_recoverable());
        assert!(err.is_resumable());
    }

    #[test]
    fn internal_error_is_not_recoverable() {
        let err = OneError::InternalError("gc panic".into());
        assert!(!err.is_recoverable());
        assert!(!err.is_resumable());
    }

    #[test]
    fn compile_error_display() {
        let err = OneError::CompileError(CompileError {
            message: "Unexpected token".into(),
            file: Some("test.js".into()),
            line: 10,
            column: 5,
        });
        let msg = format!("{err}");
        assert!(msg.contains("Unexpected token"));
        assert!(msg.contains("test.js"));
    }

    #[test]
    fn js_exception_stack_trace() {
        let err = OneError::JsException(JsException {
            name: "TypeError".into(),
            message: "x is not a function".into(),
            stack_trace: vec![
                StackFrame { function_name: Some("foo".into()), file_name: Some("a.js".into()), line: 1, column: 10 },
            ],
        });
        let trace = err.js_stack_trace().unwrap();
        assert_eq!(trace.len(), 1);
        assert_eq!(trace[0].function_name.as_deref(), Some("foo"));
    }

    #[test]
    fn error_implements_std_error() {
        let err = OneError::js_exception("Error", "something failed");
        let std_err: &dyn std::error::Error = &err;
        assert!(std_err.to_string().contains("something failed"));
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 编译失败

- [ ] **Step 3: 实现 OneError**

```rust
// crates/one_core/src/error.rs

use std::fmt;

#[derive(Debug)]
pub enum OneError {
    JsException(JsException),
    CompileError(CompileError),
    OutOfFuel { consumed: u64 },
    OutOfMemory { requested: usize, limit: usize },
    StackOverflow { depth: usize },
    ExecutionTimeout { elapsed_ms: u64 },
    InternalError(String),
    Blocked { operation: String, reason: String },
}

#[derive(Debug, Clone)]
pub struct JsException {
    pub name: String,
    pub message: String,
    pub stack_trace: Vec<StackFrame>,
}

#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
    pub file: Option<String>,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function_name: Option<String>,
    pub file_name: Option<String>,
    pub line: u32,
    pub column: u32,
}

impl OneError {
    pub fn js_exception(name: &str, message: &str) -> Self {
        OneError::JsException(JsException {
            name: name.to_owned(),
            message: message.to_owned(),
            stack_trace: Vec::new(),
        })
    }

    pub fn is_recoverable(&self) -> bool {
        !matches!(self, OneError::InternalError(_))
    }

    pub fn is_resumable(&self) -> bool {
        matches!(self, OneError::OutOfFuel { .. })
    }

    pub fn js_stack_trace(&self) -> Option<&[StackFrame]> {
        match self {
            OneError::JsException(e) => Some(&e.stack_trace),
            _ => None,
        }
    }
}

impl fmt::Display for OneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OneError::JsException(e) => write!(f, "{}: {}", e.name, e.message),
            OneError::CompileError(e) => {
                if let Some(file) = &e.file {
                    write!(f, "{}:{}:{}: {}", file, e.line, e.column, e.message)
                } else {
                    write!(f, "{}:{}: {}", e.line, e.column, e.message)
                }
            },
            OneError::OutOfFuel { consumed } => write!(f, "out of fuel after {consumed} units"),
            OneError::OutOfMemory { requested, limit } =>
                write!(f, "out of memory: requested {requested} bytes, limit {limit}"),
            OneError::StackOverflow { depth } => write!(f, "stack overflow at depth {depth}"),
            OneError::ExecutionTimeout { elapsed_ms } =>
                write!(f, "execution timeout after {elapsed_ms}ms"),
            OneError::InternalError(msg) => write!(f, "internal error: {msg}"),
            OneError::Blocked { operation, reason } =>
                write!(f, "blocked: {operation} — {reason}"),
        }
    }
}

impl std::error::Error for OneError {}

pub type OneResult<T> = Result<T, OneError>;
```

- [ ] **Step 4: 在 lib.rs 中导出**

```rust
// crates/one_core/src/lib.rs
pub mod value;
pub mod intern;
pub mod error;

pub use value::JsValue;
pub use intern::{InternId, StringInterner};
pub use error::{OneError, OneResult, CompileError, JsException, StackFrame};
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 全部通过

- [ ] **Step 6: 提交**

```bash
git add -A && git commit -m "feat(core): add OneError structured error hierarchy"
```

---

### Task 7: GcPtr 占位类型

**Files:**
- Create: `crates/one_core/src/gc_ptr.rs`
- Modify: `crates/one_core/src/lib.rs`

- [ ] **Step 1: 编写 GcPtr 测试**

```rust
// crates/one_core/src/gc_ptr.rs 底部
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gc_ptr_alloc_and_deref() {
        let ptr = GcPtr::new(42u64);
        assert_eq!(*ptr, 42);
    }

    #[test]
    fn gc_ptr_deref_mut() {
        let mut ptr = GcPtr::new(10i32);
        *ptr = 20;
        assert_eq!(*ptr, 20);
    }

    #[test]
    fn gc_ptr_as_raw_round_trip() {
        let ptr = GcPtr::new(String::from("hello"));
        let raw = ptr.as_raw();
        assert_ne!(raw, 0);
        let recovered = unsafe { GcPtr::<String>::from_raw(raw) };
        assert_eq!(*recovered, "hello");
        // 防止 double free
        std::mem::forget(recovered);
    }

    #[test]
    fn gc_ptr_clone_is_independent() {
        let a = GcPtr::new(100u32);
        let b = a.clone();
        assert_eq!(*a, *b);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 编译失败

- [ ] **Step 3: 实现 GcPtr 占位**

Phase 2 会用真正的 GC 管理指针替换。此处用 `Box` 模拟，保持 API 一致：

```rust
// crates/one_core/src/gc_ptr.rs

use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;
use std::fmt;

/// GC 管理的指针占位类型。
///
/// Phase 2 中将被真正的分代 GC 托管指针替换。
/// 当前实现：基于 Box 的简单堆分配。
pub struct GcPtr<T> {
    ptr: NonNull<T>,
}

impl<T> GcPtr<T> {
    pub fn new(value: T) -> Self {
        let boxed = Box::new(value);
        GcPtr {
            ptr: unsafe { NonNull::new_unchecked(Box::into_raw(boxed)) },
        }
    }

    pub fn as_raw(&self) -> u64 {
        self.ptr.as_ptr() as u64
    }

    /// # Safety
    /// `raw` 必须来自 `as_raw()`，且对应的 GcPtr 仍然有效
    pub unsafe fn from_raw(raw: u64) -> Self {
        GcPtr {
            ptr: NonNull::new_unchecked(raw as *mut T),
        }
    }
}

impl<T> Deref for GcPtr<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T> DerefMut for GcPtr<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T> Clone for GcPtr<T> {
    fn clone(&self) -> Self {
        // 占位实现：共享同一块内存（Phase 2 中由 GC 管理引用计数/追踪）
        GcPtr { ptr: self.ptr }
    }
}

impl<T> Drop for GcPtr<T> {
    fn drop(&mut self) {
        // 占位：不释放内存（模拟 GC 语义 — 由 GC 统一回收）
        // Phase 2 替换为真正的 GC 跟踪
    }
}

impl<T: fmt::Debug> fmt::Debug for GcPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GcPtr({:?})", self.deref())
    }
}
```

- [ ] **Step 4: 在 lib.rs 中导出**

```rust
// crates/one_core/src/lib.rs
pub mod value;
pub mod intern;
pub mod error;
pub mod gc_ptr;

pub use value::JsValue;
pub use intern::{InternId, StringInterner};
pub use error::{OneError, OneResult, CompileError, JsException, StackFrame};
pub use gc_ptr::GcPtr;
```

- [ ] **Step 5: 运行测试确认通过**

Run: `cd /Users/like/code/one && cargo test -p one_core`
Expected: 全部通过

- [ ] **Step 6: 运行全 workspace 测试 + clippy**

Run: `cd /Users/like/code/one && cargo test && cargo clippy -- -D warnings`
Expected: 全部通过，无 clippy 警告

- [ ] **Step 7: 提交**

```bash
git add -A && git commit -m "feat(core): add GcPtr placeholder type for future GC integration"
```

---

## Phase 1a 完成标准

全部通过后，`one_core` crate 提供：
- ✅ `JsValue` NaN-boxing：f64、i32、bool、null、undefined、object/string/symbol 指针
- ✅ `InternId` + `StringInterner`：字符串实习 + 常用字符串预注册
- ✅ `OneError`：结构化错误类型 + 可恢复性判断
- ✅ `GcPtr<T>`：占位 GC 指针，API 契约稳定

下一步：Phase 1b — 词法分析器 (Lexer + Token types)
