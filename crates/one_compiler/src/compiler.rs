use std::collections::{HashMap, HashSet};

use one_parser::ast::*;

use crate::codeblock::{CodeBlock, Constant};
use crate::opcode::{Instruction, Opcode};

pub struct Compiler {
    code: CodeBlock,
    next_register: u8,
    locals: HashMap<String, u8>,
    mirrored_globals: HashSet<String>,
    is_top_level: bool,
}

impl Compiler {
    pub fn new(name: String) -> Self {
        Compiler {
            code: CodeBlock::new(name),
            next_register: 0,
            locals: HashMap::new(),
            mirrored_globals: HashSet::new(),
            is_top_level: true,
        }
    }

    fn child(name: String) -> Self {
        let mut compiler = Compiler::new(name);
        compiler.is_top_level = false;
        compiler
    }

    pub fn compile(program: &Program) -> CodeBlock {
        let mut compiler = Compiler::new("<script>".into());
        let (start, is_strict) = detect_use_strict(&program.body);
        compiler.code.is_strict = is_strict;
        for stmt in &program.body[start..] {
            compiler.compile_statement(stmt);
        }
        compiler.code.emit(Instruction::op_only(Opcode::ReturnUndef));
        compiler.code
    }

    /// Compile code for eval() — returns the completion value of the last expression.
    pub fn compile_eval(program: &Program) -> CodeBlock {
        let mut compiler = Compiler::new("<eval>".into());
        let (start, is_strict) = detect_use_strict(&program.body);
        compiler.code.is_strict = is_strict;
        let body = &program.body[start..];
        let len = body.len();
        for (i, stmt) in body.iter().enumerate() {
            if i + 1 == len
                && let StatementKind::ExpressionStatement(expr) = &stmt.kind
            {
                let reg = compiler.compile_expression(expr);
                compiler.code.emit(Instruction::abx(Opcode::Return, reg, 0));
                return compiler.code;
            }
            compiler.compile_statement(stmt);
        }
        compiler.code.emit(Instruction::op_only(Opcode::ReturnUndef));
        compiler.code
    }

    fn alloc_reg(&mut self) -> u8 {
        let r = self.next_register;
        self.next_register += 1;
        if self.next_register as u16 > self.code.register_count {
            self.code.register_count = self.next_register as u16;
        }
        r
    }

    fn free_reg(&mut self) {
        if self.next_register > 0 {
            self.next_register -= 1;
        }
    }

    fn add_string(&mut self, s: &str) -> u16 {
        self.code
            .add_constant(Constant::String(s.to_string()))
    }

