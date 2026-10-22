pub mod parser;
pub mod ast;


use std::collections::HashMap;

use crate::scanner::{IdentifierOrLiteralType, Statement, TokenType};
use crate::vm::opcode::Opcode;
use crate::compiler::ast::Expr;
use crate::vm::opcode::Chunk;
use crate::vm::opcode::Value;
use crate::compiler::ast::Stmt;
use crate::errors::ErrorStatus;
use crate::errors::ErrorType;

#[derive(Debug, Clone, PartialEq, Eq,)]
pub enum CallableType {
    None,
    Subroutine,
    Procedure,
    Function
}

#[derive(Debug, Clone)]
pub struct Callable {
    label: String,
    callable_type: CallableType
}

#[derive(Debug, Clone, PartialEq, Eq,)]
pub enum LoopType {
    Repeat,
    For
}

#[derive(Debug, Clone)]
pub struct Compiler {
    pub line_number: usize,
    pub statement_number: usize,
    pub chunk: Chunk,
    pub register_callable_code: Chunk,
    pub in_callable: bool,  // This could be redundant due to CallableType::None but...idk....
    pub callable_type: CallableType,
    pub callable_line: usize,
    pub callable_stmt: usize,
    pub labels: HashMap<String, usize>,
    pub callables: HashMap<String, Callable>,
    pub if_id: usize,
    pub if_stack: Vec<usize>,
    pub loop_id: usize,
    pub loop_stack: Vec<usize>,
    pub need_end_if: bool,
    pub need_on_error_ok: bool,
    pub need_opcode_end: bool,
    pub compile_all: bool,
    pub inline_data: HashMap<usize, Vec<Value>>,
}

