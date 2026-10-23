pub mod opcode;
pub mod compiler_runners;

use std::collections::HashSet;

use rand::{RngExt, SeedableRng};
use rand::rngs::StdRng;

use crate::errors::{ErrorStatus, ErrorType, get_error_map, is_warning};
use crate::vm::compiler_runners::compile_program;
use crate::vm::opcode::Value;
use crate::vm::opcode::Opcode;
use std::collections::HashMap;
use crate::compiler::parser::Parser;
use crate::compiler::{LoopType, CallableType, Compiler};
use crate::compiler::decompile;
use crate::scanner::Statement;
use crate::compiler::ast::Stmt;
use crate::scanner::Token;


pub struct RandomState {
    rng: StdRng,
    last_value: f64,
}

impl RandomState {
/// Initialize with a default starting seed
    pub fn new(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            last_value: 0.0,
        }
    }

    pub fn rnd(&mut self, e: f64) -> f64 {
        // Condition 1: e == 0 (Repeat last number)
        if e == 0.0 {
            return self.last_value;
        }

        // Condition 2: e < 0 (Reseed)
        if e < 0.0 {
            // Convert the absolute value to a u64 to use as a deterministic seed
            let seed = e.abs().to_bits(); 
            self.rng = StdRng::seed_from_u64(seed);
        }

        let magnitude = e.abs();

        // Generate the new number using the updated rand 0.9+ methods
        let result = if magnitude > 1.0 {
            // Condition 3: magnitude > 1 (Integer from 1 to e)
            let upper_bound = magnitude.trunc() as u64;
            self.rng.random_range(1..=upper_bound) as f64
        } else {
            // Condition 4: magnitude between 0 and 1 (Float >= 0 and < 1)
            self.rng.random::<f64>()
        };

        self.last_value = result;
        result
    }
}



/// Interestingly equal for floats
pub fn interesting_float_comparison(a: f64, b: f64) -> bool {
    if a == b {
        return true
    }
    let epsilon = 1e-15;
    let abs_a = a.abs();
    let abs_b = b.abs();
    let diff = (a - b).abs();
    // Scale the epsilon relative to the largest of the two numbers
    // to account for floating point scale (larger numbers have larger absolute gaps)
    let largest = if abs_a > abs_b { abs_a } else { abs_b };
    diff <= largest * epsilon
}

/// Interestingly equal for strings
pub fn interestingly_equal_str(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// Returns true if a string is combined with a numeric in a binary expression:
pub fn mixed_string_numeric(left: &Value, right: &Value, error_status: &mut ErrorStatus) -> bool { 
    match left {
        Value::String(_) => {
            match right {
                Value::String(_) => {}
                _ => { error_status.error_type = ErrorType::StringExpressionNeeded; return true }
            }
        }
        _ => {}
    }
    match right {
        Value::String(_) => {
            match left {
                Value::String(_) => {}
                _ => { error_status.error_type = ErrorType::StringExpressionNeeded; return true  }
            }
        }
        _ => {}
    }
    false
}

#[derive(Default, Debug, Clone)]
pub struct BasicArray {
    pub bounds: Vec<usize>, 
    pub data: Vec<Value>,
}

impl BasicArray {
    pub fn new(bounds: Vec<usize>, default_value: Value) -> Self {
        let total_size = bounds.iter().product();
        BasicArray {
            bounds,
            data: vec![default_value; total_size], 
        }
    }

    pub fn calculate_flat_index(&self, indices: &[usize]) -> Option<usize> {
        if indices.len() != self.bounds.len() { return None; }
        let mut flat_index = 0;
        let mut multiplier = 1;
        for i in (0..self.bounds.len()).rev() {
            if indices[i] >= self.bounds[i] { return None; }
            flat_index += indices[i] * multiplier;
            multiplier *= self.bounds[i];
        }
        Some(flat_index)
    }
}

#[derive(Debug)]
pub struct InLineData {
    pub line_number: usize,
    pub value: Value,
}

#[derive(Debug)]
pub struct Loop {
    pub on_error_jump: OnErrorJump,
    pub on_error: bool,
    pub loop_ip: usize,
    pub loop_type: LoopType,
    pub loop_params: LoopParams,
    pub loop_id: usize,
}

#[derive(Debug)]
pub enum LoopParams {
    None,
    For { control_var: String, start: Value, finish: f64, step: f64 }, // Simpler on the back to represent start/finish/step as f64
}

#[derive(Debug)]
pub struct Callable {
    pub callable_type: CallableType,
    pub args: Vec<Token>,
    pub procedure_return_args: Vec<Token>,
}

#[derive(Default, Debug, Clone)]
pub struct OnErrorJump {
    pub scope_depth: usize,
    pub jump_to: usize,
}

#[derive(Debug)]
pub struct CallFrame {
    pub caller_ip: usize,
    pub name: String,
    pub callable_type: CallableType,
    pub locals: HashMap<String, Value>,
    pub local_arrays: HashMap<String, BasicArray>,
    pub globalized: HashSet<String>,
    pub loop_stack: Vec<Loop>, 
    pub procedure_call_receive_args: Vec<Token>,
    pub on_error_jump: OnErrorJump,
    pub on_error: bool,
}

pub struct VM {
    pub stop: bool,
    pub shutdown: bool,
    pub parser: Parser,
    pub degrees: bool,
    pub random_state: RandomState,
    pub stack: Vec<Value>,
    pub globals: HashMap<String, Value>,
    pub global_arrays: HashMap<String, BasicArray>,
    pub error_status: ErrorStatus, 
    pub entry_ip: usize,
    pub call_stack: Vec<CallFrame>,
    pub callables: HashMap<String, Callable>,
    pub inline_data: Vec<InLineData>,
    pub inline_data_pointer: usize,
}

/// Catches zero-value denominators.
fn divide_by_zero_protection(b: Value) -> bool {
    match b {
        Value::Integer(val) => if val == 0 { true } else { false },
        Value::Float(val) => if val == 0.0 { true } else { false },
        _ => { false },
    }
}

/// Evaluates any numeric value to boolean according to RM Basic's sense of truthiness.
fn is_truthy(v: Value, error_status: &mut ErrorStatus) -> bool {
    match v {
        Value::Integer(val) => if val == 0 { return false } else { return true },
        Value::Float(val) => if val == 0.0 { return false } else { return true },
        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; return false },
    } 
}

/// Represent a Rust boolean as an RM Basic TRUE/FALSE integer.
fn represent_bool(v: bool) -> Value {
    if v { Value::Integer(-1) } else { Value::Integer(0) }
}

impl VM {
    pub fn new(parser: Parser) -> Self {
        VM {
            stop: false,
            shutdown: false,
            parser: parser,
            random_state: RandomState::new(123456),
            stack: Vec::with_capacity(256), 
            globals: HashMap::new(),
            global_arrays: HashMap::new(),
            error_status: ErrorStatus{
                error_type: ErrorType::Ok,
                line_number: 0,
                token_sequence: 0,
                statement_number: 0,
            },
            degrees: true,
            entry_ip: 0,
            call_stack: Vec::new(),
            callables: HashMap::new(),
            inline_data: Vec::new(),
            inline_data_pointer: 0,
        }
    }

    /// Returns the current scope depth (call_frame.len() +
    /// all loop_stack.len() in every call_frame)
    pub fn scope_depth(&self) -> usize {
        let mut depth = self.call_stack.len();
        for frame in self.call_stack.iter() {
            depth += frame.loop_stack.len();
        }
        depth
    }

    /// Gets on_error status according to the highest available on_error flag
    pub fn get_on_error_status(&self) -> (bool, OnErrorJump) {
        let idx = self.call_stack.len()-1;
        let current_call_frame = &self.call_stack[idx];
        if current_call_frame.loop_stack.len() == 0 {
            (current_call_frame.on_error, current_call_frame.on_error_jump.clone())
        } else {
            let loop_idx = current_call_frame.loop_stack.len()-1;
            (current_call_frame.loop_stack[loop_idx].on_error, current_call_frame.on_error_jump.clone())
        }
    }

    /// Sets on_error status in the highest available on_error flag
    pub fn set_on_error_status(&mut self, on_error: bool, on_error_jump: OnErrorJump) {
        let idx = self.call_stack.len()-1;
        let current_call_frame = &mut self.call_stack[idx];
        if current_call_frame.loop_stack.len() == 0 {
            current_call_frame.on_error = on_error;
            current_call_frame.on_error_jump = on_error_jump;
        } else {
            let loop_idx = current_call_frame.loop_stack.len()-1;
            current_call_frame.loop_stack[loop_idx].on_error = on_error;
            current_call_frame.loop_stack[loop_idx].on_error_jump = on_error_jump
        }
    }