    fn compile_statement(&mut self, stmt: &Statement) {
        match &stmt.kind {
            StatementKind::ExpressionStatement(expr) => {
                let _reg = self.compile_expression(expr);
                self.free_reg();
            }
            StatementKind::BlockStatement(stmts) => {
                for s in stmts {
                    self.compile_statement(s);
                }
            }
            StatementKind::EmptyStatement => {}
            StatementKind::IfStatement {
                test,
                consequent,
                alternate,
            } => {
                let test_reg = self.compile_expression(test);
                let jump_false = self
                    .code
                    .emit(Instruction::asbx(Opcode::JumpIfFalse, test_reg, 0));
                self.free_reg();

                self.compile_statement(consequent);

                if let Some(alt) = alternate {
                    let jump_end = self.code.emit(Instruction::asbx(Opcode::Jump, 0, 0));
                    let else_start = self.code.current_offset();
                    self.code.patch_jump(jump_false, else_start);
                    self.compile_statement(alt);
                    let end = self.code.current_offset();
                    self.code.patch_jump(jump_end, end);
                } else {
                    let end = self.code.current_offset();
                    self.code.patch_jump(jump_false, end);
                }
            }
            StatementKind::WhileStatement { test, body } => {
                let loop_start = self.code.current_offset();
                let test_reg = self.compile_expression(test);
                let jump_false = self
                    .code
                    .emit(Instruction::asbx(Opcode::JumpIfFalse, test_reg, 0));
                self.free_reg();

                self.compile_statement(body);

                let jump_back = self.code.emit(Instruction::asbx(Opcode::Jump, 0, 0));
                self.code.patch_jump(jump_back, loop_start);

                let end = self.code.current_offset();
                self.code.patch_jump(jump_false, end);
            }
            StatementKind::DoWhileStatement { test, body } => {
                let loop_start = self.code.current_offset();
                self.compile_statement(body);
                let test_reg = self.compile_expression(test);
                let jump_back = self
                    .code
                    .emit(Instruction::asbx(Opcode::JumpIfTrue, test_reg, 0));
                self.free_reg();
                self.code.patch_jump(jump_back, loop_start);
            }
            StatementKind::ForStatement {
                init,
                test,
                update,
                body,
            } => {
                if let Some(init) = init {
                    self.compile_for_init(init);
                }
                let loop_start = self.code.current_offset();
                if let Some(test) = test {
                    let test_reg = self.compile_expression(test);
                    let jump_false = self
                        .code
                        .emit(Instruction::asbx(Opcode::JumpIfFalse, test_reg, 0));
                    self.free_reg();
                    self.compile_statement(body);
                    if let Some(update) = update {
                        let _update_reg = self.compile_expression(update);
                        self.free_reg();
                    }
                    let jump_back = self.code.emit(Instruction::asbx(Opcode::Jump, 0, 0));
                    self.code.patch_jump(jump_back, loop_start);
                    let end = self.code.current_offset();
                    self.code.patch_jump(jump_false, end);
                } else {
                    self.compile_statement(body);
                    if let Some(update) = update {
                        let _update_reg = self.compile_expression(update);
                        self.free_reg();
                    }
                    let jump_back = self.code.emit(Instruction::asbx(Opcode::Jump, 0, 0));
                    self.code.patch_jump(jump_back, loop_start);
                }
            }
            StatementKind::ForInStatement { right, body, .. } => {
                let _right_reg = self.compile_expression(right);
                self.free_reg();
                self.compile_statement(body);
            }
            StatementKind::ForOfStatement { left, right, body, .. } => {
                self.compile_for_of(left, right, body);
            }
            StatementKind::SwitchStatement {
                discriminant,
                cases,
            } => {
                let _disc_reg = self.compile_expression(discriminant);
                self.free_reg();
                for case in cases {
                    if let Some(test) = &case.test {
                        let _test_reg = self.compile_expression(test);
                        self.free_reg();
                    }
                    for s in &case.consequent {
                        self.compile_statement(s);
                    }
                }
            }
            StatementKind::ReturnStatement(arg) => match arg {
                Some(expr) => {
                    let reg = self.compile_expression(expr);
                    self.code.emit(Instruction::abx(Opcode::Return, reg, 0));
                }
                None => {
                    self.code.emit(Instruction::op_only(Opcode::ReturnUndef));
                }
            },
            StatementKind::BreakStatement(_) | StatementKind::ContinueStatement(_) => {}
            StatementKind::ThrowStatement(expr) => {
                let reg = self.compile_expression(expr);
                self.code.emit(Instruction::abx(Opcode::Throw, reg, 0));
            }
            StatementKind::TryStatement {
                block,
                handler,
                finalizer,
            } => {
                let catch_reg = if handler.is_some() {
                    if let Some(handler) = handler {
                        if let Some(param) = &handler.param {
                            if let PatternKind::Identifier { .. } = &param.kind {
                                self.alloc_reg()
                            } else {
                                self.alloc_reg()
                            }
                        } else {
                            self.alloc_reg()
                        }
                    } else {
                        0
                    }
                } else {
                    0
                };

                let try_start = if handler.is_some() {
                    Some(
                        self.code
                            .emit(Instruction::asbx(Opcode::TryStart, catch_reg, 0)),
                    )
                } else {
                    None
                };

                for s in block {
                    self.compile_statement(s);
                }

                if handler.is_some() {
                    self.code.emit(Instruction::op_only(Opcode::TryEnd));
                }

                let jump_normal = self.code.emit(Instruction::asbx(Opcode::Jump, 0, 0));

                let catch_start = self.code.current_offset();
                if let Some(try_start) = try_start {
                    self.code.patch_jump(try_start, catch_start);
                }

                if let Some(handler) = handler {
                    self.code
                        .emit(Instruction::abx(Opcode::CatchBind, catch_reg, 0));

                    let saved_local = if let Some(param) = &handler.param {
                        if let PatternKind::Identifier { name, .. } = &param.kind {
                            Some((name.clone(), self.locals.insert(name.clone(), catch_reg)))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    for s in &handler.body {
                        self.compile_statement(s);
                    }

                    if let Some((name, previous)) = saved_local {
                        match previous {
                            Some(prev) => {
                                self.locals.insert(name, prev);
                            }
                            None => {
                                self.locals.remove(&name);
                            }
                        }
                    }
                }

                let finally_start = self.code.current_offset();
                if let Some(finalizer) = finalizer {
                    for s in finalizer {
                        self.compile_statement(s);
                    }
                }

                let end = self.code.current_offset();
                if finalizer.is_some() {
                    self.code.patch_jump(jump_normal, finally_start);
                } else {
                    self.code.patch_jump(jump_normal, end);
                }
            }
            StatementKind::LabeledStatement { body, .. } => {
                self.compile_statement(body);
            }
            StatementKind::WithStatement { object, body } => {
                let _obj_reg = self.compile_expression(object);
                self.free_reg();
                self.compile_statement(body);
            }
            StatementKind::DebuggerStatement => {
                self.code.emit(Instruction::op_only(Opcode::Debugger));
            }
            StatementKind::Declaration(decl) => {
                self.compile_declaration(decl);
            }
        }
    }

    fn compile_for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::Expression(expr) => {
                let _reg = self.compile_expression(expr);
                self.free_reg();
            }
            ForInit::Declaration(decl) => {
                self.compile_declaration(decl);
            }
        }
    }

    fn compile_declaration(&mut self, decl: &Declaration) {
        match &decl.kind {
            DeclarationKind::VariableDeclaration {
                kind: _,
                declarations,
            } => {
                for declarator in declarations {
                    self.compile_variable_declarator(declarator);
                }
            }
            DeclarationKind::FunctionDeclaration(func) => {
                let idx = self.compile_function(func);
                let closure_reg = self.alloc_reg();
                self.code
                    .emit(Instruction::abx(Opcode::CreateClosure, closure_reg, idx));
                if let Some(name) = &func.id {
                    let name_idx = self.add_string(name);
                    self.code
                        .emit(Instruction::abx(Opcode::SetGlobal, closure_reg, name_idx));
                }
                self.free_reg();
            }
            DeclarationKind::ClassDeclaration(class) => {
                let ctor_reg = self.compile_class(class);
                if let Some(name) = &class.id {
                    let name_idx = self.add_string(name);
                    self.code
                        .emit(Instruction::abx(Opcode::SetGlobal, ctor_reg, name_idx));
                }
                self.free_reg();
            }
            DeclarationKind::ImportDeclaration { .. }
            | DeclarationKind::ExportNamedDeclaration { .. }
            | DeclarationKind::ExportDefaultDeclaration(_)
            | DeclarationKind::ExportAllDeclaration { .. } => {}
        }
    }

