use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use crate::ast::Ast;
use crate::compiler::{Chunk, Compiler};
use crate::diagnostics::SelError;
use crate::internal::{drain_captured_output, enable_output_capture};
use crate::lexer::Loc;
use crate::parser::{parse_all, resolve_ast};
use crate::runtime::{macro_expand, CallFrame, Env, VM};
use crate::types::{intern, lookup};
use crate::value::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmStatus {
    Ready,
    Running,
    Paused,
    Halted,
    Trapped(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisassembledInstruction {
    pub offset: usize,
    pub length: usize,
    pub opcode_name: String,
    pub args: String,
    pub loc: Loc,
}

pub fn disassemble_chunk(chunk: &Chunk) -> Vec<DisassembledInstruction> {
    let mut instructions = Vec::new();
    let mut ip = 0;
    let code = &chunk.code;

    while ip < code.len() {
        let offset = ip;
        let loc = chunk.get_loc(offset);
        let tag = code[ip];
        ip += 1;

        let read_u8 = |ip: &mut usize| -> u8 {
            let val = code[*ip];
            *ip += 1;
            val
        };

        let read_u32 = |ip: &mut usize| -> u32 {
            let val = u32::from_le_bytes(code[*ip..*ip + 4].try_into().unwrap());
            *ip += 4;
            val
        };

        let read_usize = |ip: &mut usize| -> usize {
            let val = u32::from_le_bytes(code[*ip..*ip + 4].try_into().unwrap());
            *ip += 4;
            val as usize
        };

        let (opcode_name, args) = match tag {
            1 => {
                let idx = read_usize(&mut ip);
                let val_str = chunk
                    .constants
                    .get(idx)
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".into());
                ("Constant", format!("#{} => {}", idx, val_str))
            }
            2 => {
                let id = read_u32(&mut ip);
                ("LoadVar", format!("'{}' (id: {})", lookup(id), id))
            }
            3 => {
                let id = read_u32(&mut ip);
                ("StoreVar", format!("'{}' (id: {})", lookup(id), id))
            }
            4 => {
                let id = read_u32(&mut ip);
                ("DefVar", format!("'{}' (id: {})", lookup(id), id))
            }
            5 => {
                let idx = read_u8(&mut ip);
                ("LoadLocal", format!("slot: {}", idx))
            }
            6 => {
                let idx = read_u8(&mut ip);
                ("StoreLocal", format!("slot: {}", idx))
            }
            7 => ("Pop", "".into()),
            8 => {
                let target = read_usize(&mut ip);
                ("JumpIfFalse", format!("-> @{:#06x}", target))
            }
            9 => {
                let target = read_usize(&mut ip);
                ("Jump", format!("-> @{:#06x}", target))
            }
            10 => {
                let arity = read_usize(&mut ip);
                ("Call", format!("arity: {}", arity))
            }
            11 => {
                let arity = read_usize(&mut ip);
                ("TailCall", format!("arity: {}", arity))
            }
            12 => {
                let idx = read_usize(&mut ip);
                ("MakeClosure", format!("const: #{}", idx))
            }
            13 => {
                let id = read_u32(&mut ip);
                let idx = read_usize(&mut ip);
                (
                    "MakeMacro",
                    format!("name: '{}', const: #{}", lookup(id), idx),
                )
            }
            14 => ("Return", "".into()),
            15 => {
                let count = read_u32(&mut ip) as usize;
                let mut vars = Vec::new();
                for _ in 0..count {
                    let id = read_u32(&mut ip);
                    vars.push(lookup(id));
                }
                ("BuildEnv", format!("[{}]", vars.join(", ")))
            }
            16 => {
                let count = read_usize(&mut ip);
                ("PopEnv", format!("count: {}", count))
            }
            17 => {
                let target = read_usize(&mut ip);
                ("RegisterCatch", format!("handler @{:#06x}", target))
            }
            18 => ("UnregisterCatch", "".into()),
            19 => ("Yield", "".into()),
            20 => ("CoResume", "".into()),
            21 => {
                let mod_name = read_u32(&mut ip);
                let has_alias = read_u8(&mut ip);
                let alias_str = if has_alias == 1 {
                    let alias_id = read_u32(&mut ip);
                    format!(" as '{}'", lookup(alias_id))
                } else {
                    "".into()
                };
                ("Import", format!("'{}'{}", lookup(mod_name), alias_str))
            }
            22 => {
                let is_pub = read_u8(&mut ip) != 0;
                ("SetVisibility", format!("public: {}", is_pub))
            }
            23 => ("MakeRecord", "".into()),
            24 => {
                let sym = read_u32(&mut ip);
                ("AssocRecord", format!("field: '{}'", lookup(sym)))
            }
            25 => {
                let len = read_usize(&mut ip);
                ("MakeList", format!("len: {}", len))
            }
            26 => {
                let count = read_usize(&mut ip);
                ("ConcatList", format!("count: {}", count))
            }
            27 => {
                let arity = read_u32(&mut ip);
                ("Sum", format!("arity: {}", arity))
            }
            28 => {
                let arity = read_u32(&mut ip);
                ("Sub", format!("arity: {}", arity))
            }
            29 => {
                let arity = read_u32(&mut ip);
                ("Mul", format!("arity: {}", arity))
            }
            30 => {
                let arity = read_u32(&mut ip);
                ("Div", format!("arity: {}", arity))
            }
            31 => ("Mod", "".into()),
            32 => {
                let arity = read_u32(&mut ip);
                ("Eq", format!("arity: {}", arity))
            }
            33 => {
                let arity = read_u32(&mut ip);
                ("NumEq", format!("arity: {}", arity))
            }
            34 => {
                let arity = read_u32(&mut ip);
                ("NumNotEq", format!("arity: {}", arity))
            }
            35 => {
                let arity = read_u32(&mut ip);
                ("NumLt", format!("arity: {}", arity))
            }
            36 => {
                let arity = read_u32(&mut ip);
                ("NumGt", format!("arity: {}", arity))
            }
            37 => {
                let arity = read_u32(&mut ip);
                ("NumLte", format!("arity: {}", arity))
            }
            38 => {
                let arity = read_u32(&mut ip);
                ("NumGte", format!("arity: {}", arity))
            }
            39 => ("Cons", "".into()),
            40 => ("Car", "".into()),
            41 => ("Cdr", "".into()),
            42 => ("Nth", "".into()),
            43 => ("Count", "".into()),
            44 => ("Empty", "".into()),
            45 => ("IsNil", "".into()),
            46 => ("IsList", "".into()),
            47 => ("IsNumber", "".into()),
            48 => ("IsString", "".into()),
            49 => ("IsSymbol", "".into()),
            50 => ("IsFunction", "".into()),
            51 => ("TypeOf", "".into()),
            52 => {
                let arity = read_u32(&mut ip);
                ("Not", format!("arity: {}", arity))
            }
            53 => ("Load", "".into()),
            54 => {
                let sym = read_u32(&mut ip);
                ("RecordGet", format!("field: '{}'", lookup(sym)))
            }
            other => ("Unknown", format!("tag: {}", other)),
        };

        let length = ip - offset;
        instructions.push(DisassembledInstruction {
            offset,
            length,
            opcode_name: opcode_name.into(),
            args,
            loc,
        });
    }

    instructions
}

#[derive(Debug, Clone, PartialEq)]
pub struct AstGraphNode {
    pub name: String,
    pub detail: Option<String>,
    pub loc: Loc,
    pub children: Vec<AstGraphNode>,
}

pub fn ast_to_graph(ast: &Ast) -> AstGraphNode {
    match ast {
        Ast::Integer(loc, i) => AstGraphNode {
            name: "Integer".into(),
            detail: Some(i.to_string()),
            loc: *loc,
            children: Vec::new(),
        },
        Ast::Float(loc, f) => AstGraphNode {
            name: "Float".into(),
            detail: Some(f.to_string()),
            loc: *loc,
            children: Vec::new(),
        },
        Ast::Boolean(loc, b) => AstGraphNode {
            name: "Boolean".into(),
            detail: Some(b.to_string()),
            loc: *loc,
            children: Vec::new(),
        },
        Ast::String(loc, s) => AstGraphNode {
            name: "String".into(),
            detail: Some(format!("{:?}", s)),
            loc: *loc,
            children: Vec::new(),
        },
        Ast::Char(loc, c) => AstGraphNode {
            name: "Char".into(),
            detail: Some(format!("{:?}", c)),
            loc: *loc,
            children: Vec::new(),
        },
        Ast::Symbol(loc, id) => AstGraphNode {
            name: "Symbol".into(),
            detail: Some(lookup(*id)),
            loc: *loc,
            children: Vec::new(),
        },
        Ast::Bind(loc, id) => AstGraphNode {
            name: "Bind".into(),
            detail: Some(lookup(*id)),
            loc: *loc,
            children: Vec::new(),
        },
        Ast::Nil(loc) => AstGraphNode {
            name: "Nil".into(),
            detail: None,
            loc: *loc,
            children: Vec::new(),
        },
        Ast::Define(loc, id, val) => AstGraphNode {
            name: "Define".into(),
            detail: Some(lookup(*id)),
            loc: *loc,
            children: vec![ast_to_graph(val)],
        },
        Ast::DefMacro(loc, id, val) => AstGraphNode {
            name: "DefMacro".into(),
            detail: Some(lookup(*id)),
            loc: *loc,
            children: vec![ast_to_graph(val)],
        },
        Ast::Set(loc, id, val) => AstGraphNode {
            name: "Set!".into(),
            detail: Some(lookup(*id)),
            loc: *loc,
            children: vec![ast_to_graph(val)],
        },
        Ast::If(loc, cond, thn, els) => {
            let mut children = vec![ast_to_graph(cond), ast_to_graph(thn)];
            if let Some(e) = els {
                children.push(ast_to_graph(e));
            }
            AstGraphNode {
                name: "If".into(),
                detail: None,
                loc: *loc,
                children,
            }
        }
        Ast::When(loc, cond, body) => {
            let mut children = vec![ast_to_graph(cond)];
            children.extend(body.iter().map(ast_to_graph));
            AstGraphNode {
                name: "When".into(),
                detail: None,
                loc: *loc,
                children,
            }
        }
        Ast::Unless(loc, cond, thn, els) => {
            let mut children = vec![ast_to_graph(cond), ast_to_graph(thn)];
            if let Some(e) = els {
                children.push(ast_to_graph(e));
            }
            AstGraphNode {
                name: "Unless".into(),
                detail: None,
                loc: *loc,
                children,
            }
        }
        Ast::Cond(loc, clauses) => {
            let mut children = Vec::new();
            for (test, expr) in clauses {
                children.push(AstGraphNode {
                    name: "Clause".into(),
                    detail: None,
                    loc: test.loc(),
                    children: vec![ast_to_graph(test), ast_to_graph(expr)],
                });
            }
            AstGraphNode {
                name: "Cond".into(),
                detail: None,
                loc: *loc,
                children,
            }
        }
        Ast::While(loc, cond, body) => AstGraphNode {
            name: "While".into(),
            detail: None,
            loc: *loc,
            children: vec![ast_to_graph(cond), ast_to_graph(body)],
        },
        Ast::Until(loc, cond, body) => AstGraphNode {
            name: "Until".into(),
            detail: None,
            loc: *loc,
            children: vec![ast_to_graph(cond), ast_to_graph(body)],
        },
        Ast::Lambda(loc, params, body) => {
            let param_names = params.iter().map(|p| lookup(*p)).collect::<Vec<_>>().join(" ");
            let children = body.iter().map(ast_to_graph).collect();
            AstGraphNode {
                name: "Lambda".into(),
                detail: Some(format!("({})", param_names)),
                loc: *loc,
                children,
            }
        }
        Ast::Let(loc, bindings, body) => {
            let mut children = Vec::new();
            for (id, val) in bindings {
                children.push(AstGraphNode {
                    name: "Binding".into(),
                    detail: Some(lookup(*id)),
                    loc: val.loc(),
                    children: vec![ast_to_graph(val)],
                });
            }
            children.extend(body.iter().map(ast_to_graph));
            AstGraphNode {
                name: "Let".into(),
                detail: None,
                loc: *loc,
                children,
            }
        }
        Ast::Begin(loc, stmts) => AstGraphNode {
            name: "Begin".into(),
            detail: None,
            loc: *loc,
            children: stmts.iter().map(ast_to_graph).collect(),
        },
        Ast::List(loc, items) => {
            let label = if let Some(first) = items.first() {
                if let Ast::Symbol(_, sym) = first {
                    format!("({})", lookup(*sym))
                } else {
                    "List".into()
                }
            } else {
                "List (empty)".into()
            };
            AstGraphNode {
                name: label,
                detail: None,
                loc: *loc,
                children: items.iter().map(ast_to_graph).collect(),
            }
        }
        Ast::And(loc, items) => AstGraphNode {
            name: "And".into(),
            detail: None,
            loc: *loc,
            children: items.iter().map(ast_to_graph).collect(),
        },
        Ast::Or(loc, items) => AstGraphNode {
            name: "Or".into(),
            detail: None,
            loc: *loc,
            children: items.iter().map(ast_to_graph).collect(),
        },
        Ast::Import(loc, id, alias) => {
            let alias_str = alias.map(|a| format!(" as {}", lookup(a))).unwrap_or_default();
            AstGraphNode {
                name: "Import".into(),
                detail: Some(format!("{}{}", lookup(*id), alias_str)),
                loc: *loc,
                children: Vec::new(),
            }
        }
        Ast::Quote(loc, val) => AstGraphNode {
            name: "Quote".into(),
            detail: None,
            loc: *loc,
            children: vec![ast_to_graph(val)],
        },
        Ast::Quasiquote(loc, val) => AstGraphNode {
            name: "Quasiquote".into(),
            detail: None,
            loc: *loc,
            children: vec![ast_to_graph(val)],
        },
        Ast::Unquote(loc, val) => AstGraphNode {
            name: "Unquote".into(),
            detail: None,
            loc: *loc,
            children: vec![ast_to_graph(val)],
        },
        Ast::UnquoteSplicing(loc, val) => AstGraphNode {
            name: "UnquoteSplicing".into(),
            detail: None,
            loc: *loc,
            children: vec![ast_to_graph(val)],
        },
        Ast::Try(loc, body, err_id, catch_body) => {
            let mut children = vec![ast_to_graph(body)];
            children.extend(catch_body.iter().map(ast_to_graph));
            AstGraphNode {
                name: "Try".into(),
                detail: Some(format!("catch as {}", lookup(*err_id))),
                loc: *loc,
                children,
            }
        }
        Ast::Yield(loc, val) => AstGraphNode {
            name: "Yield".into(),
            detail: None,
            loc: *loc,
            children: vec![ast_to_graph(val)],
        },
        Ast::CoResume(loc, target, val) => AstGraphNode {
            name: "CoResume".into(),
            detail: None,
            loc: *loc,
            children: vec![ast_to_graph(target), ast_to_graph(val)],
        },
        Ast::Record(loc, fields) => {
            let mut children = Vec::new();
            for (sym, val) in fields {
                children.push(AstGraphNode {
                    name: "Field".into(),
                    detail: Some(lookup(*sym)),
                    loc: val.loc(),
                    children: vec![ast_to_graph(val)],
                });
            }
            AstGraphNode {
                name: "Record".into(),
                detail: None,
                loc: *loc,
                children,
            }
        }
        Ast::Match(loc, target, clauses) => {
            let mut children = vec![ast_to_graph(target)];
            for c in clauses {
                let mut clause_children = Vec::new();
                if let Some(g) = &c.guard {
                    clause_children.push(ast_to_graph(g));
                }
                clause_children.extend(c.body.iter().map(ast_to_graph));
                children.push(AstGraphNode {
                    name: "MatchClause".into(),
                    detail: None,
                    loc: c.loc,
                    children: clause_children,
                });
            }
            AstGraphNode {
                name: "Match".into(),
                detail: None,
                loc: *loc,
                children,
            }
        }
        Ast::VisibilityDirective(loc, is_pub) => AstGraphNode {
            name: "Visibility".into(),
            detail: Some(if *is_pub { "public" } else { "private" }.into()),
            loc: *loc,
            children: Vec::new(),
        },
        Ast::Load(loc, path) => AstGraphNode {
            name: "Load".into(),
            detail: None,
            loc: *loc,
            children: vec![ast_to_graph(path)],
        },
        Ast::DotAccess(loc, expr, field_sym) => AstGraphNode {
            name: "DotAccess".into(),
            detail: Some(format!(".{}", lookup(*field_sym))),
            loc: *loc,
            children: vec![ast_to_graph(expr)],
        },
        Ast::Pipeline(loc, left, right, kind) => AstGraphNode {
            name: "Pipeline".into(),
            detail: Some(match kind {
                crate::ast::PipelineKind::ThreadFirst => "|>".into(),
                crate::ast::PipelineKind::ThreadLast => "|>>".into(),
            }),
            loc: *loc,
            children: vec![ast_to_graph(left), ast_to_graph(right)],
        },
    }
}

#[derive(Debug, Clone)]
pub struct ActiveVariable {
    pub name: String,
    pub value_str: String,
}

#[derive(Debug, Clone)]
pub struct CallFrameSnapshot {
    pub function_name: String,
    pub loc: Loc,
    pub ip: usize,
}

#[derive(Debug, Clone)]
pub struct VmSnapshot {
    pub ip: usize,
    pub current_chunk_idx: usize,
    pub status: VmStatus,
    pub current_loc: Loc,
    pub current_opcode: Option<String>,
    pub stack: Vec<String>,
    pub call_stack: Vec<CallFrameSnapshot>,
    pub local_variables: Vec<ActiveVariable>,
    pub stdout_drain: Vec<String>,
    pub instructions_executed: usize,
    pub trap_error: Option<String>,
    pub last_result: Option<String>,
}

pub struct DebugSession {
    pub vm: VM,
    pub frames: Vec<CallFrame>,
    pub env: Rc<RefCell<Env>>,
    pub chunks: Vec<(Loc, Rc<Chunk>)>,
    pub chunk_index: usize,
    pub status: VmStatus,
    pub line_breakpoints: HashSet<u32>,
    pub instructions_executed: usize,
    pub stdout_buffer: Vec<String>,
    pub last_result: Option<Value>,
}

impl DebugSession {
    pub fn new(source: &str, env: Rc<RefCell<Env>>) -> Result<Self, SelError> {
        enable_output_capture();
        drain_captured_output();

        let file_id = intern("<playground>");
        let mut diags = Vec::new();
        let asts = parse_all(source, file_id, &mut diags);

        if !diags.is_empty() {
            return Err(diags.remove(0));
        }

        let mut chunks = Vec::new();
        for ast in asts {
            let loc = ast.loc();
            let expanded = macro_expand(ast, env.clone())?;
            let resolved = resolve_ast(expanded)?;
            let mut chunk = Chunk::new();
            let mut compiler = Compiler::new(&mut chunk);
            compiler.compile(resolved)?;
            chunks.push((loc, Rc::new(chunk)));
        }

        let status = if chunks.is_empty() {
            VmStatus::Halted
        } else {
            VmStatus::Ready
        };

        let frames = if let Some((loc, chunk)) = chunks.first() {
            vec![CallFrame {
                loc: *loc,
                chunk: chunk.clone(),
                ip: 0,
                env: env.clone(),
                locals: Vec::new(),
            }]
        } else {
            Vec::new()
        };

        Ok(Self {
            vm: VM::new(),
            frames,
            env,
            chunks,
            chunk_index: 0,
            status,
            line_breakpoints: HashSet::new(),
            instructions_executed: 0,
            stdout_buffer: Vec::new(),
            last_result: None,
        })
    }

    pub fn toggle_line_breakpoint(&mut self, line: u32) -> bool {
        if self.line_breakpoints.contains(&line) {
            self.line_breakpoints.remove(&line);
            false
        } else {
            self.line_breakpoints.insert(line);
            true
        }
    }

    pub fn current_loc(&self) -> Loc {
        if let Some(frame) = self.frames.last() {
            frame.chunk.get_loc(frame.ip)
        } else if let Some((loc, _)) = self.chunks.get(self.chunk_index) {
            *loc
        } else {
            Loc::default()
        }
    }

    pub fn step_instruction(&mut self) -> VmSnapshot {
        if self.status == VmStatus::Halted || matches!(self.status, VmStatus::Trapped(_)) {
            return self.create_snapshot();
        }

        if self.frames.is_empty() {
            self.advance_to_next_chunk();
            if self.frames.is_empty() {
                self.status = VmStatus::Halted;
                return self.create_snapshot();
            }
        }

        match self.vm.step_frame(&mut self.frames) {
            Ok(Some(val)) => {
                self.last_result = Some(val);
                self.instructions_executed += 1;

                if self.frames.is_empty() {
                    self.advance_to_next_chunk();
                } else {
                    self.status = VmStatus::Paused;
                }
            }
            Ok(None) => {
                self.instructions_executed += 1;
                let loc = self.current_loc();
                if self.line_breakpoints.contains(&loc.line) {
                    self.status = VmStatus::Paused;
                } else {
                    self.status = VmStatus::Paused;
                }
            }
            Err(err) => {
                if !self.vm.catch_handlers.is_empty() {
                    if let Err(catch_err) = self.vm.handle_error(&mut self.frames, err) {
                        self.status = VmStatus::Trapped(catch_err.to_string());
                    } else {
                        self.status = VmStatus::Paused;
                    }
                } else {
                    self.status = VmStatus::Trapped(err.to_string());
                }
            }
        }

        self.stdout_buffer.extend(drain_captured_output());
        self.create_snapshot()
    }

    fn advance_to_next_chunk(&mut self) {
        self.chunk_index += 1;
        if self.chunk_index < self.chunks.len() {
            let (loc, chunk) = &self.chunks[self.chunk_index];
            self.frames.push(CallFrame {
                loc: *loc,
                chunk: chunk.clone(),
                ip: 0,
                env: self.env.clone(),
                locals: Vec::new(),
            });
            self.status = VmStatus::Ready;
        } else {
            self.status = VmStatus::Halted;
        }
    }

    pub fn resume_with_fuel(&mut self, max_steps: usize) -> VmSnapshot {
        let mut executed = 0;
        self.status = VmStatus::Running;
        let mut accumulated_stdout = Vec::new();

        while executed < max_steps
            && self.status != VmStatus::Halted
            && !matches!(self.status, VmStatus::Trapped(_))
        {
            if executed > 0 {
                let loc = self.current_loc();
                if self.line_breakpoints.contains(&loc.line) {
                    self.status = VmStatus::Paused;
                    break;
                }
            }

            let snap = self.step_instruction();
            accumulated_stdout.extend(snap.stdout_drain);
            executed += 1;

            if matches!(self.status, VmStatus::Trapped(_)) || self.status == VmStatus::Halted {
                break;
            }
        }

        let mut final_snap = self.create_snapshot();
        accumulated_stdout.extend(final_snap.stdout_drain);
        final_snap.stdout_drain = accumulated_stdout;
        final_snap
    }

    pub fn create_snapshot(&mut self) -> VmSnapshot {
        let loc = self.current_loc();
        let (ip, opcode_str) = if let Some(frame) = self.frames.last() {
            let ip = frame.ip;
            let op = if ip < frame.chunk.code.len() {
                let tag = frame.chunk.code[ip];
                Some(format!("opcode_{}", tag))
            } else {
                None
            };
            (ip, op)
        } else {
            (0, None)
        };

        let stack_strings = self.vm.stack.iter().map(|v| v.to_string()).collect();

        let call_stack = self
            .frames
            .iter()
            .map(|f| CallFrameSnapshot {
                function_name: "<anonymous>".into(),
                loc: f.loc,
                ip: f.ip,
            })
            .collect();

        let mut local_variables = Vec::new();
        if let Some(frame) = self.frames.last() {
            for (sym, val) in frame.env.borrow().bindings.iter() {
                local_variables.push(ActiveVariable {
                    name: lookup(*sym),
                    value_str: val.to_string(),
                });
            }
            for (idx, val) in frame.locals.iter().enumerate() {
                local_variables.push(ActiveVariable {
                    name: format!("local_{}", idx),
                    value_str: val.to_string(),
                });
            }
        }

        self.stdout_buffer.extend(drain_captured_output());
        let stdout_drain = std::mem::take(&mut self.stdout_buffer);

        let trap_error = match &self.status {
            VmStatus::Trapped(msg) => Some(msg.clone()),
            _ => None,
        };

        VmSnapshot {
            ip,
            current_chunk_idx: self.chunk_index,
            status: self.status.clone(),
            current_loc: loc,
            current_opcode: opcode_str,
            stack: stack_strings,
            call_stack,
            local_variables,
            stdout_drain,
            instructions_executed: self.instructions_executed,
            trap_error,
            last_result: self.last_result.as_ref().map(|v| v.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal::load_core_lib;

    #[test]
    fn test_stepping_basic_arithmetic() {
        let env = load_core_lib();
        let mut session = DebugSession::new("(+ 10 20)", env).expect("valid code");
        assert_eq!(session.status, VmStatus::Ready);

        let snap1 = session.step_instruction();
        assert_eq!(snap1.instructions_executed, 1);

        let snap_final = session.resume_with_fuel(100);
        assert_eq!(snap_final.status, VmStatus::Halted);
        assert_eq!(snap_final.last_result.as_deref(), Some("30"));
    }

    #[test]
    fn test_stepping_with_display() {
        let env = load_core_lib();
        let mut session = DebugSession::new(
            "(define x 42)\n(display x)\n(newline)",
            env,
        )
        .expect("valid code");

        let snap_final = session.resume_with_fuel(200);
        assert_eq!(snap_final.status, VmStatus::Halted);
        let joined_stdout = snap_final.stdout_drain.join("");
        assert!(joined_stdout.contains("42"), "stdout should capture 42, got {:?}", joined_stdout);
    }

    #[test]
    fn test_disassembly() {
        let env = load_core_lib();
        let session = DebugSession::new("(+ 1 2)", env).expect("valid code");
        assert_eq!(session.chunks.len(), 1);
        let dis = disassemble_chunk(&session.chunks[0].1);
        assert!(!dis.is_empty());
        assert!(dis.iter().any(|i| i.opcode_name == "Constant"));
        assert!(dis.iter().any(|i| i.opcode_name == "Sum"));
    }

    #[test]
    fn test_ast_to_graph() {
        let file_id = intern("<test>");
        let mut diags = Vec::new();
        let asts = parse_all("(define (add a b) (+ a b))", file_id, &mut diags);
        assert!(diags.is_empty());
        let resolved = resolve_ast(asts[0].clone()).expect("valid resolve");
        let graph = ast_to_graph(&resolved);
        assert_eq!(graph.name, "Define");
        assert_eq!(graph.detail.as_deref(), Some("add"));
    }
}