    /// Pushes a loop to the current call stack
    pub fn push_loop(&mut self, lp: Loop) {
        let idx = self.call_stack.len()-1;
        let (on_error, on_error_jump) = self.get_on_error_status();
        let updated_loop = Loop {
            loop_ip: lp.loop_ip,
            loop_type: lp.loop_type,
            loop_params: lp.loop_params,
            loop_id: lp.loop_id,
            on_error: on_error,
            on_error_jump: on_error_jump,
        };
        self.call_stack[idx].loop_stack.push(updated_loop)
    }

    /// Pops a loop from the current call stack
    pub fn pop_loop(&mut self) -> Option<Loop> {
        let idx = self.call_stack.len()-1;
        self.call_stack[idx].loop_stack.pop()
    }

    /// Returns length of current call stack's loop stack
    pub fn loop_stack_len(&self) -> usize {
        let idx = self.call_stack.len()-1;
        self.call_stack[idx].loop_stack.len()       
    }

    /// Get variable value
    pub fn get_var(&self, var_name: &String) -> Option<&Value> {
        let idx = self.call_stack.len()-1;
        if self.call_stack[idx].globalized.contains(var_name) {
            // is global
            self.globals.get(var_name)
        } else {
            // is local
            self.call_stack[idx].locals.get(var_name)  
        }      
    }

    /// Set variable value
    pub fn set_var(&mut self, var_name: String, value: Value) {
        let idx = self.call_stack.len()-1;
        if self.call_stack[idx].globalized.contains(&var_name) {
            // is global
            self.globals.insert(var_name, value);
        } else {
            // is local
            self.call_stack[idx].locals.insert(var_name, value);  
        }      
    }

    /// Generate an error message based on the current error_status
    pub fn get_error_msg(&self) -> String {

        let mut msg = "".to_string();
        let error_map = get_error_map();

        if self.error_status.error_type != ErrorType::Ok {
            let error_source = self.parser.get_program_line(
                self.error_status.line_number, 
                true, 
                self.error_status.statement_number, 
                self.error_status.token_sequence
            );
            if self.error_status.line_number == 0 {
                // Immediate mode error
                msg = format!(
                    "{}:\n{}\n", 
                    error_map[&self.error_status.error_type],
                    error_source
                );
            } else {
                // Program mode error
                msg = format!(
                    "{} in line {}:\n{}\n", 
                    error_map[&self.error_status.error_type], 
                    self.error_status.line_number,
                    error_source
                );
            }
        }

        return msg
    }

    /// New execution entry point
    pub fn execute(&mut self, error_status: &mut ErrorStatus) {

        // Reset stacks and signals
        self.call_stack = Vec::new();
        self.stop = false;

        // Run ye olde Compyler
        let mut compiler = Compiler::new();
        self.entry_ip = compile_program(&mut self.parser, &mut compiler, error_status);

        // Don't interpret if the compiler raised an error:
        if error_status.error_type == ErrorType::Ok {
            decompile(compiler.chunk.clone(), self.entry_ip);
            // Handle inline data - need to sort it first because it might have
            // got jumbled around during compilation:
            self.inline_data = Vec::new();
            self.inline_data_pointer = 0;
            let mut line_numbers: Vec<usize> = compiler.inline_data.keys().copied().collect();
            line_numbers.sort();
            for l in line_numbers.iter() {
                for v in compiler.inline_data.get(l).unwrap() {
                    self.inline_data.push(InLineData { line_number: *l, value: v.clone() });
                }
            }
            // Go!!!
            let _result = self.interpret(&compiler, error_status);
        }

        self.error_status = error_status.clone();        
    }

    /// Handle a line of input from the REPL
    pub fn handle_repl_input(&mut self, stmts: Vec<Statement>) {
        let immediate_mode = self.parser.load_program_line(stmts.clone());
        
        let mut error_status = ErrorStatus{
            error_type: ErrorType::Ok,
            line_number: 0,
            statement_number: 0,
            token_sequence: 0,
        };
        self.error_status = error_status.clone();
        
        if immediate_mode {
            self.execute(&mut error_status)
        }
    }