    fn compile_variable_declarator(&mut self, declarator: &VariableDeclarator) {
        let Some(init) = &declarator.init else {
            return;
        };

        match &declarator.id.kind {
            PatternKind::Identifier { name, .. } => {
                let reg = if let Some(&existing) = self.locals.get(name) {
                    existing
                } else {
                    let r = self.alloc_reg();
                    self.locals.insert(name.clone(), r);
                    r
                };
                self.compile_expression_to(init, reg);
                if self.is_top_level {
                    let name_idx = self.add_string(name);
                    self.code
                        .emit(Instruction::abx(Opcode::SetGlobal, reg, name_idx));
                    self.mirrored_globals.insert(name.clone());
                }
            }
            PatternKind::ArrayPattern { elements, .. } => {
                let arr_reg = self.compile_expression(init);
                for (i, elem) in elements.iter().enumerate() {
                    if let Some(pat) = elem {
                        let elem_reg = self.alloc_reg();
                        let idx_reg = self.alloc_reg();
                        self.code
                            .emit(Instruction::asbx(Opcode::LoadInt, idx_reg, i as i16));
                        self.code.emit(Instruction::abc(
                            Opcode::GetElem,
                            elem_reg,
                            arr_reg,
                            idx_reg,
                        ));
                        self.free_reg();
                        if !self.compile_pattern_binding(pat, elem_reg) {
                            self.free_reg();
                        }
                    }
                }
                self.free_reg();
            }
            PatternKind::ObjectPattern { properties, .. } => {
                let obj_reg = self.compile_expression(init);
                for prop in properties {
                    let key_name = property_key_name(&prop.key);
                    let elem_reg = self.alloc_reg();
                    let key_idx = self.add_string(&key_name);
                    self.code.emit(Instruction::abc(
                        Opcode::GetProp,
                        elem_reg,
                        obj_reg,
                        key_idx as u8,
                    ));
                    if !self.compile_pattern_binding(&prop.value, elem_reg) {
                        self.free_reg();
                    }
                }
                self.free_reg();
            }
            _ => {
                let _value_reg = self.compile_expression(init);
                self.free_reg();
            }
        }
    }

    /// Bind `value_reg` to a destructuring pattern. Returns true if `value_reg` was
    /// consumed as a new local (caller must not free it).
    fn compile_pattern_binding(&mut self, pat: &Pattern, value_reg: u8) -> bool {
        match &pat.kind {
            PatternKind::Identifier { name, .. } => {
                if let Some(&local_reg) = self.locals.get(name) {
                    self.code
                        .emit(Instruction::abc(Opcode::Move, local_reg, value_reg, 0));
                    if self.is_top_level {
                        let name_idx = self.add_string(name);
                        self.code.emit(Instruction::abx(
                            Opcode::SetGlobal,
                            value_reg,
                            name_idx,
                        ));
                        self.mirrored_globals.insert(name.clone());
                    }
                    false
                } else {
                    self.locals.insert(name.clone(), value_reg);
                    if self.is_top_level {
                        let name_idx = self.add_string(name);
                        self.code.emit(Instruction::abx(
                            Opcode::SetGlobal,
                            value_reg,
                            name_idx,
                        ));
                        self.mirrored_globals.insert(name.clone());
                    }
                    true
                }
            }
            _ => false,
        }
    }

    fn compile_for_of(&mut self, left: &ForInOfLeft, right: &Expression, body: &Statement) {
        let iter_reg = self.compile_expression(right);

        let idx_reg = self.alloc_reg();
        self.code.emit(Instruction::asbx(Opcode::LoadInt, idx_reg, 0));

        let loop_start = self.code.current_offset();

        let len_reg = self.alloc_reg();
        let length_const = self.add_string("length");
        self.code.emit(Instruction::abc(
            Opcode::GetProp,
            len_reg,
            iter_reg,
            length_const as u8,
        ));

        let cmp_reg = self.alloc_reg();
        self.code
            .emit(Instruction::abc(Opcode::Lt, cmp_reg, idx_reg, len_reg));
        let jump_end = self
            .code
            .emit(Instruction::asbx(Opcode::JumpIfFalse, cmp_reg, 0));
        self.free_reg();
        self.free_reg();

        let elem_reg = self.alloc_reg();
        self.code.emit(Instruction::abc(
            Opcode::GetElem,
            elem_reg,
            iter_reg,
            idx_reg,
        ));

        let bound_local = self.compile_for_of_left(left, elem_reg);

        self.compile_statement(body);

        let one_reg = self.alloc_reg();
        self.code.emit(Instruction::asbx(Opcode::LoadInt, one_reg, 1));
        self.code
            .emit(Instruction::abc(Opcode::Add, idx_reg, idx_reg, one_reg));
        self.free_reg();

        let jump_back = self.code.emit(Instruction::asbx(Opcode::Jump, 0, 0));
        self.code.patch_jump(jump_back, loop_start);

        let end = self.code.current_offset();
        self.code.patch_jump(jump_end, end);

        if !bound_local {
            self.free_reg();
        }
        self.free_reg();
        self.free_reg();
    }