pub fn decompile(chunk: Chunk, entry_ip: usize) {
    println!("-- Code ----------------------------------");
    println!(" addr | line | stmt | opcode");
    println!("------------------------------------------");

    for (addr,(line,stmt,opcode)) in chunk.code.iter().enumerate() {
        if addr == entry_ip {
            println!(" ↓------------------- ENTRY POINT");
        }
        println!(" {} | {} | {} | {:?}", addr, line, stmt, opcode);
    }
    println!("-- Constants -----------------------------");
    println!(" idx | value ");
    println!("------------------------------------------");
    for (idx, v) in chunk.constants.iter().enumerate() {
        println!(" {} |  {:?}", idx, v);
    }
    println!("------------------------------------------");
    //println!("{:?}", chunk.program_location_to_ip);
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            line_number: 0,
            statement_number: 0,
            chunk: Chunk::new(),
            register_callable_code: Chunk::new(),
            in_callable: false,
            callable_type: CallableType::None,
            callable_line: 0,
            callable_stmt: 0,
            labels: HashMap::new(),
            callables: HashMap::new(),
            if_id: 0,
            if_stack: Vec::new(),
            loop_id: 0,
            loop_stack: Vec::new(),
            need_end_if: false,
            need_on_error_ok: false,
            need_opcode_end: false,
            compile_all: false,
            inline_data: HashMap::new(),
        }
    }

    /// Recursively walks the AST and emits stack-based bytecode
    pub fn compile_expression(&mut self, expr: &Expr, error_status: &mut ErrorStatus) {

        fn require_one_arg(args: &Vec<Expr>, error_status: &mut ErrorStatus) -> bool {
            if args.len() != 1 { error_status.error_type = ErrorType::InvalidExpressionFound; return false }
            true
        }

        fn required_args(arg_count: usize, args: &Vec<Expr>, error_status: &mut ErrorStatus) -> bool {
            if args.len() != arg_count { error_status.error_type = ErrorType::InvalidExpressionFound; return false }
            true
        }

        match expr {
            Expr::Empty => {}

            Expr::Builtin { name, args } => {
                let expr = args[0].clone();
                match expr {
                    Expr::Null => { error_status.error_type = ErrorType::InvalidExpressionFound }
                    _ => {}
                }
                self.compile_expression(&expr, error_status);
                match name.token_type {
                    TokenType::Abs => if require_one_arg(args, error_status) { 
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Abs)
                    },
                    TokenType::Asc => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Asc)
                    },
                    TokenType::Atn => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Atn)
                    },
                    TokenType::ChrStr => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::ChrStr)
                    },
                    TokenType::Cos => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Cos)
                    },
                    TokenType::Exp => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Exp)
                    },
                    TokenType::HexStr => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::HexStr)
                    },
                    TokenType::Instr => {
                        // Require 2, 3 or 4 args and compile them (arg 1 has already been compiled)
                        match args.len() {
                            2 => {
                                self.compile_expression(&args[1], error_status);
                                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Instr(2))
                            }
                            3 => {
                                self.compile_expression(&args[1], error_status);
                                self.compile_expression(&args[2], error_status);
                                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Instr(3))
                            }
                            4 => {
                                self.compile_expression(&args[1], error_status);
                                self.compile_expression(&args[2], error_status); 
                                self.compile_expression(&args[3], error_status);
                                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Instr(4))                            
                            }
                            _ => error_status.error_type = ErrorType::InvalidExpressionFound
                        }
                    },
                    TokenType::Int => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Int)
                    },
                    TokenType::LeftStr => if required_args(2, args, error_status) {
                        self.compile_expression(&args[1], error_status);
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::LeftStr)
                    },
                    TokenType::Len => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Len)
                    },
                    TokenType::Ln => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Ln)
                    },
                    TokenType::Log => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Log)
                    },
                    TokenType::MidStr => {
                        // Require 2 or 3 and compile them (arg 1 has already been compiled)
                        match args.len() {
                            2 => {
                                self.compile_expression(&args[1], error_status);
                                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::MidStr(2))                                
                            }
                            3 => {
                                self.compile_expression(&args[1], error_status);
                                self.compile_expression(&args[2], error_status);
                                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::MidStr(3))                                 
                            }
                            _ => error_status.error_type = ErrorType::InvalidExpressionFound
                        }
                    },
                    TokenType::RightStr => if required_args(2, args, error_status) {
                        self.compile_expression(&args[1], error_status);
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::RightStr)
                    },
                    TokenType::Rnd => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Rnd)
                    },
                    TokenType::Sgn => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Sgn)
                    },
                    TokenType::Sin => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Sin)
                    },
                    TokenType::Spc => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Spc)
                    },
                    TokenType::Sqr => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Sqr)
                    },
                    TokenType::StringStr => if required_args(2, args, error_status) {
                        self.compile_expression(&args[1], error_status);
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::StringStr)
                    },
                    TokenType::StrStr => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::StrStr)
                    },
                    TokenType::Tan => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Tan)
                    },
                    TokenType::Val => if require_one_arg(args, error_status) {
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Val)
                    },
                    _ => panic!("Builtin function '{}' has not been implemented yet", name.value),
                }
            }

            Expr::ArrayReference(name) => {
                match self.callables.get(name) {
                    Some(c) => {
                        // Function call with no args:
                        match c.callable_type {
                            CallableType::Function => {
                                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PushCall { 
                                    callable_type: c.callable_type.clone(), identifier: name.clone(), procedure_call_receive_args: Vec::new(),
                                });
                                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLabel{ 
                                    label: c.label.clone(), callable_type: CallableType::Function});
                            }
                            _ => panic!("Unsupported callable type: {:?}", c)
                        }
                    }
                    None => {
                        // Array reference:
                        let constant_idx = self.chunk.add_constant(Value::ArrayReference { 
                            call_stack_idx: 0,      // <--- will be updated by VM
                            name: name.to_string()
                        });
                        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Constant(constant_idx));                        
                    }
                }
            }

            Expr::Null => {
                // We need Null internally for things like optional parameters:
                let constant_idx = self.chunk.add_constant(Value::Null);
                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Constant(constant_idx));
            }
            
            Expr::Literal(token) => {
                if token.token_type == TokenType::NumberLiteral || token.token_type == TokenType::StringLiteral {
                    let value = match token.identifier_or_literal_type {
                        IdentifierOrLiteralType::Float => Value::Float(token.value.parse::<f64>().unwrap()),
                        IdentifierOrLiteralType::Integer => Value::Integer(token.value.parse::<i32>().unwrap()),
                        IdentifierOrLiteralType::String => Value::String(token.value.parse::<String>().unwrap()),
                        _ => panic!("Trying to parse non-literal token as a literal")
                    };
                    // Add the value to the pool and emit a Constant instruction
                    let constant_idx = self.chunk.add_constant(value);
                    self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Constant(constant_idx));
                } else {
                    let value = match token.token_type {
                        TokenType::True => { Value::Integer(-1) }
                        TokenType::False => {Value::Integer(0) }
                        TokenType::Pi => { Value::Float(std::f64::consts::PI) }
                        _ => panic!("Trying to parse non-literal token as a literal")
                    };
                    let constant_idx = self.chunk.add_constant(value);
                    self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Constant(constant_idx));
                }
            }
            
            Expr::Grouping(inner_expr) => {
                // Parentheses just mean we compile the inside normally
                self.compile_expression(inner_expr, error_status);
            }
            
            Expr::Unary { operator, right } => {
                // Compile the right-hand side first (puts it on the stack)
                self.compile_expression(right, error_status);
                
                // Then apply the unary operation
                match operator.token_type {
                    TokenType::Minus => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Negate),
                    TokenType::Not => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Not),
                    _ => error_status.error_type = ErrorType::InvalidExpressionFound
                }
            }
            
            Expr::Binary { left, operator, right } => {
                // Compile left side (pushes to stack)
                self.compile_expression(left, error_status);
                
                // Compile right side (pushes to stack)
                self.compile_expression(right, error_status);
                
                // Emit the operator (will pop the top two stack items)
                match operator.token_type {
                    TokenType::Plus => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Add),
                    TokenType::Star => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Multiply),
                    TokenType::Minus => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Subtract),
                    TokenType::ForwardSlash => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Divide),
                    TokenType::BackSlash => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::IntegerDivide),
                    TokenType::Caret => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Power),
                    TokenType::Mod => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Mod),
                    TokenType::And => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::And),
                    TokenType::Or => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Or),
                    TokenType::Xor => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Xor),
                    TokenType::Equals => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Equal),
                    TokenType::Less => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::LessThan),
                    TokenType::Greater => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::GreaterThan),
                    TokenType::LessEqual => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::LessEqual),
                    TokenType::GreaterEqual => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::GreaterEqual),
                    TokenType::Inequality => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Inequality),
                    TokenType::Interesting => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Interesting),
                    _ => error_status.error_type = ErrorType::InvalidExpressionFound
                }
            }
            
            Expr::Variable(token) => {
                // Add the variable name to the constant pool
                let name_val = Value::String(token.value.clone());
                let name_idx = self.chunk.add_constant(name_val);
                
                // Emit instruction to fetch it at runtime
                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::GetVar(name_idx));
            }

            // ArrayAccess also handles Function calls:
            Expr::ArrayAccess { name, subscripts } => {
                for subscript in subscripts {
                    self.compile_expression(subscript, error_status);
                }
                match self.callables.get(&name.value) {
                    Some(c) => {
                        // Function call:
                        match c.callable_type {
                            CallableType::Function => {
                                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PushCall { 
                                    callable_type: c.callable_type.clone(), identifier: name.value.clone(), procedure_call_receive_args: Vec::new(),
                                });
                                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLabel{ 
                                    label: c.label.clone(), callable_type: CallableType::Function});
                            }
                            _ => panic!("Unsupported callable type: {:?}", c)
                        }
                    }
                    None => {
                        // Array access:
                        let name_val = Value::String(name.value.clone());
                        let name_idx = self.chunk.add_constant(name_val);
                        self.chunk.write_opcode(
                            self.line_number, 
                            self.statement_number, 
                            Opcode::GetArray(name_idx, subscripts.len())
                        );
                    }
                }
            }
        }
    }

    /// Create a label for REPEAT that can't clash with user-created labels
    pub fn infer_repeat_label(&self) -> String {
        return format!("___repeat_{}___", self.loop_id)
    }

    /// Create a label for FOR that can't clash with user-created labels
    pub fn infer_for_label(&self) -> String {
        return format!("___for_{}___", self.loop_id)
    }

    /// Create a label for ELSE that can't clash with user-created labels
    pub fn infer_else_label(&self) -> String {
        return format!("___else_{}___", self.if_stack[self.if_stack.len()-1])
    }

    /// Create a label for END IF that can't clash with user-created labels
    pub fn infer_end_if_label(&self) -> String {
        println!("infer_end_if_label");
        return format!("___end_if_{}___", self.line_number)
    }   

    /// Create a label for ON ERROR OK that can't clash with user-created labels
    pub fn infer_on_error_ok_label(&self) -> String {
        return format!("___on_error_ok_{}___", self.line_number)
    }

    /// Create a label for END_LOOP that can't clash with user-created labels
    pub fn infer_end_loop_label(&self) -> String {
        return format!("___end_loop_{}___", self.loop_stack[self.loop_stack.len()-1])
    }

    /// Create a label for a callable that can't clash with user-created labels
    pub fn infer_callable_label(&self, callable_type: CallableType, name: String) -> String {
        let category = match callable_type {
            CallableType::Subroutine => "subroutine",
            CallableType::Procedure => "procedure",
            CallableType::Function => "function",
            CallableType::None => panic!("Trying to label a None callable")
        };
        return format!("___{}_{}___", category, name)
    }

    /// Entry point for compilation
    pub fn compile_statement(&mut self, stmt: &Stmt, error_status: &mut ErrorStatus) {

        // Add current line number / statement number combo to IP map if not already in:
        self.chunk.program_location_to_ip.entry(
            (self.line_number as u32, self.statement_number as u32)
        ).or_insert(self.chunk.code.len());

        match stmt {
            Stmt::Empty => {},
            Stmt::OnError(v) => {
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::OnError(*v));
                let label = self.infer_on_error_ok_label();
                self.need_on_error_ok = true;
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLabel{ label: label.clone(), callable_type: CallableType::Subroutine });
            },
            Stmt::SetDeg(v) => self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::SetDeg(*v)),
            Stmt::OnGoto { expr, labels } => {
                self.compile_expression(expr, error_status);
                let mut lines = Vec::new();
                for l in labels.iter() {
                    let line: u32 = l.parse().unwrap();
                    lines.push(line);
                }
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::OnGoto(lines));
            },
            Stmt::OnGosub { expr, labels } => {
                self.compile_expression(expr, error_status);
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::OnGosub(labels.clone()));
            },
            Stmt::Data(payload) => {
                if self.inline_data.contains_key(&self.line_number) {
                    let data = self.inline_data.get_mut(&self.line_number).unwrap();
                    data.extend(payload.clone());
                } else {
                    _ = self.inline_data.insert(self.line_number, payload.clone())
                }
            },
            Stmt::Restore(label) => {
                if self.line_number == 0 && label != "" {
                    error_status.error_type = ErrorType::NoMoreDataToBeRead;
                } else {
                    let actual_label = if label == "" { String::from("0") } else { label.clone() };
                    let line: usize = actual_label.parse().unwrap();
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Restore(line));
                }                
            }
            Stmt::Read(vars) => {
                for arg in vars.iter() { 
                    match arg {
                        Expr::Variable(name) => {
                            self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Read);                
                            let name_val = Value::String(name.value.clone()); 
                            let name_idx = self.chunk.add_constant(name_val);
                            self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::SetVar(name_idx));                            
                        }
                        Expr::ArrayAccess { name, subscripts } => {
                            for subscript in subscripts {
                                self.compile_expression(subscript, error_status);
                                // TODO: error check
                            }
                            // TODO: error check                
                            let name_val = Value::String(name.value.clone()); 
                            let name_idx = self.chunk.add_constant(name_val);
                            self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Read);
                            self.chunk.write_opcode(
                                self.line_number, 
                                self.statement_number, 
                                Opcode::SetArray(name_idx, subscripts.len())
                            );
                        }
                        _ => panic!("Received {:?} as an argument to READ", arg)
                    }
                }
             }
            Stmt::Assignment { name, value } => {
                self.compile_expression(value, error_status); 
                // TODO: error check                 
                let name_val = Value::String(name.value.clone()); 
                let name_idx = self.chunk.add_constant(name_val);
                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::SetVar(name_idx));
            }
            Stmt::ArrayAssignment { name, subscripts, value } => {
                for subscript in subscripts {
                    self.compile_expression(subscript, error_status);
                }
                self.compile_expression(value, error_status);                 
                let name_val = Value::String(name.value.clone()); 
                let name_idx = self.chunk.add_constant(name_val);
                self.chunk.write_opcode(
                    self.line_number, 
                    self.statement_number, 
                    Opcode::SetArray(name_idx, subscripts.len())
                );
            }
            Stmt::Expression(expr) => self.compile_expression(expr, error_status),
            Stmt::List => self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::List),
            Stmt::Run => {
                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::JumpToLabel{
                label: String::from("___run_from_here___"), callable_type: CallableType::None })
            },
            Stmt::Bye => self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Bye),
            Stmt::End => self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::End),
            Stmt::Print(expr) => {
                self.compile_expression(expr, error_status);
                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Print);
            }
            Stmt::Goto(label) => {
                if self.line_number == 0 {
                    error_status.error_type = ErrorType::UnsafeGoto;
                } else {
                    let line: u32 = label.parse().unwrap();
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLine(line));
                }
            }
            Stmt::Function { name, args } => {
                let label = self.infer_callable_label(CallableType::Function, name.value.clone());
                if self.in_callable { 
                    error_status.error_type = ErrorType::CannotExecuteDefinition;
                } else {
                    _ = self.callables.insert(name.value.clone(), Callable { 
                        label: label.clone(), 
                        callable_type: CallableType::Function
                    });
                    self.in_callable = true;
                    self.callable_type = CallableType::Function;
                    self.callable_line = self.line_number;
                    self.callable_stmt = self.statement_number;
                    self.register_callable_code.write_opcode(self.line_number, self.statement_number, Opcode::RegisterCallable { 
                        callable_type: CallableType::Function, 
                        identifier: name.value.clone(), 
                        args: args.clone(),
                        procedure_return_args: Vec::new(),
                    });
                    self.labels.insert(label.clone(), self.chunk.code.len());
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Label(label.clone()));
                }
            }
            Stmt::Result(expr) => {
                self.compile_expression(expr, error_status);
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PopCall);
            }
            Stmt::Endfun => {
                self.in_callable = false;
                self.callable_type = CallableType::None;
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Endfun);
            }
            Stmt::Subroutine(label) => {
                let full_label = self.infer_callable_label(CallableType::Subroutine, label.to_string());
                if self.in_callable { 
                    error_status.error_type = ErrorType::CannotExecuteDefinition;
                } else {
                    self.in_callable = true;
                    self.callable_type = CallableType::Subroutine;
                    self.callable_line = self.line_number;
                    self.callable_stmt = self.statement_number;
                    // Subroutines can't have args or return vals so we don't need to register them:
                    self.labels.insert(full_label.clone(), self.chunk.code.len());
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Label(full_label));
                }
            }
            Stmt::Return => {
                if self.callable_type != CallableType::Subroutine {
                    error_status.error_type = ErrorType::ReturnWithoutAnyGosub;
                } else {
                    self.in_callable = false;
                    self.callable_type = CallableType::None;
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PopCall);
                }
            }
            Stmt::Gosub(label) => {
                let full_label = self.infer_callable_label(CallableType::Subroutine, label.to_string());
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PushCall { 
                    callable_type: CallableType::Subroutine, identifier: label.to_string(), procedure_call_receive_args: Vec::new(),
                });
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLabel{
                    label: full_label.clone(), callable_type: CallableType::Subroutine });               
            }
            Stmt::Procedure { name, receive_args, return_args }=> {
                let label = self.infer_callable_label(CallableType::Procedure, name.value.clone());
                if self.in_callable { 
                    error_status.error_type = ErrorType::CannotExecuteDefinition;
                } else {
                    _ = self.callables.insert(name.value.clone(), Callable { 
                        label: label.clone(), 
                        callable_type: CallableType::Procedure
                    });
                    self.in_callable = true;
                    self.callable_type = CallableType::Procedure;
                    self.callable_line = self.line_number;
                    self.callable_stmt = self.statement_number;
                    self.register_callable_code.write_opcode(self.line_number, self.statement_number, Opcode::RegisterCallable { 
                        callable_type: CallableType::Procedure, 
                        identifier: name.value.clone(), 
                        args: receive_args.clone(),
                        procedure_return_args: return_args.clone()
                    });
                    self.labels.insert(label.clone(), self.chunk.code.len());
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Label(label.clone()));
                }
            }
            Stmt::Leave => {
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PopCall);
            }
            Stmt::Endproc => {
                self.in_callable = false;
                self.callable_type = CallableType::None;
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PopCall);
            }
            Stmt::Global(label) => {
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Global(label.to_string()));
            }
            Stmt::Repeat => {
                self.loop_id += 1;
                self.loop_stack.push(self.loop_id);
                let label = self.infer_repeat_label();
                self.labels.insert(label.clone(), self.chunk.code.len());
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PushLoop(LoopType::Repeat, String::from(""), self.loop_id));
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Label(label.clone()));
            }
            Stmt::Until(expr) => {
                self.compile_expression(expr, error_status);
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Loop(LoopType::Repeat));
                let label = self.infer_end_loop_label();
                self.labels.insert(label.clone(), self.chunk.code.len());
                _ = self.loop_stack.pop();
            }
            Stmt::For { control_var, start, finish, step } => {
                self.loop_id += 1;
                self.loop_stack.push(self.loop_id);
                self.compile_expression(start, error_status);                  
                let name_val = Value::String(control_var.value.clone()); 
                let name_idx = self.chunk.add_constant(name_val);
                self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::SetVar(name_idx));
                self.compile_expression(start, error_status); 
                self.compile_expression(finish, error_status);
                match step {
                    Expr::Empty => self.compile_expression(&Expr::Null, error_status),
                    _ => self.compile_expression(step, error_status),
                }
                let label = self.infer_for_label();
                self.labels.insert(label.clone(), self.chunk.code.len());
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PushLoop(LoopType::For, control_var.value.clone(), self.loop_id));
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Label(label.clone()));
            }
            Stmt::Next(_) => {
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Loop(LoopType::For)); 
                let label = self.infer_end_loop_label();
                self.labels.insert(label.clone(), self.chunk.code.len());
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Label(label.clone()));
                _ = self.loop_stack.pop();
            }
            Stmt::Break => {
                if self.loop_stack.len() == 0 {
                    // pass
                } else {
                    let label = self.infer_end_loop_label();
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PopLoop);
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLabel{ label: label.clone(), callable_type: CallableType::None });
                }
            }
            Stmt::Dim { name, bounds } => {
                for bound in bounds {
                    self.compile_expression(bound, error_status);
                }
                let name_val = Value::String(name.value.clone());
                let name_idx = self.chunk.add_constant(name_val);
                self.chunk.write_opcode(
                    self.line_number, 
                    self.statement_number, 
                    Opcode::Dim(name_idx, bounds.len())
                );
            }
            Stmt::ProcedureCall { name, args, receive_args } => {
                let full_label = self.infer_callable_label(CallableType::Procedure, name.value.to_string());
                for expr in args {
                    self.compile_expression(expr, error_status);
                }
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::PushCall { 
                    callable_type: CallableType::Procedure, identifier: name.value.to_string(), procedure_call_receive_args: receive_args.clone(),
                });
                self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLabel {
                    label: full_label.clone(), callable_type: CallableType::Procedure });            
            }
            Stmt::If(expr) => {
                self.need_end_if = true;
                self.if_id += 1;
                self.if_stack.push(self.if_id);
                self.compile_expression(expr, error_status);
            }
            Stmt::Then => {
                if self.if_stack.len() == 0 {
                    error_status.error_type = ErrorType::ThenWithNoMatchingIf;
                } else {
                    let label = self.infer_else_label();
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpIfFalseToLabel(label));
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Label(String::from("___then___")));
                }
            }
            Stmt::Else => {
                if self.if_stack.len() == 0 {
                    // Unexecutable code, but we still need to compile it to make sure the program
                    // is valid.
                    //self.need_end_if = true;
                    let label = self.infer_end_if_label();
                    self.labels.insert(label.clone(), self.chunk.code.len());
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLabel{
                        label: label.clone(), callable_type: CallableType::None });
                    //_ = self.if_stack.pop();
                } else {
                    // JUMP to ___end_if___ before ELSE so THEN doesn't execute ELSE:
                    let label = self.infer_end_if_label();
                    self.labels.insert(label.clone(), self.chunk.code.len());
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLabel{
                        label: label.clone(), callable_type: CallableType::None });
                    // Label the ELSE statement:
                    let label = self.infer_else_label();
                    self.labels.insert(label.clone(), self.chunk.code.len());
                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::Label(label));
                    _ = self.if_stack.pop();
                }                
            }
            Stmt::LineNumber(t) => {
                // Mega-Hack to support implicit goto after THEN or ELSE, e.g THEN 310 or ELSE 20:
                // if self.chunk[ip-1] == Label("___then___") or contains "_else_" then set jump
                // otherwise set error
                if self.chunk.code.len() > 0 {
                    let (_, _, previous_opcode) = &self.chunk.code[self.chunk.code.len()-1];
                    match previous_opcode {
                        Opcode::Label(label) => {
                            if label.starts_with("___else_") || label.clone() == "___then___" {
                                if self.line_number == 0 {
                                    error_status.error_type = ErrorType::UnsafeGoto;
                                } else {
                                    let line: u32 = t.value.parse().unwrap();
                                    self.chunk.write_opcode(self.line_number, self.statement_number, Opcode::JumpToLine(line));
                                }
                            } else  {
                                error_status.error_type = ErrorType::VariableNameIsNeeded
                            }
                        }
                        _ => error_status.error_type = ErrorType::VariableNameIsNeeded
                    }
                } else {
                    error_status.error_type = ErrorType::VariableNameIsNeeded 
                }
            }
            //_ => error_status.error_type = ErrorType::UnknownCommandProcedure
        }
    }

    /// Compiles the top-level expression and adds a Return instruction
    pub fn finish(&mut self) {
        self.chunk.write_opcode(self.line_number, self. statement_number, Opcode::Return);
    }
}