    /// Executes a chunk of bytecode, but as we're bunging the entire program into a single chunk
    /// it will execute the entire program.
    pub fn interpret(&mut self, compiler: &Compiler, error_status: &mut ErrorStatus) -> Value {
        let mut ip = self.entry_ip;
        self.call_stack.push(CallFrame { 
            caller_ip: self.entry_ip,
            callable_type: CallableType::None,
            name: String::from("___main___"), 
            loop_stack: Vec::new() ,
            locals: HashMap::new(),
            local_arrays: HashMap::new(),
            globalized: HashSet::new(),
            procedure_call_receive_args: Vec::new(),
            on_error_jump: OnErrorJump { scope_depth: self.scope_depth(), jump_to: 0 },
            on_error: false,
        });

        loop {
            // Intercept and print warning but continue execution:
            if is_warning(error_status.error_type) {
                let error_map = get_error_map();
                println!("{}", error_map[&error_status.error_type]);
                error_status.error_type = ErrorType::Ok;
            }

            // Break on error or run alternate code:
            if error_status.error_type != ErrorType::Ok {
                let (on_error, ..) = self.get_on_error_status();

                if !on_error {
                    // ON ERROR not set, so unwind to the base call_stack so the user can check vars etc:
                    self.stop = true;
                    while self.call_stack.len() > 1 {
                        _ = self.call_stack.pop();
                        break
                    }
                } else {
                    // ON ERROR set, so unwind until we reach the scope of the
                    // alternate code and then run it:
                    let mut idx = self.call_stack.len()-1;
                    let target_scope_depth = self.call_stack[idx].on_error_jump.scope_depth;
                    let jump_to_ip = self.call_stack[idx].on_error_jump.jump_to;
                    while self.scope_depth() > target_scope_depth {
                        idx = self.call_stack.len()-1;
                        if self.call_stack[idx].loop_stack.len() > 0 {
                            _ = self.call_stack[idx].loop_stack.pop()
                        } else {
                            _ = self.call_stack.pop()
                        }
                    }
                    ip = jump_to_ip;
                    error_status.error_type = ErrorType::Ok;
                    continue
                }
            }

            if self.stop {
                break
            }

            let (chunk_line_number, chunk_statement_number, instruction) = &compiler.chunk.code[ip];
            ip += 1;
            error_status.line_number = *chunk_line_number;
            error_status.statement_number = *chunk_statement_number;

            match instruction {
                Opcode::OnGoto(lines) => {
                    if let Some(value) = self.stack.pop() {
                        let mut result: usize = 0;
                        match value {
                            Value::Integer(v) => result = v as usize,
                            Value::Float(v) => result  = v as usize,
                            _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                        };
                        if result >= 1 && result <= lines.len() {
                            // We can't use label patching for ON GOTO so we have to look up the ip's at runtime:
                            let jump_to_line = lines[result-1] as usize;
                            if let Some(jump_to_ip) = compiler.chunk.program_location_to_ip.get(&(jump_to_line as u32, 0)) {
                                ip = *jump_to_ip;
                                continue
                            } else {
                                error_status.error_type = ErrorType::SpecifiedLineNotFound
                            }
                        } else {
                            continue
                        }
                    } else {
                        panic!("ON GOTO found no result pushed to stack");
                    }
                }
                Opcode::OnGosub(labels) => {
                    if let Some(value) = self.stack.pop() {
                        let mut result: usize = 0;
                        match value {
                            Value::Integer(v) => result = v as usize,
                            Value::Float(v) => result  = v as usize,
                            _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                        };
                        if result >= 1 && result <= labels.len() {
                            // We can't use label patching for ON GOTO so we have to look up the ip's at runtime:
                            let jump_to_label = &labels[result-1];
                            let full_label = compiler.infer_callable_label(CallableType::Subroutine, jump_to_label.to_string());
                            if let Some(jump_to_ip) = compiler.labels.get(&full_label) {
                                // push call
                                let locals: HashMap<String, Value> = HashMap::new();
                                let current_call_stack = &self.call_stack[self.call_stack.len()-1];
                                let (on_error, on_error_jump) = self.get_on_error_status();
                                self.call_stack.push(CallFrame { 
                                    caller_ip: ip-1, // <-- because we didn't push a PushCall opcode
                                    name: jump_to_label.clone(),
                                    callable_type: CallableType::Subroutine,
                                    loop_stack: Vec::new(),
                                    locals: locals,
                                    local_arrays: HashMap::new(),
                                    globalized: HashSet::new(),
                                    procedure_call_receive_args: Vec::new(),
                                    on_error_jump: on_error_jump,
                                    on_error: on_error
                                });                               
                                ip = *jump_to_ip;
                                continue
                            } else {
                                error_status.error_type = ErrorType::UnknownSubroutine
                            }
                        } else {
                            continue
                        }
                    } else {
                        panic!("ON GOSUB found no result pushed to stack");
                    }
                }
                Opcode::OnError(v) => {
                    if *v {
                        self.set_on_error_status(true, OnErrorJump { scope_depth: self.scope_depth(), jump_to: ip+1 })
                    } else {
                        self.set_on_error_status(false, OnErrorJump { scope_depth:0, jump_to: 0 })
                    }
                }
                Opcode::Restore(line) => {
                    self.inline_data_pointer = 0;
                    if *line == 0 || self.inline_data.len() == 0 { continue }
                    loop {
                        if self.inline_data_pointer >= self.inline_data.len()-1 { break }
                        if self.inline_data[self.inline_data_pointer].line_number >= *line { break }
                        self.inline_data_pointer += 1
                    }
                }
                Opcode::Read => {
                    if self.inline_data_pointer >= self.inline_data.len() {
                        error_status.error_type = ErrorType::NoMoreDataToBeRead;
                        continue
                    }
                    let value = &self.inline_data[self.inline_data_pointer].value;
                    self.stack.push(value.clone());
                    self.inline_data_pointer += 1;
                }
                Opcode::ClearVars => {
                    self.globals = HashMap::new();
                    self.global_arrays = HashMap::new();
                }
                Opcode::Bye => {
                    // End program execution and send shutdown message to app
                    self.shutdown = true;
                    return Value::Null
                }
                Opcode::End => return Value::Null,
                Opcode::Jump(jump_to_ip) => ip = *jump_to_ip,
                Opcode::JumpIfFalse(jump_to_ip) => {
                    let v = self.stack.pop().unwrap();
                    if !is_truthy(v, error_status) { ip = *jump_to_ip }
                }
                Opcode::JumpToLine(line) => panic!("Unpatched JumpToLine {}", line),
                Opcode::JumpToLabel { label, callable_type } => panic!("Unpatched JumpToLabel: {}, {:?}", label, callable_type),
                Opcode::JumpIfFalseToLabel(label) => panic!("Unpatched JumpIfFalseToLabel {}", label),
                Opcode::Global(label) => {
                    let idx = self.call_stack.len()-1;
                    if label.starts_with("()") {
                        // Array
                        // Ensure the array is not already in call_frame.local_arrays before adding
                        // to globalized:
                        match self.call_stack[idx].local_arrays.get(label) {
                            Some(_) => error_status.error_type = ErrorType::VariableUsedAsALocal,
                            None => _ = self.call_stack[idx].globalized.insert(label.to_string())
                        }
                    } else {
                        // Variable
                        // Ensure the variable is not already in call_frame.locals before adding
                        // to globalized:
                        match self.call_stack[idx].locals.get(label) {
                            Some(_) => error_status.error_type = ErrorType::VariableUsedAsALocal,
                            None => _ = self.call_stack[idx].globalized.insert(label.to_string())
                        }
                    }
                },
                Opcode::Print => {
                    if let Some(value) = self.stack.pop() {
                        match value {
                            Value::Integer(v) => println!("{}", v),
                            Value::Float(v) => println!("{}", v),
                            Value::String(s) => println!("{}", s),
                            Value::ArrayReference { call_stack_idx, name } => println!("ArrayReference{{call_stack_idx: {:?}, name: {}}}", call_stack_idx, name),
                            Value::Null => println!("null"),
                        }
                    } else {
                        println!("");
                    }
                },
                Opcode::RegisterCallable { callable_type, identifier, args, procedure_return_args }
                    => _ = self.callables.insert(
                        identifier.to_string(), 
                        Callable { 
                            callable_type: callable_type.clone(),
                            args: args.clone() ,
                            procedure_return_args: procedure_return_args.clone()
                }),
                Opcode::Label(_) => {},
                Opcode::PushCall { callable_type, identifier, procedure_call_receive_args } 
                    => {
                        let mut locals: HashMap<String, Value> = HashMap::new();
                        // fetch the callable from the register:
                        match callable_type {
                            CallableType::Function => match self.callables.get(identifier) {
                                None => error_status.error_type = ErrorType::FunctionArrayNotFound,
                                Some(c) => {
                                    // Is a function - unpack args:
                                    for arg in c.args.iter().rev() {
                                        let received_value = self.stack.pop().expect("Stack underflow while fetching function args");
                                        if arg.value.starts_with("()") {
                                            // Require array reference:
                                            match received_value {
                                                Value::ArrayReference { call_stack_idx:_, name } => {
                                                    let idx = self.call_stack.len()-1;
                                                    locals.insert(
                                                        arg.value.clone(),
                                                        Value::ArrayReference{call_stack_idx: idx, name: name}
                                                    );
                                                }
                                                _ => error_status.error_type = ErrorType::BadArgumentForFunction
                                            }
                                        } else {
                                            // Require variable:
                                            match received_value {
                                                Value::ArrayReference{..} => error_status.error_type = ErrorType::BadArgumentForFunction,
                                                _ => {
                                                    locals.insert(
                                                        arg.value.clone(),
                                                        received_value
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            CallableType::Procedure => match self.callables.get(identifier) {
                                None => error_status.error_type = ErrorType::UnknownCommandProcedure,
                                Some(c) => {
                                    // Is a procedure - unpack args:
                                    for arg in c.args.iter().rev() {
                                        let received_value = self.stack.pop().expect("Stack underflow while fetching procedure args");
                                        if arg.value.starts_with("()") {
                                            // Require array reference:
                                            match received_value {
                                                Value::ArrayReference { call_stack_idx:_, name } => {
                                                    let idx = self.call_stack.len()-1;
                                                    locals.insert(
                                                        arg.value.clone(),
                                                        Value::ArrayReference{call_stack_idx: idx, name: name}
                                                    );
                                                }
                                                _ => error_status.error_type = ErrorType::BadArgumentForFunction
                                            }
                                        } else {
                                            // Require variable:
                                            match received_value {
                                                Value::ArrayReference{..} => error_status.error_type = ErrorType::BadArgumentForFunction,
                                                _ => {
                                                    locals.insert(
                                                        arg.value.clone(),
                                                        received_value
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            CallableType::Subroutine => {}
                            _ => panic!("Unsupported callable type")
                        }
                        // push the call:
                        let (on_error, on_error_jump) = self.get_on_error_status();
                        self.call_stack.push(CallFrame { 
                            caller_ip: ip,
                            name: identifier.to_string(),
                            callable_type: callable_type.clone(),
                            loop_stack: Vec::new(),
                            locals: locals,
                            local_arrays: HashMap::new(),
                            globalized: HashSet::new(),
                            procedure_call_receive_args: procedure_call_receive_args.clone(),
                            on_error_jump: on_error_jump,
                            on_error: on_error
                        })
                },
                Opcode::PopCall => {
                    if let Some(call) = self.call_stack.pop() {
                        match call.callable_type {
                            CallableType::Function => {
                                // Handle return value:
                                if let Some(value) = self.stack.pop() {
                                    // First infer value type from Identifier name, then 
                                    // check for type mismatch, or cast if necessary:
                                    let last_char = call.name.chars().last().unwrap();
                                    match last_char {
                                        '%' => {
                                            match value {
                                                Value::Integer(_) => self.stack.push(value),
                                                Value::Float(v) => self.stack.push(Value::Integer(v as i32)),
                                                Value::String(_) => error_status.error_type = ErrorType::NumericExpressionNeeded,
                                                Value::ArrayReference { .. } => panic!("Callable cannot return an array reference"),
                                                Value::Null => panic!("Function pushed Null value to stack")
                                            }
                                        }
                                        '$' => {
                                            match value {
                                                Value::String(_) => self.stack.push(value),
                                                Value::Null => panic!("Function pushed Null value to stack"),
                                                Value::ArrayReference { .. } => panic!("Callable cannot return an array reference"),
                                                _ => error_status.error_type = ErrorType::StringExpressionNeeded,
                                            }
                                         }
                                        _ => {
                                            match value {
                                                Value::Integer(v) => self.stack.push(Value::Float(v as f64)),
                                                Value::Float(_) => self.stack.push(value),
                                                Value::String(_) => error_status.error_type = ErrorType::NumericExpressionNeeded,
                                                Value::ArrayReference { .. } => panic!("Callable cannot return an array reference"),
                                                Value::Null => panic!("Function pushed Null value to stack")
                                            }
                                         }
                                    }
                                } else {
                                    error_status.error_type = ErrorType::StackOverUnderflow
                                }
                            }
                            CallableType::Procedure => { 
                                // Handle procedure return values (if any):
                                let idx = self.call_stack.len()-1;
                                if let Some(call_definition) = self.callables.get(&call.name) {
                                    if call_definition.procedure_return_args.len() != call.procedure_call_receive_args.len() {
                                        panic!("Wrong number of RETURN args"); // <--- TODO: Proper error handling for this
                                    }
                                    for (index, return_arg) in call_definition.procedure_return_args.iter().enumerate() {
                                        let receive_arg = &call.procedure_call_receive_args[index];
                                        // get value of return_arg from call.locals (default value for type if it doesn't exist)
                                        let return_value = match call.locals.get(&return_arg.value) {
                                            Some(return_var) => {
                                                return_var.clone()
                                            }
                                            None => {
                                                error_status.error_type = ErrorType::VariableWithoutAnyValue;
                                                let last_char = &return_arg.value.chars().last().unwrap();
                                                match last_char {
                                                    '%' => Value::Integer(0),
                                                    '$' => Value::String(String::from("")),
                                                    _ => Value::Float(0.0)
                                                }
                                            }
                                        };
                                        // write the value to current locals
                                        let last_char = &receive_arg.value.chars().last().unwrap();
                                        match last_char {
                                            '%' => match return_value {
                                                Value::Integer(_) => _ = self.call_stack[idx].locals.insert(receive_arg.value.clone(), return_value),
                                                Value::Float(v) => _ = self.call_stack[idx].locals.insert(receive_arg.value.clone(), Value::Integer(v as i32)),
                                                Value::String(_) => error_status.error_type = ErrorType::NumericExpressionNeeded,
                                                Value::ArrayReference { .. } => panic!("Can't return array references from a procedure"),
                                                Value::Null => panic!("Trying to assign null value to procedure receive thingy")
                                            }
                                            '$' => match return_value {
                                                Value::Integer(_) => _ = error_status.error_type = ErrorType::StringExpressionNeeded,
                                                Value::Float(_) => _ = error_status.error_type = ErrorType::StringExpressionNeeded,
                                                Value::String(_) => _ = self.call_stack[idx].locals.insert(receive_arg.value.clone(), return_value),
                                                Value::ArrayReference { .. } => panic!("Can't return array references from a procedure"),
                                                Value::Null => panic!("Trying to assign null value to procedure receive thingy")
                                            }
                                            _ => match return_value {
                                                Value::Integer(v) => _ = self.call_stack[idx].locals.insert(receive_arg.value.clone(), Value::Float(v as f64)),
                                                Value::Float(_) => _ = self.call_stack[idx].locals.insert(receive_arg.value.clone(), return_value),
                                                Value::String(_) => error_status.error_type = ErrorType::NumericExpressionNeeded,
                                                Value::ArrayReference { .. } => panic!("Can't return array references from a procedure"),
                                                Value::Null => panic!("Trying to assign null value to procedure receive thingy")
                                            }                                            
                                        }
                                    }
                                } else { panic!("Couldn't get procedure definition from callables") }
                            }
                            CallableType::Subroutine => {}
                            _ => panic!("Callable type not supported: {:?}", call)
                        }
                        ip = call.caller_ip+1;
                    } else {
                        error_status.error_type = ErrorType::StackOverUnderflow
                    }
                },
                Opcode::Endfun => error_status.error_type = ErrorType::NeedResultToExitFunction,
                Opcode::PushLoop(loop_type, control_var, loop_id) => {
                    let (on_error, ..) = self.get_on_error_status();
                    match loop_type {
                        LoopType::Repeat => _ = self.push_loop(Loop{
                            on_error_jump: OnErrorJump { scope_depth: 0, jump_to: 0 }, // <-- updated by push_loop
                            loop_ip: ip, 
                            loop_type: loop_type.clone(), 
                            loop_params: LoopParams::None,
                            loop_id: *loop_id,
                            on_error: on_error,
                        }),
                        LoopType::For => {
                            let step_value = self.stack.pop().expect("Stack underflow while retrieving STEP value");
                            let finish_value = self.stack.pop().expect("Stack underflow while retrieving FOR finish value");
                            let start_value = self.stack.pop().expect("Stack underflow while retrieving FOR start value");
                            let mut finish: f64 = 0.0;
                            match finish_value {
                                Value::Float(v) => finish = v,
                                Value::Integer(v) => finish = v as f64,
                                Value::ArrayReference { .. } => panic!("FOR does not handle array references"),
                                Value::Null => panic!("Finish value in FOR loop cannot be Null"),
                                Value::String(_) => panic!("Finish value in FOR loop cannot be string"),
                            }
                            let mut step: f64 = 0.0;
                            match step_value {
                                Value::Float(v) => step = v,
                                Value::Integer(v) => step = v as f64,
                                Value::Null => {
                                    // If not STEP given, use +1.0 unless start > finish (counting backards)
                                    let mut start: f64 = 0.0;
                                    match start_value {
                                        Value::Float(v) => start = v,
                                        Value::Integer(v) => start = v as f64,
                                        _ => panic!("Start value in FOR loop must be integer or float")
                                    }
                                    if start <= finish {
                                        step = 1.0
                                    } else {
                                        step = -1.0
                                    }
                                },
                                Value::String(_) => panic!("Step value in FOR loop cannot be string"),
                                Value::ArrayReference { .. } => panic!("Print does not handle array references"),
                            }
                            self.push_loop(Loop{
                                on_error_jump: OnErrorJump { scope_depth: 0, jump_to: 0 }, // <-- updated by push_loop
                                loop_ip: ip, 
                                loop_type: loop_type.clone(), 
                                loop_params: LoopParams::For{
                                    control_var: control_var.clone(), 
                                    start: start_value.clone(), 
                                    finish: finish, 
                                    step: step 
                                },
                                loop_id: *loop_id,
                                on_error: on_error,
                            });
                        }
                    }
                },
                Opcode::PopLoop => {
                    if let Some(_) = self.pop_loop() {
                        // Nothing to do because the IP is already at the end of the loop
                    } else {
                        error_status.error_type = ErrorType::StackOverUnderflow
                    }
                },
                Opcode::Loop(end_loop_type) => {
                    if self.loop_stack_len() == 0 {
                        error_status.error_type = ErrorType::UntilWithoutAnyRepeat;
                        continue;
                    }
                    if let Some(lp) = self.pop_loop() {
                        match lp.loop_type {
                            LoopType::Repeat => {
                                // Trap mismatched loop types:
                                if *end_loop_type != LoopType::Repeat {
                                    error_status.error_type = ErrorType::UntilWithoutAnyRepeat
                                } else {
                                    let value = self.stack.pop().expect("Stack underflow");
                                    let r = is_truthy(value, error_status);
                                    if error_status.error_type == ErrorType::Ok {
                                        if r {
                                            // Do nothing...?
                                        } else {
                                            ip = lp.loop_ip;
                                            self.push_loop(lp);
                                        }
                                    }
                                } 
                            }
                            LoopType::For => {
                                // Trap mismatched loop types:
                                if *end_loop_type == LoopType::Repeat {
                                    error_status.error_type = ErrorType::NextWithoutMatchingFor
                                } else {
                                    let mut control_var_name = String::from("");
                                    match &lp.loop_params {
                                        LoopParams::For { control_var , .. } => control_var_name = control_var.to_string(),
                                        LoopParams::None => panic!("Loop parameters missing in FOR loop")
                                    }
                                    let control_value = self.get_var(&control_var_name).expect("FOR loop control variable not found");
                                    let mut control : f64 = 0.0;
                                    match control_value {
                                        Value::Float(v) => control = *v,
                                        Value::Integer(v) => control = *v as f64,
                                        _ => panic!("FOR loop control variable must be integer for float")
                                    }
                                    match &lp.loop_params {
                                        LoopParams::For { 
                                            control_var, 
                                            start:_, 
                                            finish, 
                                            step } => {
                                                let new_v = control + step;
                                                if *step > 0.0 {
                                                    if new_v > *finish {
                                                        // end of loop, don't repush
                                                    } else {
                                                        // loop again, update control var and repush
                                                        ip = lp.loop_ip;
                                                        self.set_var(control_var.clone(), Value::Float(new_v));
                                                        self.push_loop(lp);
                                                    }
                                                } else {
                                                    if new_v < *finish {
                                                        // end of loop, don't repush
                                                    } else {
                                                        // loop again, update control var and repush
                                                        ip = lp.loop_ip;
                                                        self.set_var(control_var.clone(), Value::Float(new_v));
                                                        self.push_loop(lp);
                                                    }
                                                }
                                                
                                            },
                                        LoopParams::None => panic!("Loop parameters missing in FOR loop")
                                    }
                                }
                            }
                        }
                    } else {
                        error_status.error_type = ErrorType::StackOverUnderflow
                    }
                }
                Opcode::SetVar(name_idx) => {
                    let value = self.stack.pop().expect("Stack underflow");
                    // Retrieve the variable's name from the constant pool
                    let var_name = if let Value::String(s) = &compiler.chunk.constants[*name_idx] {
                        s.clone()
                    } else {
                        panic!("Variable name constant must be a string"); // should never happen, so panic
                    };
                    // Cast as necessary and catch illegal assignments:
                    match var_name.chars().last().unwrap() {
                        '%' => {
                            // Integer variable
                            match value {
                                Value::Float(v) => self.set_var(var_name, Value::Integer(v as i32)),
                                Value::Integer(_) => self.set_var(var_name, value),
                                _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                            }
                        },
                        '$' => {
                            // String variable
                            match value {
                                Value::String(_) => self.set_var(var_name, value),
                                _ => error_status.error_type = ErrorType::StringExpressionNeeded
                            }                         
                        },
                        _ => {
                            // Float variable
                            match value {
                                Value::Float(_) => self.set_var(var_name, value),
                                Value::Integer(v) => self.set_var(var_name, Value::Float(v as f64)),
                                _ => error_status.error_type = ErrorType::NumericExpressionNeeded                            
                            }
                        },
                    };
                }
                Opcode::GetVar(name_idx) => {
                    let var_name = if let Value::String(s) = &compiler.chunk.constants[*name_idx] {
                        s.clone()
                    } else {
                        panic!("Variable name constant must be a string");
                    };
                    match self.get_var(&var_name) {
                        Some(val) => self.stack.push(val.clone()),
                        None => {
                            error_status.error_type = ErrorType::VariableWithoutAnyValue;
                            if var_name.ends_with('$') {
                                self.stack.push(Value::String("".to_string()));
                            } else if var_name.ends_with('%') {
                                self.stack.push(Value::Integer(0));
                            } else {
                                self.stack.push(Value::Float(0.0));
                            }
                        }
                    }
                }
                Opcode::Dim(name_idx, arity) => {
                    let name = match &compiler.chunk.constants[*name_idx] {
                        Value::String(s) => s.clone(),
                        _ => panic!("Expected string for array name"),
                    };

                    let idx = self.call_stack.len()-1;
                    let call_frame = &self.call_stack[idx];
                    let is_global = self.call_stack[idx].globalized.contains(&name);
                    let current_array = if is_global { self.global_arrays.get(&name ) }
                        else { call_frame.local_arrays.get(&name)  };

                    match current_array {
                        Some(_) => error_status.error_type = ErrorType::ArrayAlreadyDimensioned,
                        None => {
                            let mut bounds = Vec::new();
                            for _ in 0..*arity {
                                let val = self.stack.pop().unwrap(); // panic here or...?
                                let bound = match val {
                                    Value::Integer(i) => i as usize,
                                    Value::Float(f) => f as usize,
                                    _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; break },
                                };
                                // RM BASIC is 0-indexed, so DIM A(10) means indices 0..10 (size 11)
                                bounds.push(bound + 1); 
                            }
                            match error_status.error_type {
                                ErrorType::Ok => {
                                    bounds.reverse();
                                    let array = match name.chars().last().unwrap() {
                                        '%' => BasicArray::new(bounds, Value::Integer(0)),
                                        '$' => BasicArray::new(bounds, Value::String(String::from(""))),
                                        _ => BasicArray::new(bounds, Value::Float(0.0)),
                                    };
                                    if is_global { _ = self.global_arrays.insert(name, array) } 
                                        else { _ = self.call_stack[idx].local_arrays.insert(name, array)  }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Opcode::SetArray(name_idx, arity) => {
                    let mut target_name = match &compiler.chunk.constants[*name_idx] {
                        Value::String(s) => s.clone(),
                        _ => panic!("Expected string for array name"),
                    };

                    let mut idx = self.call_stack.len()-1;
                    let mut call_frame = &self.call_stack[idx];
                    // Check if array_reference by checking in .locals:
                    match call_frame.locals.get(&target_name) {
                        Some(v) => {
                            match v {
                                Value::ArrayReference { call_stack_idx, name } => {
                                    // Is a reference!  Redirect.
                                    idx = *call_stack_idx;
                                    call_frame = &self.call_stack[idx];
                                    target_name = name.to_string();
                                }
                                _ => panic!("Array reference written to local variables as normal variable")
                            }
                        }
                        None => {}
                    }
                    let is_global = self.call_stack[idx].globalized.contains(&target_name);
                    let current_array = if is_global { self.global_arrays.get(&target_name ) }
                        else { call_frame.local_arrays.get(&target_name)  }; 

                    let value_to_assign = self.stack.pop().unwrap();
                    let mut indices = Vec::new();
                    for _ in 0..*arity {
                        let val = self.stack.pop().unwrap();
                        let idx = match val {
                            Value::Integer(i) => i as usize,
                            Value::Float(f) => f as usize,
                            _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; break },
                        };
                        indices.push(idx);
                    }
                    match error_status.error_type {
                        ErrorType::Ok =>  {
                            indices.reverse();
                            if let Some(array) = current_array {
                                if let Some(flat_idx) = array.calculate_flat_index(&indices) {
                                    // Type checking/casting:
                                    let test_item = &array.data[0];
                                    match value_to_assign {
                                        Value::Float(v) => {
                                            match test_item {
                                                Value::Float(_) => { 
                                                    if is_global {
                                                        self.global_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::Float(v) 
                                                    } else { 
                                                        self.call_stack[idx].local_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::Float(v)
                                                    }
                                                }
                                                Value::Integer(_) => { 
                                                    if is_global {
                                                        self.global_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::Float(v as f64) 
                                                    } else { 
                                                        self.call_stack[idx].local_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::Float(v as f64)
                                                    }
                                                }
                                                Value::String(_) => error_status.error_type = ErrorType::NumericExpressionNeeded,
                                                _ => panic!("Array populated with null values")
                                            }
                                        }
                                        Value::Integer(v) => {
                                            match test_item {
                                                Value::Float(_) => { 
                                                    if is_global {
                                                        self.global_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::Integer(v as i32) 
                                                    } else { 
                                                        self.call_stack[idx].local_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::Integer(v as i32)
                                                    }
                                                }
                                                Value::Integer(_) => { 
                                                    if is_global {
                                                        self.global_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::Integer(v) 
                                                    } else { 
                                                        self.call_stack[idx].local_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::Integer(v as i32)
                                                    }
                                                }
                                                Value::String(_) => error_status.error_type = ErrorType::NumericExpressionNeeded,
                                                _ => panic!("Array populated with null values")
                                            }
                                        }
                                        Value::String(v) => {
                                            match test_item {
                                                Value::Float(_) => error_status.error_type = ErrorType::StringExpressionNeeded,
                                                Value::Integer(_) => error_status.error_type = ErrorType::StringExpressionNeeded,
                                                Value::String(_) => { 
                                                    if is_global {
                                                        self.global_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::String(v.clone()) 
                                                    } else { 
                                                        self.call_stack[idx].local_arrays.get_mut(&target_name).unwrap().data[flat_idx] = Value::String(v.clone())
                                                    }
                                                }
                                                _ => panic!("Array populated with null values")
                                            }
                                        }
                                        _ => panic!("Got a null value to assign to an array")
                                    }
                                } else {
                                    // here!
                                    error_status.error_type = ErrorType::ArraySubscriptIsWrong;
                                }
                            } else {
                                error_status.error_type = ErrorType::ArrayVariableIsWrong; 
                            }
                        }
                        _ => {}
                    }
                }
                Opcode::GetArray(name_idx, arity) => {
                    let mut target_name = match &compiler.chunk.constants[*name_idx] {
                        Value::String(s) => s.clone(),
                        _ => panic!("Expected string for array name"),
                    };

                    let mut idx = self.call_stack.len()-1;
                    let mut call_frame = &self.call_stack[idx];
                    // Check if array_reference by checking in .locals:
                    match call_frame.locals.get(&target_name) {
                        Some(v) => {
                            match v {
                                Value::ArrayReference { call_stack_idx, name } => {
                                    // Is a reference!  Redirect.
                                    idx = *call_stack_idx;
                                    call_frame = &self.call_stack[idx];
                                    target_name = name.to_string()
                                }
                                _ => panic!("Array reference written to local variables as normal variable")
                            }
                        }
                        None => {}
                    }
                    let is_global = self.call_stack[idx].globalized.contains(&target_name);
                    let current_array = if is_global { self.global_arrays.get(&target_name ) }
                        else { call_frame.local_arrays.get(&target_name)  };                   

                    let mut indices = Vec::new();
                    for _ in 0..*arity {
                        let val = self.stack.pop().unwrap();
                        let idx = match val {
                            Value::Integer(i) => i as usize,
                            Value::Float(f) => f as usize,
                            _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; break },
                        };
                        indices.push(idx);
                    }
                    match error_status.error_type {
                        ErrorType::Ok =>  {
                            indices.reverse();
                            if let Some(array) = current_array {
                                if let Some(flat_idx) = array.calculate_flat_index(&indices) {
                                    self.stack.push(array.data[flat_idx].clone());
                                } else {
                                    error_status.error_type = ErrorType::ArraySubscriptIsWrong;
                                }
                            } else {
                                error_status.error_type = ErrorType::ArrayVariableIsWrong;
                            }
                        }
                        _ => {}
                    }
                }
                Opcode::Return => {
                    // The result of the expression should be the last thing on the stack
                    return match self.stack.pop() {
                        Some(v) => v,
                        None => Value::Null,
                    }          
                }
                Opcode::Constant(idx) => {
                    // Read from the chunk's constant pool and push to the stack
                    let value = compiler.chunk.constants[*idx].clone();
                    self.stack.push(value);
                }
                Opcode::SetDeg(v) => self.degrees = *v,
                Opcode::Abs => {
                    let value = self.stack.pop().unwrap();
                    match value {
                        Value::Integer(v) => self.stack.push(Value::Integer(v.abs())),
                        Value::Float(v) => self.stack.push(Value::Float(v.abs())),
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                }
                Opcode::Atn => {
                    let value = self.stack.pop().unwrap();
                    let mut ratio: f64 = 0.0;
                    match value {
                        Value::Integer(v) => ratio = v as f64,
                        Value::Float(v) => ratio = v,
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                    if self.degrees { self.stack.push(Value::Float(ratio.atan().to_degrees())) }
                    else { self.stack.push(Value::Float(ratio.atan())) }
                }
                Opcode::Asc => {
                    let value = self.stack.pop().unwrap();
                    match value {
                        Value::String(v) => {
                            if v.len() == 0 {
                                error_status.error_type = ErrorType::EmptyStringNotAllowed
                            } else {
                                self.stack.push(Value::Integer(v.as_bytes()[0] as i32))
                            }
                        },
                        _ => error_status.error_type = ErrorType::StringExpressionNeeded
                    }
                }
                Opcode::ChrStr => {
                    let value = self.stack.pop().unwrap();
                    match value {
                        Value::Integer(v) => self.stack.push(Value::String((v as u8 as char).to_string())),
                        Value::Float(v) => self.stack.push(Value::String((v as u8 as char).to_string())),
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                }
                Opcode::Cos => {
                    let value = self.stack.pop().unwrap();
                    let mut angle: f64 = 0.0;
                    match value {
                        Value::Integer(v) => angle = v as f64,
                        Value::Float(v) => angle = v,
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                    if self.degrees { self.stack.push(Value::Float(angle.to_radians().cos())) }
                    else { self.stack.push(Value::Float(angle.cos())) }
                }
                Opcode::Exp => {
                    let value = self.stack.pop().unwrap();
                    let mut n : f64 = 0.0;
                    match value {
                        Value::Integer(v) => n = v as f64,
                        Value::Float(v) => n = v,
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                    self.stack.push(Value::Float(n.exp()))
                }
                Opcode::HexStr => {
                    let value = self.stack.pop().unwrap();
                    match value {
                        Value::Integer(v) => self.stack.push(Value::String(format!("{:X}", v))),
                        Value::Float(v) => self.stack.push(Value::String(format!("{:X}", v as i32))),
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                }
                Opcode::Instr(n) => {
                    // We pop them in reverse order, remember?!?!
                    let mut e1_str: Value = Value::Null;
                    let mut e2_str: Value = Value::Null;
                    let mut start: Value = Value::Null;
                    let mut length: Value = Value::Null;
                    match n {
                        2 => {
                            e2_str = self.stack.pop().unwrap();
                            e1_str = self.stack.pop().unwrap();
                        }
                        3 => {
                            start = self.stack.pop().unwrap();
                            e2_str = self.stack.pop().unwrap();
                            e1_str = self.stack.pop().unwrap();
                        }
                        4 => {
                            length = self.stack.pop().unwrap();
                            start = self.stack.pop().unwrap();
                            e2_str = self.stack.pop().unwrap();
                            e1_str = self.stack.pop().unwrap();
                        }
                        _ => { error_status.error_type = ErrorType::InvalidExpressionFound; continue }
                    }
                    let e1_val = match e1_str {
                        Value::String(v) => {v}
                        _ => { error_status.error_type = ErrorType::StringExpressionNeeded; continue }
                    };
                    let e2_val = match e2_str {
                        Value::String(v) => {v}
                        _ => { error_status.error_type = ErrorType::StringExpressionNeeded; continue }
                    };
                    if e1_val.len() == 0 || e2_val.len() == 0 { error_status.error_type = ErrorType::EmptyStringNotAllowed; continue }
                    let result = match n {
                        2 => {
                            match e1_val.find(&e2_val) {
                                Some(pos) => pos,
                                None => 0,
                            }
                        }
                        3 => {
                            let start_val = match start {
                                Value::Integer(v) => v as usize,
                                Value::Float(v) => v as usize,
                                _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                            };
                            match e1_val[start_val..].find(&e2_val).map(|p| p + start_val) {
                                Some(pos) => pos,
                                None => 0,
                            }
                        }
                        4 => {
                            let start_val = match start {
                                Value::Integer(v) => v as usize,
                                Value::Float(v) => v as usize,
                                _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                            };
                            let length_val = match length {
                                Value::Integer(v) => v as usize,
                                Value::Float(v) => v as usize,
                                _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                            };
                            match e1_val[start_val..(start_val + length_val)].find(&e2_val).map(|p| p + start_val) {
                                Some(pos) => pos,
                                None => 0,
                            }
                        }
                        _ => 0, // This should never happen because we trust the compiler to set n correctly.
                    };
                    self.stack.push(Value::Integer((result as i32)+1)); // <-- RM Basic starts arrays at 1, not zero
                }
                Opcode::Int => {
                    let value = self.stack.pop().unwrap();
                    match value {
                        Value::Integer(_) => self.stack.push(value),
                        Value::Float(v) => self.stack.push(Value::Integer(v as i32)),
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                }
                Opcode::LeftStr => {
                    let n = self.stack.pop().unwrap();
                    let s = self.stack.pop().unwrap();
                    let n_val = match n {
                        Value::Integer(v) => v as usize,
                        Value::Float(v) => v as usize,
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    let s_val = match s {
                        Value::String(v) => v,
                        _ => { error_status.error_type = ErrorType::StringExpressionNeeded; continue }
                    };
                    if n_val < 1 { error_status.error_type = ErrorType::StringOffsetOutOfRange; continue }
                    let native_n = if n_val > s_val.len() {
                        s_val.len()
                    } else {
                        n_val
                    };
                    if s_val.len() == 0 { error_status.error_type = ErrorType::EmptyStringNotAllowed; continue };
                    let result = match s_val.char_indices().nth(native_n) {
                        Some((idx, _)) => &s_val[..idx],
                        None => &s_val,
                    };
                    self.stack.push(Value::String(result.to_string()))
                }
                Opcode::Len => {
                    let s = self.stack.pop().unwrap();
                    let s_val = match s {
                        Value::String(v) => v,
                        _ => { error_status.error_type = ErrorType::StringExpressionNeeded; continue }
                    };
                    self.stack.push(Value::Integer(s_val.len() as i32));
                }
                Opcode::Ln => {
                    let n = self.stack.pop().unwrap();
                    let n_val = match n {
                        Value::Integer(v) => v as f64,
                        Value::Float(v) => v,
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    if n_val <= 0.0 { error_status.error_type = ErrorType::BadArgumentForFunction; continue }
                    self.stack.push(Value::Float(n_val.ln()));
                }
                Opcode::Log => {
                    let n = self.stack.pop().unwrap();
                    let n_val = match n {
                        Value::Integer(v) => v as f64,
                        Value::Float(v) => v,
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    if n_val <= 0.0 { error_status.error_type = ErrorType::BadArgumentForFunction; continue }
                    self.stack.push(Value::Float(n_val.log10()));
                }
                Opcode::MidStr(n) => {
                    let mut s: Value = Value::Null;
                    let mut start: Value = Value::Null;
                    let mut length: Value = Value::Null;  
                    let mut result = String::from("");
                    match n {
                        2 => {
                            start = self.stack.pop().unwrap();
                            s = self.stack.pop().unwrap();
                        }
                        3 => {
                            length = self.stack.pop().unwrap();
                            start = self.stack.pop().unwrap();
                            s = self.stack.pop().unwrap();
                        }
                        _ => { error_status.error_type = ErrorType::InvalidExpressionFound; continue }
                    }
                    let s_val = match s {
                        Value::String(v) => v,
                        _ => { error_status.error_type = ErrorType::StringExpressionNeeded; continue }
                    };
                    if s_val.len() == 0 { error_status.error_type = ErrorType::EmptyStringNotAllowed; continue }
                    let start_val = match start {
                        Value::Integer(v) => v as usize,
                        Value::Float(v) => v as usize,
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    if start_val < 1 || start_val > s_val.len() { error_status.error_type = ErrorType::StringOffsetOutOfRange; continue }
                    match n {
                        2 => {
                            result = s_val.chars().skip(start_val).collect();
                        }
                        3 => {
                            let mut length_val = match length {
                                Value::Integer(v) => v as usize,
                                Value::Float(v) => v as usize,
                                _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                            };
                            if length_val < 1 { error_status.error_type = ErrorType::StringOffsetOutOfRange; continue }
                            let native_start = start_val - 1;
                            let max_length = s_val.len() - native_start;
                            if length_val > max_length { length_val = max_length }
                            result = s_val.chars().skip(native_start).take(length_val).collect()
                        }
                        _ => { error_status.error_type = ErrorType::InvalidExpressionFound; continue }
                    } 
                    self.stack.push(Value::String(result))   
                }
                Opcode::RightStr => {
                    let n = self.stack.pop().unwrap();
                    let s = self.stack.pop().unwrap();
                    let mut n_val = match n {
                        Value::Integer(v) => v as usize,
                        Value::Float(v) => v as usize,
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    let s_val = match s {
                        Value::String(v) => v,
                        _ => { error_status.error_type = ErrorType::StringExpressionNeeded; continue }
                    };
                    if s_val.len() == 0 { error_status.error_type = ErrorType::EmptyStringNotAllowed; continue };
                    if n_val < 1 { error_status.error_type = ErrorType::StringOffsetOutOfRange; continue }
                    if n_val > s_val.len() { n_val = s_val.len() }
                    n_val = s_val.chars().count().saturating_sub(n_val);
                    let result = match s_val.char_indices().nth(n_val) {
                        Some((idx, _)) => &s_val[idx..],
                        None => "",
                    };
                    self.stack.push(Value::String(result.to_string()))
                }
                Opcode::Rnd => {
                    let value = self.stack.pop().unwrap();
                    let mut cast_to_int = false;
                    let v = match value {
                        Value::Integer(v) => { cast_to_int = true; v as f64 }
                        Value::Float(v) => v,
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    let result = self.random_state.rnd(v);
                    if cast_to_int {
                        self.stack.push(Value::Integer(result as i32))
                    } else {
                        self.stack.push(Value::Float(result))
                    }
                }
                Opcode::Sgn => {
                    let value = self.stack.pop().unwrap();
                    let result = match value {
                        Value::Integer(v) => if v < 0 { 1 } else if v == 0 { 0 } else { 1 }
                        Value::Float(v) => if v < 0.0 { 1 } else if v == 0.0 { 0 } else { 1 }
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    self.stack.push(Value::Integer(result));
                }
                Opcode::Sin => {
                    let value = self.stack.pop().unwrap();
                    let mut angle: f64 = 0.0;
                    match value {
                        Value::Integer(v) => angle = v as f64,
                        Value::Float(v) => angle = v,
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                    if self.degrees { self.stack.push(Value::Float(angle.to_radians().sin())) }
                    else { self.stack.push(Value::Float(angle.sin())) }
                }
                Opcode::Spc => {
                    let value = self.stack.pop().unwrap();
                    let mut number_of_spaces = 0;
                    match value {
                        Value::Integer(v) => number_of_spaces = v as usize,
                        Value::Float(v) => number_of_spaces = v as usize,
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                    let result = if number_of_spaces > 0 { " ".repeat(number_of_spaces) } else { "".to_string() };
                    self.stack.push(Value::String(result))
                }
                Opcode::Sqr => {
                    let n = self.stack.pop().unwrap();
                    let n_val = match n {
                        Value::Integer(v) => v as f64,
                        Value::Float(v) => v,
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    if n_val <= 0.0 { error_status.error_type = ErrorType::BadArgumentForFunction; continue }
                    self.stack.push(Value::Float(n_val.sqrt()));
                }
                Opcode::StringStr => {
                    let n = self.stack.pop().unwrap();
                    let s = self.stack.pop().unwrap();
                    let mut n_val = match n {
                        Value::Integer(v) => v as usize,
                        Value::Float(v) => v as usize,
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    let s_val = match s {
                        Value::String(v) => v,
                        _ => { error_status.error_type = ErrorType::StringExpressionNeeded; continue }
                    };
                    if n_val < 1 { error_status.error_type = ErrorType::BadArgumentForFunction; continue }
                    if s_val.len() == 0 { error_status.error_type = ErrorType::EmptyStringNotAllowed; continue };
                    n_val = s_val.chars().count().saturating_sub(n_val+1); // account for potential unicode chars (TODO- include in other string functions...)
                    let result = s_val.repeat(n_val);
                    self.stack.push(Value::String(result))
                }
                Opcode::StrStr => {
                    let value = self.stack.pop().unwrap();
                    let result = match value {
                        Value::Integer(v) => { if v >= 0 { format!(" {}", v) } else { v.to_string() } }
                        Value::Float(v) => { if v >= 0.0 { format!(" {}", v) } else { v.to_string() } }
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded; continue }
                    };
                    self.stack.push(Value::String(result))
                }
                Opcode::Tan => {
                    let value = self.stack.pop().unwrap();
                    let mut angle: f64 = 0.0;
                    match value {
                        Value::Integer(v) => angle = v as f64,
                        Value::Float(v) => angle = v,
                        _ => error_status.error_type = ErrorType::NumericExpressionNeeded
                    }
                    if self.degrees { self.stack.push(Value::Float(angle.to_radians().tan())) }
                    else { self.stack.push(Value::Float(angle.tan())) }
                }
                Opcode::Val => {
                    let mut result =Value::Integer(0);
                    let value = self.stack.pop().unwrap();
                    let val_str = match value {
                        Value::String(v) => v,
                        _ => { error_status.error_type = ErrorType::StringExpressionNeeded; continue }
                    };
                    if val_str.len() == 0 { error_status.error_type = ErrorType::EmptyStringNotAllowed; continue }
                    let trimmed = val_str.trim_start();
                    let end = trimmed
                        .char_indices()
                        .find(|&(_, c)| !matches!(c, '0'..='9' | '.' | '+' | '-' | 'e' | 'E'))
                        .map(|(i, _)| i)
                        .unwrap_or(trimmed.len());
                    let numeric_part = &trimmed[..end];
                    if numeric_part.is_empty() {
                        result = Value::Integer(0);
                    }
                    if let Ok(i) = numeric_part.parse::<i32>() {
                        result = Value::Integer(i)
                    } 
                    else if let Ok(f) = numeric_part.parse::<f64>() {
                        result = Value::Float(f)
                    } 
                    self.stack.push(result)
                }
                Opcode::Negate => {
                    let value = self.stack.pop().expect("Stack underflow on Negate!");
                    match value {
                        Value::Integer(n) => self.stack.push(Value::Integer(-n)),
                        Value::Float(n) => self.stack.push(Value::Float(-n)),
                        Value::String(_) => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                        _ => { error_status.error_type = ErrorType::InvalidExpressionFound },
                    }
                }
                Opcode::Not => {
                    let value = self.stack.pop().expect("Stack underflow on Not!");
                    match value {
                        Value::Integer(n) => self.stack.push(Value::Integer(!n)),
                        Value::Float(n) => self.stack.push(Value::Integer(!(n as i32))),
                        Value::String(_) => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                        _ => error_status.error_type = ErrorType::InvalidExpressionFound,
                    }
                }
                Opcode::Add => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    // Runtime type resolution - as well as casting between floats and intergers is purely
                    // numeric expressions, if one side is a string and the other is a number, we must convert
                    // the number to a string representation:
                    match (left, right) {
                        // number+number or string+string:
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a + b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Float(a + b)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(Value::Float(a as f64 + b)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Float(a + b as f64)),
                        (Value::String(a), Value::String(b)) => self.stack.push(Value::String(format!("{}{}", a, b))),
                        // string+number:
                        (Value::String(a), Value::Integer(b)) => self.stack.push(Value::String(format!("{}{}", a, b))),
                        (Value::String(a), Value::Float(b)) => self.stack.push(Value::String(format!("{}{}", a, b))),
                        (Value::Integer(a), Value::String(b)) => self.stack.push(Value::String(format!("{}{}", a, b))),
                        (Value::Float(a), Value::String(b)) => self.stack.push(Value::String(format!("{}{}", a, b))),
                        _ => error_status.error_type = ErrorType::InvalidExpressionFound, 
                    }
                }
                Opcode::Subtract => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    match (left, right) {
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a - b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Float(a - b)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(Value::Float(a as f64 - b)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Float(a - b as f64)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }
                }
                Opcode::Multiply => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    match (left, right) {
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a * b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Float(a * b)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(Value::Float(a as f64 * b)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Float(a * b as f64)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }
                }
                Opcode::Divide => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if divide_by_zero_protection(right.clone()) { 
                        error_status.error_type = ErrorType::TryingToDivideByZero 
                    } else {
                        match (left, right) {
                            (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a / b)),
                            (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Float(a / b)),
                            (Value::Integer(a), Value::Float(b)) => self.stack.push(Value::Float(a as f64 / b)),
                            (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Float(a / b as f64)),
                            _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                        }
                    }
                }
                Opcode::Mod => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if divide_by_zero_protection(right.clone()) {
                        error_status.error_type = ErrorType::TryingToDivideByZero
                    } else {
                        // Perform MOD or %, casting as needed:
                        match (left, right) {
                            (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a % b)),
                            (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Integer(a as i32 % b as i32)),
                            (Value::Integer(a), Value::Float(b)) => self.stack.push(Value::Integer(a % b as i32)),
                            (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Integer(a as i32 % b)),
                            _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                        }
                    }                  
                }
                Opcode::IntegerDivide => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if divide_by_zero_protection(right.clone()) {
                        error_status.error_type = ErrorType::TryingToDivideByZero
                    } else {
                        // Perform integer division, casting as needed:
                        match (left, right) {
                            (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a / b)),
                            (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Integer(a as i32 / b as i32)),
                            (Value::Integer(a), Value::Float(b)) => self.stack.push(Value::Integer(a / b as i32)),
                            (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Integer(a as i32 / b)),
                            _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                        }
                    }
                }
                Opcode::Power => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    match (left, right) {
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a.pow(b as u32))),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Float(a.powf(b as f64))),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }
                }
                Opcode::And => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    match (left, right) {
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a & b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Integer(a as i32 & b as i32)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(Value::Integer(a & b as i32)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Integer(a as i32 & b)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }
                }
                Opcode::Or => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    match (left, right) {
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a | b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Integer(a as i32 | b as i32)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(Value::Integer(a | b as i32)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Integer(a as i32 | b)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }
                }
                Opcode::Xor => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    match (left, right) {
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(Value::Integer(a ^ b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(Value::Integer(a as i32 ^ b as i32)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(Value::Integer(a ^ b as i32)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(Value::Integer(a as i32 ^ b)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }
                }
                Opcode::Equal => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if mixed_string_numeric(&left, &right, error_status) { continue };
                    match (left, right) {  
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(represent_bool(a == b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(represent_bool(a == b)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(represent_bool(a as f64 == b)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(represent_bool(a == b as f64)),
                        (Value::String(a), Value::String(b)) => self.stack.push(represent_bool(a == b)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },   // TODO: Strings!
                    }                
                }
                Opcode::LessThan => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if mixed_string_numeric(&left, &right, error_status) { continue };
                    match (left, right) {  
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(represent_bool(a < b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(represent_bool(a < b)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(represent_bool((a as f64) < b)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(represent_bool(a < b as f64)),
                        (Value::String(a), Value::String(b)) => self.stack.push(represent_bool(a < b)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }                
                }
                Opcode::GreaterThan => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if mixed_string_numeric(&left, &right, error_status) { continue };
                    match (left, right) {  
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(represent_bool(a > b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(represent_bool(a > b)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(represent_bool(a as f64 > b)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(represent_bool(a > b as f64)),
                        (Value::String(a), Value::String(b)) => self.stack.push(represent_bool(a > b)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }                
                }
                Opcode::LessEqual => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if mixed_string_numeric(&left, &right, error_status) { continue };
                    match (left, right) {  
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(represent_bool(a <= b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(represent_bool(a <= b)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(represent_bool(a as f64 <= b)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(represent_bool(a <= b as f64)),
                        (Value::String(a), Value::String(b)) => self.stack.push(represent_bool(a <= b)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }                
                }
                Opcode::GreaterEqual => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if mixed_string_numeric(&left, &right, error_status) { continue };
                    match (left, right) {  
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(represent_bool(a >= b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(represent_bool(a >= b)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(represent_bool(a as f64 >= b)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(represent_bool(a >= b as f64)),
                        (Value::String(a), Value::String(b)) => self.stack.push(represent_bool(a >= b)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }                
                }
                Opcode::Inequality => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if mixed_string_numeric(&left, &right, error_status) { continue };  
                    match (left, right) {  
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(represent_bool(a != b)),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(represent_bool(a != b)),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(represent_bool(a as f64 != b)),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(represent_bool(a != b as f64)),
                        (Value::String(a), Value::String(b)) => self.stack.push(represent_bool(a != b)),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }                
                }
                Opcode::Interesting => {
                    let right = self.stack.pop().unwrap();
                    let left = self.stack.pop().unwrap();
                    if mixed_string_numeric(&left, &right, error_status) { continue };                    
                    match (left, right) {  
                        (Value::Integer(a), Value::Integer(b)) => self.stack.push(
                            represent_bool(interesting_float_comparison(a as f64, b as f64))),
                        (Value::Float(a), Value::Float(b)) => self.stack.push(
                            represent_bool(interesting_float_comparison(a, b))),
                        (Value::Integer(a), Value::Float(b)) => self.stack.push(
                            represent_bool(interesting_float_comparison(a as f64, b))),
                        (Value::Float(a), Value::Integer(b)) => self.stack.push(
                            represent_bool(interesting_float_comparison(a, b as f64))),
                        (Value::String(a), Value::String(b)) => self.stack.push(
                            represent_bool(interestingly_equal_str(&a, &b))
                        ),
                        _ => { error_status.error_type = ErrorType::NumericExpressionNeeded },
                    }                
                }
                Opcode::List => print!("{}", self.parser.list_program()),
            }
        }
        Value::Null
    }
}