    fn compile_for_of_left(&mut self, left: &ForInOfLeft, elem_reg: u8) -> bool {
        match left {
            ForInOfLeft::Declaration(decl) => {
                if let DeclarationKind::VariableDeclaration { declarations, .. } = &decl.kind
                    && let Some(d) = declarations.first()
                {
                    return self.compile_pattern_binding(&d.id, elem_reg);
                }
                false
            }
            ForInOfLeft::Pattern(pat) => self.compile_pattern_binding(pat, elem_reg),
            ForInOfLeft::Expression(expr) => {
                if let ExpressionKind::Identifier(name) = &expr.kind {
                    if let Some(&local_reg) = self.locals.get(name) {
                        self.code
                            .emit(Instruction::abc(Opcode::Move, local_reg, elem_reg, 0));
                    } else {
                        let name_idx = self.add_string(name);
                        self.code
                            .emit(Instruction::abx(Opcode::SetGlobal, elem_reg, name_idx));
                    }
                }
                false
            }
        }
    }

    fn compile_function(&mut self, func: &Function) -> u16 {
        let name = func.id.as_deref().unwrap_or("<anonymous>");
        let mut inner = Compiler::child(name.to_string());
        inner.code.param_count = func.params.len() as u16;
        inner.code.is_async = func.is_async;
        inner.code.is_generator = func.is_generator;

        for (i, param) in func.params.iter().enumerate() {
            if let PatternKind::Identifier { name, .. } = &param.kind {
                inner.locals.insert(name.clone(), i as u8);
            }
            inner.alloc_reg();
        }

        match &func.body {
            FunctionBody::Block(stmts) => {
                let (start, is_strict) = detect_use_strict(stmts);
                inner.code.is_strict = is_strict;
                for stmt in &stmts[start..] {
                    inner.compile_statement(stmt);
                }
                inner.code.emit(Instruction::op_only(Opcode::ReturnUndef));
            }
            FunctionBody::Expression(expr) => {
                let reg = inner.compile_expression(expr);
                inner.code.emit(Instruction::abx(Opcode::Return, reg, 0));
            }
            FunctionBody::Lazy(_) => {
                inner.code.emit(Instruction::op_only(Opcode::ReturnUndef));
            }
        }

        let idx = self.code.inner_functions.len();
        self.code.inner_functions.push(inner.code);
        idx as u16
    }

    fn compile_class(&mut self, class: &Class) -> u8 {
        let mut constructor: Option<Function> = None;
        let mut methods: Vec<(String, Function)> = Vec::new();

        for member in &class.body {
            if let ClassMemberKind::Method {
                key,
                value,
                is_static,
                computed,
                ..
            } = &member.kind
            {
                if *is_static || *computed {
                    continue;
                }
                let name = property_key_name(key);
                if name == "constructor" {
                    constructor = Some(value.clone());
                } else {
                    methods.push((name, value.clone()));
                }
            }
        }

        let mut constructor = constructor.unwrap_or_else(|| Function {
            id: class.id.clone(),
            params: vec![],
            body: FunctionBody::Block(vec![]),
            is_async: false,
            is_generator: false,
            span: class.span,
        });
        if constructor.id.is_none() {
            constructor.id = class.id.clone();
        }

        let ctor_idx = self.compile_function(&constructor);
        let ctor_reg = self.alloc_reg();
        self.code
            .emit(Instruction::abx(Opcode::CreateClosure, ctor_reg, ctor_idx));

        let proto_reg = self.alloc_reg();
        let prototype_idx = self.add_string("prototype");
        self.code.emit(Instruction::abc(
            Opcode::GetProp,
            proto_reg,
            ctor_reg,
            prototype_idx as u8,
        ));

        for (name, method) in methods {
            let method_idx = self.compile_function(&method);
            let method_reg = self.alloc_reg();
            self.code
                .emit(Instruction::abx(Opcode::CreateClosure, method_reg, method_idx));
            let name_idx = self.add_string(&name);
            self.code.emit(Instruction::abc(
                Opcode::InitProp,
                proto_reg,
                name_idx as u8,
                method_reg,
            ));
            self.free_reg();
        }

        self.free_reg();
        ctor_reg
    }

    fn compile_expression(&mut self, expr: &Expression) -> u8 {
        let dest = self.alloc_reg();
        self.compile_expression_to(expr, dest);
        dest
    }

