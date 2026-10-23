use rustc_hash::FxHashMap;

use crate::{compiler::{CallableType, LoopType}, scanner::Token};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Integer(i32),
    Float(f64),
    String(String),
    ArrayReference{ call_stack_idx: usize, name: String },
    Null,               // Don't panic, this is for internal use. We're not implementing Nulls in RM Basic!
}

#[derive(Debug, Clone)]
pub enum Opcode {
    ClearVars,
    Constant(usize),
    GetVar(usize), // Pops nothing, pushes Value. index points to variable name in constant pool
    SetVar(usize), // Pops Value, pushes nothing. index points to variable name
    Jump(usize),
    Global(String),
    Add, Subtract, Multiply, Divide, IntegerDivide, Power, Negate, Mod, Not, And, Or, Xor,
    Equal, GreaterThan, LessThan, GreaterEqual, LessEqual, Inequality, Interesting,
    List,
    Return,
    Bye,
    End,
    Print,
    Label(String),
    JumpToLine(u32),
    JumpToLabel{ label: String, callable_type: CallableType },
    JumpIfFalse(usize),
    JumpIfFalseToLabel(String),
    RegisterCallable{ callable_type: CallableType, identifier: String, args: Vec<Token>, procedure_return_args: Vec<Token> },
    PushCall{ callable_type: CallableType, identifier: String, procedure_call_receive_args: Vec<Token> },
    PopCall,
    PushLoop(LoopType, String, usize),
    PopLoop,
    Loop(LoopType),
    Endfun,
    GetArray(usize, usize),
    SetArray(usize, usize),
    Dim(usize, usize),
    Restore(usize),
    Read,
    OnError(bool),
    OnGoto(Vec<u32>),
    OnGosub(Vec<String>),
    SetDeg(bool),
    Abs, Asc, Atn, ChrStr, Cos, Exp, HexStr, Instr(usize), Int, LeftStr, Len, Ln, Log,
    MidStr(usize), RightStr, Rnd, Sgn, Sin, Spc, Sqr, StringStr, StrStr, Tan, Val,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub code: Vec<(usize, usize, Opcode)>, // line_number, statement_number, OpCode
    pub constants: Vec<Value>,
    pub program_location_to_ip: FxHashMap<(u32, u32), usize>,
}

impl Chunk {
    pub fn new() -> Self {
        Chunk {
            code: Vec::new(),
            constants: Vec::new(),
            program_location_to_ip: FxHashMap::default(),
        }
    }

    pub fn write_opcode(&mut self, line_number: usize, statement_number: usize, opcode: Opcode) {
        self.code.push((line_number, statement_number, opcode));
    }

    pub fn add_constant(&mut self, value: Value) -> usize {
        self.constants.push(value);
        self.constants.len() - 1 // Return the index of the new constant
    }

    pub fn overwrite_opcode(&mut self, ip: usize, line_number: usize, statement_number: usize, opcode: Opcode) {
        self.code[ip] = (line_number, statement_number, opcode)
    }
}
