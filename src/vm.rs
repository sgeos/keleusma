extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bytecode::*;
use crate::verify;

/// A runtime error from the Keleusma VM.
#[derive(Debug, Clone)]
pub enum VmError {
    /// The value stack was empty when a pop was attempted.
    StackUnderflow,
    /// A type mismatch occurred during an operation.
    TypeError(String),
    /// Division or modulo by zero.
    DivisionByZero,
    /// Array or tuple index out of bounds.
    IndexOutOfBounds(i64, usize),
    /// Struct field not found.
    FieldNotFound(String, String),
    /// No pattern matched in match expression or multiheaded function.
    NoMatch(String),
    /// A native function returned an error.
    NativeError(String),
    /// Invalid or unexpected bytecode.
    InvalidBytecode(String),
    /// Script execution was halted by a Trap instruction.
    Trap(String),
    /// Structural verification failed at load time.
    VerifyError(String),
}

/// The execution state of the VM.
#[derive(Debug, Clone)]
pub enum VmState {
    /// The coroutine yielded a value and is suspended.
    Yielded(Value),
    /// The function completed with a return value.
    Finished(Value),
    /// The stream hit a Reset boundary. The host may hot-swap and resume.
    Reset,
}

/// A call frame on the VM call stack.
struct CallFrame {
    /// Index of the chunk being executed.
    chunk_idx: usize,
    /// Instruction pointer (next instruction to execute).
    ip: usize,
    /// Stack base for this frame's local variables.
    base: usize,
}

/// Type alias for a native function callable from Keleusma.
type NativeFn = Box<dyn Fn(&[Value]) -> Result<Value, VmError>>;

/// A registered native function.
struct NativeEntry {
    #[allow(dead_code)]
    name: String,
    func: NativeFn,
}

/// The Keleusma virtual machine.
pub struct Vm {
    module: Module,
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    natives: Vec<NativeEntry>,
    started: bool,
}