    fn compile_expression_to(&mut self, expr: &Expression, dest: u8) {
        match &expr.kind {
            ExpressionKind::NumberLiteral(n) => {
                let i = *n as i32;
                if i as f64 == *n && i >= i16::MIN as i32 && i <= i16::MAX as i32 {
                    self.code
                        .emit(Instruction::asbx(Opcode::LoadInt, dest, i as i16));
                } else {
                    let idx = self
                        .code
                        .add_constant(Constant::Number(*n));
                    self.code
                        .emit(Instruction::abx(Opcode::LoadConst, dest, idx));
                }
            }
            ExpressionKind::StringLiteral(s) => {
                let idx = self
                    .code
                    .add_constant(Constant::String(s.clone()));
                self.code
                    .emit(Instruction::abx(Opcode::LoadConst, dest, idx));
            }
            ExpressionKind::BooleanLiteral(true) => {
                self.code.emit(Instruction::abx(Opcode::LoadTrue, dest, 0));
            }
            ExpressionKind::BooleanLiteral(false) => {
                self.code.emit(Instruction::abx(Opcode::LoadFalse, dest, 0));
            }
            ExpressionKind::NullLiteral => {
                self.code.emit(Instruction::abx(Opcode::LoadNull, dest, 0));
            }
            ExpressionKind::Identifier(name) => {
                if self.is_top_level && self.mirrored_globals.contains(name) {
                    let idx = self.add_string(name);
                    self.code
                        .emit(Instruction::abx(Opcode::GetGlobal, dest, idx));
                } else if let Some(&reg) = self.locals.get(name) {
                    self.code.emit(Instruction::abc(Opcode::Move, dest, reg, 0));
                } else {
                    let idx = self.add_string(name);
                    self.code
                        .emit(Instruction::abx(Opcode::GetGlobal, dest, idx));
                }
            }
            ExpressionKind::This => {
                let idx = self.add_string("this");
                self.code
                    .emit(Instruction::abx(Opcode::GetGlobal, dest, idx));
            }
            ExpressionKind::Super => {
                let idx = self.add_string("super");
                self.code
                    .emit(Instruction::abx(Opcode::GetGlobal, dest, idx));
            }
            ExpressionKind::UnaryExpression {
                operator,
                argument,
                ..
            } => match operator {
                UnaryOp::Minus => {
                    let arg_reg = self.compile_expression(argument);
                    self.code.emit(Instruction::abc(Opcode::Neg, dest, arg_reg, 0));
                    self.free_reg();
                }
                UnaryOp::Plus => {
                    self.compile_expression_to(argument, dest);
                }
                UnaryOp::Not => {
                    let arg_reg = self.compile_expression(argument);
                    self.code.emit(Instruction::abc(Opcode::Not, dest, arg_reg, 0));
                    self.free_reg();
                }
                UnaryOp::BitNot => {
                    let arg_reg = self.compile_expression(argument);
                    self.code
                        .emit(Instruction::abc(Opcode::BitNot, dest, arg_reg, 0));
                    self.free_reg();
                }
                UnaryOp::Typeof => {
                    let arg_reg = self.compile_expression(argument);
                    self.code
                        .emit(Instruction::abc(Opcode::TypeOf, dest, arg_reg, 0));
                    self.free_reg();
                }
                UnaryOp::Void => {
                    let _arg_reg = self.compile_expression(argument);
                    self.free_reg();
                    self.code.emit(Instruction::abx(Opcode::LoadUndef, dest, 0));
                }
                UnaryOp::Delete => {
                    self.compile_expression_to(argument, dest);
                }
            },
            ExpressionKind::UpdateExpression {
                operator,
                argument,
                prefix,
            } => {
                let _ = (operator, prefix);
                self.compile_expression_to(argument, dest);
            }
            ExpressionKind::BinaryExpression {
                operator,
                left,
                right,
            } => {
                let left_reg = self.compile_expression(left);
                let right_reg = self.compile_expression(right);
                let op = binary_opcode(*operator);
                self.code
                    .emit(Instruction::abc(op, dest, left_reg, right_reg));
                self.free_reg();
                self.free_reg();
            }
            ExpressionKind::LogicalExpression {
                operator,
                left,
                right,
            } => match operator {
                LogicalOp::And => {
                    let left_reg = self.compile_expression(left);
                    let jump_short = self
                        .code
                        .emit(Instruction::asbx(Opcode::JumpIfFalse, left_reg, 0));
                    self.code.emit(Instruction::abc(Opcode::Move, dest, left_reg, 0));
                    self.free_reg();
                    self.compile_expression_to(right, dest);
                    let end = self.code.current_offset();
                    self.code.patch_jump(jump_short, end);
                }
                LogicalOp::Or => {
                    let left_reg = self.compile_expression(left);
                    let jump_short = self
                        .code
                        .emit(Instruction::asbx(Opcode::JumpIfTrue, left_reg, 0));
                    self.code.emit(Instruction::abc(Opcode::Move, dest, left_reg, 0));
                    self.free_reg();
                    self.compile_expression_to(right, dest);
                    let end = self.code.current_offset();
                    self.code.patch_jump(jump_short, end);
                }
            },
            ExpressionKind::AssignmentExpression {
                operator,
                left,
                right,
            } => {
                if *operator == AssignOp::Assign {
                    self.compile_simple_assignment(left, right, dest);
                } else {
                    let value_reg = self.compile_expression(right);
                    self.code.emit(Instruction::abc(Opcode::Move, dest, value_reg, 0));
                    self.free_reg();
                }
            }
            ExpressionKind::MemberExpression {
                object,
                property,
                computed,
                ..
            } => {
                let obj_reg = self.compile_expression(object);
                if *computed {
                    if let MemberProperty::Expression(key_expr) = property {
                        let key_reg = self.compile_expression(key_expr);
                        self.code
                            .emit(Instruction::abc(Opcode::GetElem, dest, obj_reg, key_reg));
                        self.free_reg();
                    }
                } else {
                    let name = member_property_name(property);
                    let idx = self.add_string(&name);
                    self.code.emit(Instruction::abc(
                        Opcode::GetProp,
                        dest,
                        obj_reg,
                        idx as u8,
                    ));
                }
                self.free_reg();
            }
            ExpressionKind::CallExpression {
                callee,
                arguments,
                ..
            } => {
                if let ExpressionKind::MemberExpression {
                    object,
                    property,
                    computed,
                    ..
                } = &callee.kind
                {
                    let obj_reg = self.alloc_reg();
                    self.compile_expression_to(object, obj_reg);

                    let func_reg = obj_reg + 1;
                    self.ensure_register(func_reg + 1);
                    if *computed {
                        if let MemberProperty::Expression(key_expr) = property {
                            let key_reg = self.compile_expression(key_expr);
                            self.code.emit(Instruction::abc(
                                Opcode::GetElem,
                                func_reg,
                                obj_reg,
                                key_reg,
                            ));
                            self.free_reg();
                        }
                    } else {
                        let name = member_property_name(property);
                        let idx = self.add_string(&name);
                        self.code.emit(Instruction::abc(
                            Opcode::GetProp,
                            func_reg,
                            obj_reg,
                            idx as u8,
                        ));
                    }

                    let this_idx = self.add_string("this");
                    self.code
                        .emit(Instruction::abx(Opcode::SetGlobal, obj_reg, this_idx));

                    let argc = arguments.len() as u8;
                    for (i, arg) in arguments.iter().enumerate() {
                        let arg_reg = func_reg + 1 + i as u8;
                        self.ensure_register(arg_reg + 1);
                        self.compile_expression_to(arg, arg_reg);
                    }
                    self.next_register = func_reg + 1 + argc;

                    self.code
                        .emit(Instruction::abc(Opcode::Call, dest, func_reg, argc));
                } else {
                    let func_reg = self.alloc_reg();
                    self.compile_expression_to(callee, func_reg);

                    let argc = arguments.len() as u8;
                    for (i, arg) in arguments.iter().enumerate() {
                        let arg_reg = func_reg + 1 + i as u8;
                        self.ensure_register(arg_reg + 1);
                        self.compile_expression_to(arg, arg_reg);
                    }
                    self.next_register = func_reg + 1 + argc;

                    self.code
                        .emit(Instruction::abc(Opcode::Call, dest, func_reg, argc));
                }
            }
            ExpressionKind::NewExpression {
                callee,
                arguments,
            } => {
                let ctor_reg = self.alloc_reg();
                self.compile_expression_to(callee, ctor_reg);

                let argc = arguments.len() as u8;
                for (i, arg) in arguments.iter().enumerate() {
                    let arg_reg = ctor_reg + 1 + i as u8;
                    self.ensure_register(arg_reg + 1);
                    self.compile_expression_to(arg, arg_reg);
                }
                self.next_register = ctor_reg + 1 + argc;

                self.code
                    .emit(Instruction::abc(Opcode::New, dest, ctor_reg, argc));
            }
            ExpressionKind::ConditionalExpression {
                test,
                consequent,
                alternate,
            } => {
                let test_reg = self.compile_expression(test);
                let jump_false = self
                    .code
                    .emit(Instruction::asbx(Opcode::JumpIfFalse, test_reg, 0));
                self.free_reg();
                self.compile_expression_to(consequent, dest);
                let jump_end = self.code.emit(Instruction::asbx(Opcode::Jump, 0, 0));
                let else_start = self.code.current_offset();
                self.code.patch_jump(jump_false, else_start);
                self.compile_expression_to(alternate, dest);
                let end = self.code.current_offset();
                self.code.patch_jump(jump_end, end);
            }
            ExpressionKind::SequenceExpression(exprs) => {
                for (i, e) in exprs.iter().enumerate() {
                    if i + 1 == exprs.len() {
                        self.compile_expression_to(e, dest);
                    } else {
                        let _reg = self.compile_expression(e);
                        self.free_reg();
                    }
                }
            }
            ExpressionKind::ArrayExpression(elements) => {
                let len = elements.len() as u8;
                self.code.emit(Instruction::abc(Opcode::CreateArray, dest, len, 0));
                for (i, elem) in elements.iter().enumerate() {
                    if let Some(expr) = elem {
                        let val_reg = self.compile_expression(expr);
                        self.code.emit(Instruction::abc(
                            Opcode::SetArrayElem,
                            dest,
                            i as u8,
                            val_reg,
                        ));
                        self.free_reg();
                    }
                }
            }
            ExpressionKind::ObjectExpression(properties) => {
                let len = properties.len() as u8;
                self.code
                    .emit(Instruction::abc(Opcode::CreateObject, dest, len, 0));
                for prop in properties {
                    self.compile_object_property(dest, prop);
                }
            }
            ExpressionKind::ArrowFunctionExpression(arrow) => {
                let func = Function {
                    id: None,
                    params: arrow.params.clone(),
                    body: arrow.body.clone(),
                    is_async: arrow.is_async,
                    is_generator: false,
                    span: arrow.span,
                };
                let idx = self.compile_function(&func);
                self.code
                    .emit(Instruction::abx(Opcode::CreateClosure, dest, idx));
            }
            ExpressionKind::FunctionExpression(func) => {
                let idx = self.compile_function(func);
                self.code
                    .emit(Instruction::abx(Opcode::CreateClosure, dest, idx));
            }
            ExpressionKind::ClassExpression(class) => {
                let ctor_reg = self.compile_class(class);
                if dest != ctor_reg {
                    self.code
                        .emit(Instruction::abc(Opcode::Move, dest, ctor_reg, 0));
                }
                self.free_reg();
            }
            ExpressionKind::SpreadElement(inner) => {
                let inner_reg = self.compile_expression(inner);
                self.code
                    .emit(Instruction::abc(Opcode::Spread, dest, inner_reg, 0));
                self.free_reg();
            }
            ExpressionKind::ParenthesizedExpression(inner) => {
                self.compile_expression_to(inner, dest);
            }
            ExpressionKind::BigIntLiteral(_)
            | ExpressionKind::RegExpLiteral { .. }
            | ExpressionKind::TemplateLiteral(_)
            | ExpressionKind::TaggedTemplateExpression { .. }
            | ExpressionKind::YieldExpression { .. }
            | ExpressionKind::AwaitExpression(_)
            | ExpressionKind::MetaProperty { .. }
            | ExpressionKind::ImportExpression(_) => {
                self.code.emit(Instruction::abx(Opcode::LoadUndef, dest, 0));
            }
        }
    }

