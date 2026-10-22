use crate::scanner::Token;
use crate::vm::opcode::Value;


#[derive(Debug, Clone)]
pub enum Expr {
    Empty,
    Null,
    ArrayReference(String),
    Literal(Token),
    Variable(Token),
    Unary { 
        operator: Token, 
        right: Box<Expr> 
    },
    Binary { 
        left: Box<Expr>, 
        operator: Token, 
        right: Box<Expr> 
    },
    Grouping(Box<Expr>),
    ArrayAccess{                // Also used for function calls
        name: Token,
        subscripts: Vec<Expr>
    },
    Builtin{
        name: Token,
        args: Vec<Expr>
    }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    LineNumber(Token),
    Global(String),
    Assignment { name: Token, value: Expr },
    Dim { name: Token, bounds: Vec<Expr> },
    ArrayAssignment { name: Token, subscripts: Vec<Expr>, value: Expr },
    Expression(Expr),
    Empty,
    List,
    Run,
    Bye,
    End,
    Print(Expr),
    Goto(String),
    Gosub(String),
    Subroutine(String),
    Return,
    Repeat,
    Until(Expr),
    For { control_var: Token, start: Expr, finish: Expr, step: Expr },
    Next(Token),
    Function { name: Token, args: Vec<Token> },
    Result(Expr),
    Endfun,
    If(Expr),
    Then,
    Else,
    Procedure { name: Token, receive_args: Vec<Token>, return_args: Vec<Token>},
    ProcedureCall { name: Token, args: Vec<Expr>, receive_args: Vec<Token>},
    Leave,
    Endproc,
    Data(Vec<Value>),
    Restore(String),
    Read(Vec<Expr>),
    OnError(bool),
    OnGoto{ expr: Expr, labels: Vec<String>},
    OnGosub{ expr: Expr, labels: Vec<String>},
    SetDeg(bool),
    Break,
}