impl Vm {
    /// Create a new VM with the given compiled module.
    ///
    /// Runs structural verification on the module. Returns an error if
    /// verification fails.
    pub fn new(module: Module) -> Result<Self, VmError> {
        verify::verify(&module).map_err(|e| {
            VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message))
        })?;
        Ok(Self {
            module,
            stack: Vec::new(),
            frames: Vec::new(),
            natives: Vec::new(),
            started: false,
        })
    }

    /// Register a native function by name using a function pointer.
    pub fn register_native(
        &mut self,
        name: &str,
        func: fn(&[Value]) -> Result<Value, VmError>,
    ) {
        self.natives.push(NativeEntry {
            name: String::from(name),
            func: Box::new(func),
        });
    }

    /// Register a native function by name using a closure.
    ///
    /// This allows closures that capture state, such as a shared command
    /// buffer for audio script integration.
    pub fn register_native_closure<F>(
        &mut self,
        name: &str,
        func: F,
    )
    where
        F: Fn(&[Value]) -> Result<Value, VmError> + 'static,
    {
        self.natives.push(NativeEntry {
            name: String::from(name),
            func: Box::new(func),
        });
    }

    /// Call the module's entry point with the given arguments.
    pub fn call(&mut self, args: &[Value]) -> Result<VmState, VmError> {
        let entry = self.module.entry_point
            .ok_or_else(|| VmError::InvalidBytecode(String::from("no entry point")))?;
        self.call_function(entry, args)
    }

    /// Call a specific function by chunk index with the given arguments.
    pub fn call_function(&mut self, chunk_idx: usize, args: &[Value]) -> Result<VmState, VmError> {
        let chunk = self.module.chunks.get(chunk_idx)
            .ok_or_else(|| VmError::InvalidBytecode(format!("invalid chunk index: {}", chunk_idx)))?;

        if args.len() > chunk.local_count as usize {
            return Err(VmError::InvalidBytecode(format!(
                "too many arguments: expected at most {}, got {}",
                chunk.local_count, args.len()
            )));
        }

        let base = self.stack.len();
        // Push arguments as the first local slots.
        for arg in args {
            self.stack.push(arg.clone());
        }
        // Extend stack for remaining local slots.
        let extra = chunk.local_count as usize - args.len();
        for _ in 0..extra {
            self.stack.push(Value::Unit);
        }

        self.frames.push(CallFrame {
            chunk_idx,
            ip: 0,
            base,
        });
        self.started = true;

        self.run()
    }

    /// Resume execution after a yield or reset, providing the input value.
    pub fn resume(&mut self, input: Value) -> Result<VmState, VmError> {
        if !self.started || self.frames.is_empty() {
            return Err(VmError::InvalidBytecode(String::from("cannot resume: VM not suspended")));
        }
        // For stream functions, update the parameter slot with the new input.
        // This ensures the next iteration sees the latest input.
        if let Some(base_frame) = self.frames.first() {
            let chunk = &self.module.chunks[base_frame.chunk_idx];
            if chunk.block_type == BlockType::Stream && chunk.param_count > 0 {
                let base = base_frame.base;
                self.stack[base] = input.clone();
            }
        }
        // Push the input value onto the stack (it becomes the yield expression result).
        self.stack.push(input);
        self.run()
    }

    /// Execute bytecode until yield, return, reset, or error.
    fn run(&mut self) -> Result<VmState, VmError> {
        loop {
            if self.frames.is_empty() {
                return Err(VmError::InvalidBytecode(String::from("empty call stack")));
            }

            let frame = self.frames.last().unwrap();
            let chunk_idx = frame.chunk_idx;
            let ip = frame.ip;
            let base = frame.base;

            let chunk = &self.module.chunks[chunk_idx];
            if ip >= chunk.ops.len() {
                // End of chunk without explicit return: return Unit.
                let result = self.stack.pop().unwrap_or(Value::Unit);
                self.frames.pop();
                if self.frames.is_empty() {
                    return Ok(VmState::Finished(result));
                }
                self.stack.push(result);
                continue;
            }

            let op = chunk.ops[ip].clone();
            // Advance IP.
            self.frames.last_mut().unwrap().ip += 1;

            match op {
                Op::Const(idx) => {
                    let val = self.module.chunks[chunk_idx].constants[idx as usize].clone();
                    self.stack.push(val);
                }
                Op::PushUnit => self.stack.push(Value::Unit),
                Op::PushTrue => self.stack.push(Value::Bool(true)),
                Op::PushFalse => self.stack.push(Value::Bool(false)),

                Op::GetLocal(slot) => {
                    let val = self.stack[base + slot as usize].clone();
                    self.stack.push(val);
                }
                Op::SetLocal(slot) => {
                    let val = self.pop()?;
                    self.stack[base + slot as usize] = val;
                }

                Op::Add => self.binary_op(|a, b| match (a, b) {
                    (Value::Int(x), Value::Int(y)) => Ok(Value::Int(x.wrapping_add(y))),
                    (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
                    (Value::Str(x), Value::Str(y)) => {
                        let mut s = x;
                        s.push_str(&y);
                        Ok(Value::Str(s))
                    }
                    (a, b) => Err(VmError::TypeError(format!("cannot add {} and {}", a.type_name(), b.type_name()))),
                })?,
                Op::Sub => self.binary_arith(|a, b| a.wrapping_sub(b), |a, b| a - b)?,
                Op::Mul => self.binary_arith(|a, b| a.wrapping_mul(b), |a, b| a * b)?,
                Op::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(_), Value::Int(0)) => return Err(VmError::DivisionByZero),
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x / y)),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x / y)),
                        (a, b) => return Err(VmError::TypeError(format!("cannot divide {} by {}", a.type_name(), b.type_name()))),
                    }
                }
                Op::Mod => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(_), Value::Int(0)) => return Err(VmError::DivisionByZero),
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(x % y)),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x % y)),
                        (a, b) => return Err(VmError::TypeError(format!("cannot modulo {} by {}", a.type_name(), b.type_name()))),
                    }
                }
                Op::Neg => {
                    let val = self.pop()?;
                    match val {
                        Value::Int(x) => self.stack.push(Value::Int(-x)),
                        Value::Float(x) => self.stack.push(Value::Float(-x)),
                        v => return Err(VmError::TypeError(format!("cannot negate {}", v.type_name()))),
                    }
                }

                Op::CmpEq => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a == b)); }
                Op::CmpNe => { let b = self.pop()?; let a = self.pop()?; self.stack.push(Value::Bool(a != b)); }
                Op::CmpLt => self.compare_op(|ord| ord.is_lt())?,
                Op::CmpGt => self.compare_op(|ord| ord.is_gt())?,
                Op::CmpLe => self.compare_op(|ord| ord.is_le())?,
                Op::CmpGe => self.compare_op(|ord| ord.is_ge())?,

                Op::Not => {
                    let val = self.pop()?;
                    match val {
                        Value::Bool(b) => self.stack.push(Value::Bool(!b)),
                        v => return Err(VmError::TypeError(format!("cannot apply not to {}", v.type_name()))),
                    }
                }

                // -- Block-structured control flow --

                Op::If(target) => {
                    let val = self.pop()?;
                    match val {
                        Value::Bool(false) => {
                            self.frames.last_mut().unwrap().ip = target as usize;
                        }
                        Value::Bool(true) => {
                            // Continue to then-block.
                        }
                        v => return Err(VmError::TypeError(format!("condition must be Bool, got {}", v.type_name()))),
                    }
                }
                Op::Else(target) => {
                    // Reached when then-block completes. Skip else-block.
                    self.frames.last_mut().unwrap().ip = target as usize;
                }
                Op::EndIf => {
                    // No-op. Block delimiter.
                }

                Op::Loop(_) => {
                    // No-op at entry. Target is used by Break/BreakIf.
                }
                Op::EndLoop(target) => {
                    // Back-edge: jump to instruction after Loop.
                    self.frames.last_mut().unwrap().ip = target as usize;
                }
                Op::Break(target) => {
                    self.frames.last_mut().unwrap().ip = target as usize;
                }
                Op::BreakIf(target) => {
                    let val = self.pop()?;
                    match val {
                        Value::Bool(true) => {
                            self.frames.last_mut().unwrap().ip = target as usize;
                        }
                        Value::Bool(false) => {
                            // Continue loop body.
                        }
                        v => return Err(VmError::TypeError(format!("BreakIf condition must be Bool, got {}", v.type_name()))),
                    }
                }

                // -- Streaming --

                Op::Stream => {
                    // No-op. Marks the stream entry point.
                }
                Op::Reset => {
                    // Clear arena: reset locals to Unit, truncate stack.
                    let frame = self.frames.last_mut().unwrap();
                    let reset_base = frame.base;
                    let reset_chunk_idx = frame.chunk_idx;
                    let local_count = self.module.chunks[reset_chunk_idx].local_count as usize;

                    // Clear locals to Unit.
                    for i in 0..local_count {
                        self.stack[reset_base + i] = Value::Unit;
                    }
                    // Truncate stack to just the locals.
                    self.stack.truncate(reset_base + local_count);

                    // Find Stream instruction and set IP to instruction after it.
                    let ops = &self.module.chunks[reset_chunk_idx].ops;
                    let stream_ip = ops.iter().position(|op| matches!(op, Op::Stream));
                    match stream_ip {
                        Some(pos) => frame.ip = pos + 1,
                        None => return Err(VmError::InvalidBytecode(
                            String::from("Reset without Stream in chunk")
                        )),
                    }

                    return Ok(VmState::Reset);
                }

                // -- Functions --

                Op::Call(idx, arg_count) => {
                    let called_chunk = self.module.chunks.get(idx as usize)
                        .ok_or_else(|| VmError::InvalidBytecode(format!("invalid chunk: {}", idx)))?;
                    let new_base = self.stack.len() - arg_count as usize;
                    let extra = called_chunk.local_count as usize - arg_count as usize;
                    for _ in 0..extra {
                        self.stack.push(Value::Unit);
                    }
                    self.frames.push(CallFrame {
                        chunk_idx: idx as usize,
                        ip: 0,
                        base: new_base,
                    });
                }
                Op::CallNative(idx, arg_count) => {
                    let n = arg_count as usize;
                    if self.stack.len() < n {
                        return Err(VmError::StackUnderflow);
                    }
                    let args: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
                    let native_name = self.module.native_names.get(idx as usize)
                        .ok_or_else(|| VmError::InvalidBytecode(format!("invalid native index: {}", idx)))?;
                    let entry = self.natives.iter()
                        .find(|e| e.name == *native_name)
                        .ok_or_else(|| VmError::InvalidBytecode(format!("unregistered native: {}", native_name)))?;
                    let result = (entry.func)(&args)?;
                    self.stack.push(result);
                }
                Op::Return => {
                    let result = self.pop()?;
                    let old_frame = self.frames.pop().unwrap();
                    self.stack.truncate(old_frame.base);
                    if self.frames.is_empty() {
                        return Ok(VmState::Finished(result));
                    }
                    self.stack.push(result);
                }

                Op::Yield => {
                    let output = self.pop()?;
                    return Ok(VmState::Yielded(output));
                }

                Op::Pop => { self.pop()?; }
                Op::Dup => {
                    let val = self.stack.last()
                        .ok_or(VmError::StackUnderflow)?
                        .clone();
                    self.stack.push(val);
                }

                Op::NewStruct(template_idx) => {
                    let template = &self.module.chunks[chunk_idx]
                        .struct_templates[template_idx as usize];
                    let n = template.field_names.len();
                    if self.stack.len() < n {
                        return Err(VmError::StackUnderflow);
                    }
                    let values: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
                    let fields: Vec<(String, Value)> = template.field_names.iter()
                        .zip(values)
                        .map(|(name, val)| (name.clone(), val))
                        .collect();
                    self.stack.push(Value::Struct {
                        type_name: template.type_name.clone(),
                        fields,
                    });
                }
                Op::NewEnum(enum_const, var_const, arg_count) => {
                    let type_name = match &self.module.chunks[chunk_idx].constants[enum_const as usize] {
                        Value::Str(s) => s.clone(),
                        _ => return Err(VmError::InvalidBytecode(String::from("enum name not a string"))),
                    };
                    let variant = match &self.module.chunks[chunk_idx].constants[var_const as usize] {
                        Value::Str(s) => s.clone(),
                        _ => return Err(VmError::InvalidBytecode(String::from("variant name not a string"))),
                    };
                    let n = arg_count as usize;
                    let fields: Vec<Value> = if n > 0 {
                        self.stack.drain(self.stack.len() - n..).collect()
                    } else {
                        Vec::new()
                    };
                    self.stack.push(Value::Enum { type_name, variant, fields });
                }
                Op::NewArray(count) => {
                    let n = count as usize;
                    let elements: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
                    self.stack.push(Value::Array(elements));
                }
                Op::NewTuple(count) => {
                    let n = count as usize;
                    let elements: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
                    self.stack.push(Value::Tuple(elements));
                }
                Op::WrapSome => {
                    // In our representation, Some(v) is just v. None is Value::None.
                    // WrapSome is a no-op for the value itself.
                }
                Op::PushNone => {
                    self.stack.push(Value::None);
                }

                Op::GetField(name_const) => {
                    let container = self.pop()?;
                    let field_name = match &self.module.chunks[chunk_idx].constants[name_const as usize] {
                        Value::Str(s) => s.clone(),
                        _ => return Err(VmError::InvalidBytecode(String::from("field name not a string"))),
                    };
                    match container {
                        Value::Struct { type_name, fields } => {
                            let val = fields.iter()
                                .find(|(n, _)| n == &field_name)
                                .map(|(_, v)| v.clone())
                                .ok_or(VmError::FieldNotFound(type_name, field_name))?;
                            self.stack.push(val);
                        }
                        v => return Err(VmError::TypeError(format!("cannot access field on {}", v.type_name()))),
                    }
                }
                Op::GetIndex => {
                    let index = self.pop()?;
                    let container = self.pop()?;
                    match (container, index) {
                        (Value::Array(arr), Value::Int(i)) => {
                            let len = arr.len();
                            if i < 0 || i as usize >= len {
                                return Err(VmError::IndexOutOfBounds(i, len));
                            }
                            self.stack.push(arr[i as usize].clone());
                        }
                        (c, i) => return Err(VmError::TypeError(format!("cannot index {} with {}", c.type_name(), i.type_name()))),
                    }
                }
                Op::GetTupleField(idx) => {
                    let container = self.pop()?;
                    match container {
                        Value::Tuple(elems) => {
                            let i = idx as usize;
                            if i >= elems.len() {
                                return Err(VmError::IndexOutOfBounds(i as i64, elems.len()));
                            }
                            self.stack.push(elems[i].clone());
                        }
                        v => return Err(VmError::TypeError(format!("cannot tuple-index {}", v.type_name()))),
                    }
                }
                Op::GetEnumField(idx) => {
                    let container = self.pop()?;
                    match container {
                        Value::Enum { fields, .. } => {
                            let i = idx as usize;
                            if i >= fields.len() {
                                return Err(VmError::IndexOutOfBounds(i as i64, fields.len()));
                            }
                            self.stack.push(fields[i].clone());
                        }
                        v => return Err(VmError::TypeError(format!("cannot enum-field {}", v.type_name()))),
                    }
                }

                // -- Type predicates (push bool, no jump) --

                Op::IsEnum(enum_const, var_const) => {
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?;
                    let expected_type = match &self.module.chunks[chunk_idx].constants[enum_const as usize] {
                        Value::Str(s) => s.as_str(),
                        _ => return Err(VmError::InvalidBytecode(String::from("enum const not string"))),
                    };
                    let expected_var = match &self.module.chunks[chunk_idx].constants[var_const as usize] {
                        Value::Str(s) => s.as_str(),
                        _ => return Err(VmError::InvalidBytecode(String::from("variant const not string"))),
                    };
                    let matches = matches!(
                        val,
                        Value::Enum { type_name, variant, .. }
                            if type_name == expected_type && variant == expected_var
                    );
                    self.stack.push(Value::Bool(matches));
                }
                Op::IsStruct(type_const) => {
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?;
                    let expected = match &self.module.chunks[chunk_idx].constants[type_const as usize] {
                        Value::Str(s) => s.as_str(),
                        _ => return Err(VmError::InvalidBytecode(String::from("type const not string"))),
                    };
                    let matches = matches!(val, Value::Struct { type_name, .. } if type_name == expected);
                    self.stack.push(Value::Bool(matches));
                }

                Op::IntToFloat => {
                    let val = self.pop()?;
                    match val {
                        Value::Int(i) => self.stack.push(Value::Float(i as f64)),
                        v => return Err(VmError::TypeError(format!("cannot cast {} to f64", v.type_name()))),
                    }
                }
                Op::FloatToInt => {
                    let val = self.pop()?;
                    match val {
                        Value::Float(f) => self.stack.push(Value::Int(f as i64)),
                        v => return Err(VmError::TypeError(format!("cannot cast {} to i64", v.type_name()))),
                    }
                }

                Op::Trap(msg_const) => {
                    let msg = match &self.module.chunks[chunk_idx].constants[msg_const as usize] {
                        Value::Str(s) => s.clone(),
                        _ => String::from("trap"),
                    };
                    return Err(VmError::Trap(msg));
                }
            }
        }
    }

    fn pop(&mut self) -> Result<Value, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn binary_op<F>(&mut self, f: F) -> Result<(), VmError>
    where
        F: FnOnce(Value, Value) -> Result<Value, VmError>,
    {
        let b = self.pop()?;
        let a = self.pop()?;
        let result = f(a, b)?;
        self.stack.push(result);
        Ok(())
    }

    fn binary_arith(
        &mut self,
        int_op: fn(i64, i64) -> i64,
        float_op: fn(f64, f64) -> f64,
    ) -> Result<(), VmError> {
        let b = self.pop()?;
        let a = self.pop()?;
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(int_op(x, y))),
            (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(float_op(x, y))),
            (a, b) => return Err(VmError::TypeError(format!("type mismatch: {} and {}", a.type_name(), b.type_name()))),
        }
        Ok(())
    }

    fn compare_op<F>(&mut self, pred: F) -> Result<(), VmError>
    where
        F: FnOnce(core::cmp::Ordering) -> bool,
    {
        let b = self.pop()?;
        let a = self.pop()?;
        let ord = match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => x.cmp(y),
            (Value::Float(x), Value::Float(y)) => {
                x.partial_cmp(y).unwrap_or(core::cmp::Ordering::Equal)
            }
            (Value::Str(x), Value::Str(y)) => x.cmp(y),
            _ => return Err(VmError::TypeError(format!("cannot compare {} and {}", a.type_name(), b.type_name()))),
        };
        self.stack.push(Value::Bool(pred(ord)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn run_program(src: &str, args: &[Value]) -> Result<VmState, VmError> {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module)?;
        vm.call(args)
    }

    fn run_expect(src: &str, args: &[Value]) -> Value {
        match run_program(src, args).unwrap() {
            VmState::Finished(v) => v,
            VmState::Yielded(v) => panic!("unexpected yield: {:?}", v),
            VmState::Reset => panic!("unexpected reset"),
        }
    }

    #[test]
    fn eval_literal() {
        let val = run_expect("fn main() -> i64 { 42 }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_add() {
        let val = run_expect("fn main() -> i64 { 10 + 32 }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_arithmetic() {
        let val = run_expect("fn main() -> i64 { (2 + 3) * 4 - 1 }", &[]);
        assert_eq!(val, Value::Int(19));
    }

    #[test]
    fn eval_comparison() {
        let val = run_expect("fn main() -> bool { 10 > 5 }", &[]);
        assert_eq!(val, Value::Bool(true));
    }

    #[test]
    fn eval_logical_and() {
        let val = run_expect("fn main() -> bool { true and false }", &[]);
        assert_eq!(val, Value::Bool(false));
    }

    #[test]
    fn eval_logical_or() {
        let val = run_expect("fn main() -> bool { false or true }", &[]);
        assert_eq!(val, Value::Bool(true));
    }

    #[test]
    fn eval_negation() {
        let val = run_expect("fn main() -> i64 { -42 }", &[]);
        assert_eq!(val, Value::Int(-42));
    }

    #[test]
    fn eval_not() {
        let val = run_expect("fn main() -> bool { not true }", &[]);
        assert_eq!(val, Value::Bool(false));
    }

    #[test]
    fn eval_if_true() {
        let val = run_expect("fn main() -> i64 { if true { 1 } else { 2 } }", &[]);
        assert_eq!(val, Value::Int(1));
    }

    #[test]
    fn eval_if_false() {
        let val = run_expect("fn main() -> i64 { if false { 1 } else { 2 } }", &[]);
        assert_eq!(val, Value::Int(2));
    }

    #[test]
    fn eval_let_binding() {
        let val = run_expect("fn main() -> i64 { let x = 10; let y = 32; x + y }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_function_call() {
        let val = run_expect(
            "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { double(21) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_nested_calls() {
        let val = run_expect(
            "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { double(double(10)) + 2 }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_with_args() {
        let val = run_expect(
            "fn main(x: i64) -> i64 { x + 1 }",
            &[Value::Int(41)],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_for_range() {
        let val = run_expect(
            "fn main() -> i64 { let sum = 0; for i in 0..5 { let x = sum + i; } sum }",
            &[],
        );
        // Lexical scoping: inner `let x` shadows but does not mutate outer `sum`.
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn eval_string_literal() {
        let val = run_expect("fn main() -> String { \"hello\" }", &[]);
        assert_eq!(val, Value::Str(String::from("hello")));
    }

    #[test]
    fn eval_float_arithmetic() {
        let val = run_expect("fn main() -> f64 { 1.5 + 2.5 }", &[]);
        assert_eq!(val, Value::Float(4.0));
    }

    #[test]
    fn eval_cast_int_to_float() {
        let val = run_expect("fn main() -> f64 { 42 as f64 }", &[]);
        assert_eq!(val, Value::Float(42.0));
    }

    #[test]
    fn eval_cast_float_to_int() {
        let val = run_expect("fn main() -> i64 { 3.7 as i64 }", &[]);
        assert_eq!(val, Value::Int(3));
    }

    #[test]
    fn eval_struct_init_and_field() {
        let val = run_expect(
            "fn main() -> i64 { let p = Point { x: 10, y: 32 }; p.x + p.y }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_enum_variant() {
        let val = run_expect(
            "fn main() -> i64 { let c = Color::Red(); 42 }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_array_literal_and_index() {
        let val = run_expect(
            "fn main() -> i64 { let arr = [10, 20, 30]; arr[1] }",
            &[],
        );
        assert_eq!(val, Value::Int(20));
    }

    #[test]
    fn eval_yield_and_resume() {
        let src = "loop main(input: i64) -> i64 { let input = yield input * 2; input }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();

        // First call: main(5) -> yields 5 * 2 = 10.
        match vm.call(&[Value::Int(5)]).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(10)),
            other => panic!("expected yield, got {:?}", other),
        }

        // Resume with 7: continues after yield, sets input=7, reaches Reset.
        match vm.resume(Value::Int(7)).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }

        // Resume after Reset with 7: restarts stream, yields 7 * 2 = 14.
        match vm.resume(Value::Int(7)).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(14)),
            other => panic!("expected yield, got {:?}", other),
        }

        // Resume with 0: reaches Reset.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }

        // Resume after Reset with 0: yields 0 * 2 = 0.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(0)),
            other => panic!("expected yield, got {:?}", other),
        }
    }

    #[test]
    fn eval_multiheaded_literal() {
        let val = run_expect(
            "fn classify(0) -> String { \"zero\" }\nfn classify(x: i64) -> String { \"other\" }\nfn main() -> String { classify(0) }",
            &[],
        );
        assert_eq!(val, Value::Str(String::from("zero")));
    }

    #[test]
    fn eval_multiheaded_fallthrough() {
        let val = run_expect(
            "fn classify(0) -> String { \"zero\" }\nfn classify(x: i64) -> String { \"other\" }\nfn main() -> String { classify(5) }",
            &[],
        );
        assert_eq!(val, Value::Str(String::from("other")));
    }

    #[test]
    fn eval_pipeline() {
        let val = run_expect(
            "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { 21 |> double() }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_match_literal() {
        let val = run_expect(
            "fn main() -> String { let x = 1; match x { 1 => \"one\", 2 => \"two\", _ => \"other\" } }",
            &[],
        );
        assert_eq!(val, Value::Str(String::from("one")));
    }

    #[test]
    fn eval_match_wildcard() {
        let val = run_expect(
            "fn main() -> String { let x = 99; match x { 1 => \"one\", _ => \"other\" } }",
            &[],
        );
        assert_eq!(val, Value::Str(String::from("other")));
    }

    #[test]
    fn eval_division_by_zero() {
        let result = run_program("fn main() -> i64 { 1 / 0 }", &[]);
        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
    fn eval_index_out_of_bounds() {
        let result = run_program("fn main() -> i64 { let a = [1, 2]; a[5] }", &[]);
        assert!(matches!(result, Err(VmError::IndexOutOfBounds(5, 2))));
    }

    #[test]
    fn eval_native_function() {
        let src = "use math::add_one\nfn main(x: i64) -> i64 { math::add_one(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
        vm.register_native("math::add_one", |args| {
            match &args[0] {
                Value::Int(x) => Ok(Value::Int(x + 1)),
                _ => Err(VmError::TypeError(String::from("expected Int"))),
            }
        });
        match vm.call(&[Value::Int(41)]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn eval_guard_clause() {
        let val = run_expect(
            "fn abs(x: i64) -> i64 when x < 0 { -x }\nfn abs(x: i64) -> i64 { x }\nfn main() -> i64 { abs(-5) + abs(3) }",
            &[],
        );
        assert_eq!(val, Value::Int(8));
    }

    #[test]
    fn eval_string_concat() {
        let val = run_expect(
            "fn main() -> String { \"hello\" + \" world\" }",
            &[],
        );
        assert_eq!(val, Value::Str(String::from("hello world")));
    }
}