    fn ensure_register(&mut self, count: u8) {
        if self.next_register < count {
            self.next_register = count;
        }
        if self.next_register as u16 > self.code.register_count {
            self.code.register_count = self.next_register as u16;
        }
    }

    fn compile_simple_assignment(
        &mut self,
        left: &AssignTarget,
        right: &Expression,
        dest: u8,
    ) {
        let value_reg = self.compile_expression(right);
        match left {
            AssignTarget::Identifier(name) => {
                if let Some(&reg) = self.locals.get(name) {
                    if reg != value_reg {
                        self.code
                            .emit(Instruction::abc(Opcode::Move, reg, value_reg, 0));
                    }
                    if self.is_top_level {
                        let name_idx = self.add_string(name);
                        self.code
                            .emit(Instruction::abx(Opcode::SetGlobal, value_reg, name_idx));
                        self.mirrored_globals.insert(name.clone());
                    }
                    if dest != reg {
                        self.code
                            .emit(Instruction::abc(Opcode::Move, dest, value_reg, 0));
                    }
                } else {
                    let name_idx = self.add_string(name);
                    self.code
                        .emit(Instruction::abx(Opcode::SetGlobal, value_reg, name_idx));
                    if dest != value_reg {
                        self.code
                            .emit(Instruction::abc(Opcode::Move, dest, value_reg, 0));
                    }
                }
            }
            AssignTarget::Member(member) => {
                if let ExpressionKind::MemberExpression {
                    object,
                    property,
                    computed,
                    ..
                } = &member.kind
                {
                    let obj_reg = self.compile_expression(object);
                    if *computed {
                        if let MemberProperty::Expression(key_expr) = property {
                            let key_reg = self.compile_expression(key_expr);
                            self.code.emit(Instruction::abc(
                                Opcode::SetElem,
                                obj_reg,
                                key_reg,
                                value_reg,
                            ));
                            self.free_reg();
                            self.free_reg();
                        }
                    } else {
                        let name = member_property_name(property);
                        let idx = self.add_string(&name);
                        self.code.emit(Instruction::abc(
                            Opcode::SetProp,
                            obj_reg,
                            idx as u8,
                            value_reg,
                        ));
                        self.free_reg();
                    }
                    self.code
                        .emit(Instruction::abc(Opcode::Move, dest, value_reg, 0));
                }
            }
            AssignTarget::Pattern(_) => {
                self.code
                    .emit(Instruction::abc(Opcode::Move, dest, value_reg, 0));
            }
        }
        self.free_reg();
    }

    fn compile_object_property(&mut self, obj_reg: u8, prop: &ObjectProperty) {
        match &prop.kind {
            ObjectPropertyKind::Property {
                key,
                value,
                computed,
                shorthand,
            } => {
                let key_name = if *shorthand {
                    if let PropertyKey::Identifier(name) = key {
                        name.clone()
                    } else {
                        return;
                    }
                } else if *computed {
                    return;
                } else {
                    property_key_name(key)
                };
                let val_reg = self.compile_expression(value);
                let idx = self.add_string(&key_name);
                self.code.emit(Instruction::abc(
                    Opcode::InitProp,
                    obj_reg,
                    idx as u8,
                    val_reg,
                ));
                self.free_reg();
            }
            ObjectPropertyKind::Method { .. } | ObjectPropertyKind::SpreadElement(_) => {}
        }
    }
}

fn binary_opcode(op: BinaryOp) -> Opcode {
    match op {
        BinaryOp::Add => Opcode::Add,
        BinaryOp::Sub => Opcode::Sub,
        BinaryOp::Mul => Opcode::Mul,
        BinaryOp::Div => Opcode::Div,
        BinaryOp::Mod => Opcode::Mod,
        BinaryOp::Exp => Opcode::Exp,
        BinaryOp::Eq => Opcode::Eq,
        BinaryOp::NotEq => Opcode::Eq,
        BinaryOp::StrictEq => Opcode::StrictEq,
        BinaryOp::StrictNotEq => Opcode::StrictEq,
        BinaryOp::Lt => Opcode::Lt,
        BinaryOp::LtEq => Opcode::LtEq,
        BinaryOp::Gt => Opcode::Gt,
        BinaryOp::GtEq => Opcode::GtEq,
        BinaryOp::Shl => Opcode::Shl,
        BinaryOp::Shr => Opcode::Shr,
        BinaryOp::UShr => Opcode::UShr,
        BinaryOp::BitAnd => Opcode::BitAnd,
        BinaryOp::BitOr => Opcode::BitOr,
        BinaryOp::BitXor => Opcode::BitXor,
        BinaryOp::In => Opcode::In,
        BinaryOp::Instanceof => Opcode::InstanceOf,
        BinaryOp::NullishCoalescing => Opcode::Eq,
    }
}

fn member_property_name(property: &MemberProperty) -> String {
    match property {
        MemberProperty::Identifier(name) => name.clone(),
        MemberProperty::PrivateIdentifier(name) => name.clone(),
        MemberProperty::Expression(expr) => match &expr.kind {
            ExpressionKind::StringLiteral(s) => s.clone(),
            ExpressionKind::Identifier(name) => name.clone(),
            _ => String::new(),
        },
    }
}

fn detect_use_strict(stmts: &[Statement]) -> (usize, bool) {
    let Some(first) = stmts.first() else {
        return (0, false);
    };
    if is_use_strict_directive(first) {
        (1, true)
    } else {
        (0, false)
    }
}

fn is_use_strict_directive(stmt: &Statement) -> bool {
    matches!(
        &stmt.kind,
        StatementKind::ExpressionStatement(expr)
            if matches!(&expr.kind, ExpressionKind::StringLiteral(s) if s == "use strict")
    )
}

fn property_key_name(key: &PropertyKey) -> String {
    match key {
        PropertyKey::Identifier(name) => name.clone(),
        PropertyKey::String(name) => name.clone(),
        PropertyKey::Number(n) => n.to_string(),
        PropertyKey::Computed(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codeblock::Constant;
    use one_parser::parser::Parser;

    fn compile(src: &str) -> CodeBlock {
        let program = Parser::parse(src).expect("parse failed");
        Compiler::compile(&program)
    }

    #[test]
    fn compile_number_literal() {
        let code = compile("42;");
        assert!(code.bytecode.len() >= 2); // LoadInt + ReturnUndef
        assert_eq!(code.bytecode[0].opcode(), Opcode::LoadInt);
        assert_eq!(code.bytecode[0].sbx(), 42);
    }

    #[test]
    fn compile_float_literal() {
        let code = compile("3.14;");
        assert_eq!(code.bytecode[0].opcode(), Opcode::LoadConst);
        assert!(matches!(
            &code.constants[0],
            Constant::Number(n) if (*n - 3.14).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn compile_string_literal() {
        let code = compile(r#""hello";"#);
        assert_eq!(code.bytecode[0].opcode(), Opcode::LoadConst);
        assert!(matches!(
            &code.constants[0],
            Constant::String(s) if s == "hello"
        ));
    }

    #[test]
    fn compile_boolean_true() {
        let code = compile("true;");
        assert_eq!(code.bytecode[0].opcode(), Opcode::LoadTrue);
    }

    #[test]
    fn compile_null() {
        let code = compile("null;");
        assert_eq!(code.bytecode[0].opcode(), Opcode::LoadNull);
    }

    #[test]
    fn compile_binary_add() {
        let code = compile("1 + 2;");
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::Add));
    }

    #[test]
    fn compile_variable_declaration() {
        let code = compile("let x = 42;");
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::SetGlobal) || opcodes.contains(&Opcode::LoadInt));
    }

    #[test]
    fn compile_identifier() {
        let code = compile("x;");
        assert_eq!(code.bytecode[0].opcode(), Opcode::GetGlobal);
    }

    #[test]
    fn compile_member_expression() {
        let code = compile("a.b;");
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::GetGlobal));
        assert!(opcodes.contains(&Opcode::GetProp));
    }

    #[test]
    fn compile_call_expression() {
        let code = compile("foo(1);");
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::Call));
    }

    #[test]
    fn compile_console_log() {
        let code = compile(r#"console.log("Hello World");"#);
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::GetGlobal));
        assert!(opcodes.contains(&Opcode::GetProp));
        assert!(opcodes.contains(&Opcode::LoadConst));
        assert!(opcodes.contains(&Opcode::Call));
    }

    #[test]
    fn compile_if_statement() {
        let code = compile("if (true) { 1; }");
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::JumpIfFalse));
    }

    #[test]
    fn compile_while_loop() {
        let code = compile("while (true) { 1; }");
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::Jump));
        assert!(opcodes.contains(&Opcode::JumpIfFalse));
    }

    #[test]
    fn compile_return_value() {
        let code = compile("return 42;");
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::Return));
    }

    #[test]
    fn compile_unary_minus() {
        let code = compile("-x;");
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::Neg));
    }

    #[test]
    fn ends_with_return_undef() {
        let code = compile("42;");
        let last = code.bytecode.last().unwrap();
        assert_eq!(last.opcode(), Opcode::ReturnUndef);
    }

    #[test]
    fn use_strict_sets_flag() {
        let code = compile(r#""use strict"; 42;"#);
        assert!(code.is_strict);
    }

    #[test]
    fn compile_eval_returns_last_expression() {
        let program = Parser::parse("1 + 2;").expect("parse failed");
        let code = Compiler::compile_eval(&program);
        let opcodes: Vec<_> = code.bytecode.iter().map(|i| i.opcode()).collect();
        assert!(opcodes.contains(&Opcode::Return));
        assert!(!opcodes.contains(&Opcode::ReturnUndef));
    }
}
