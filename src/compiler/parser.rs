use std::collections::HashMap;
use crate::compiler::Statement;
use crate::compiler::TokenType;
use crate::errors::ErrorStatus;
use crate::errors::ErrorType;
use crate::scanner::token::{is_builtin};
use crate::scanner::{Token, IdentifierOrLiteralType};
use crate::compiler::ast::Expr;
use crate::compiler::ast::Stmt;
use crate::vm::opcode::Value;

#[derive(Debug, Clone)]
pub struct Parser {
    // Key is line_number and Value is list of statements in the line
    pub program: HashMap<usize, Vec<Statement>>,
    pub compile_all: bool,
    pub line_number: usize,
}

impl Parser {
    /// Load a line of tokenized statements into the program. Returns true if
    /// immediate execution is required.
    pub fn load_program_line(&mut self, statements: Vec<Statement>) -> bool {
        if statements.is_empty() {
            return false;
        }
        if let Some(first_token) = statements[0].tokens.first() {
            if first_token.token_type == TokenType::LineNumber {
                let line_number: usize = first_token.value.parse().unwrap();
                // Program mode
                self.program.insert(line_number, statements);
                return false;
            }
        }
        // Immediate mode: line number = 0
        self.program.insert(0, statements);
        true
    }

    /// Get a printable program line with optional token highlighting
    pub fn get_program_line(&self, line_number: usize, highlight_token: bool, statement_number: usize, token_number: usize) -> String {
        let mut line = "".to_string();
        let stmts = self.program.get(&line_number).unwrap();

        for (current_stmt_number, stmt) in stmts.iter().enumerate() {
            for (current_token_number, token) in stmt.tokens.iter().enumerate() {
                if highlight_token && current_stmt_number == statement_number && current_token_number == token_number {
                    line += &format!(">>> ");
                }
                let mut value = token.value.clone();
                if token.token_type == TokenType::StringLiteral {
                    // Surround string literals with double-quotes:
                    value = String::from("\"") + &value + &String::from("\"");
                }
                line += &format!("{} ", value);
            }
            if current_stmt_number < (stmts.len() - 1) { line += &String::from(": ") };
        }
        line
    }

    /// Get a printable program listing.
    pub fn list_program(&self) -> String  {
        let mut listing = String::from("");
        let mut line_numbers: Vec<_> = self.program.keys().copied().collect();
        line_numbers.sort();

        for line_number in line_numbers.iter() {
            // Skip immediate mode command
            if *line_number == 0 { continue };
            listing += &self.get_program_line(*line_number, false, 0, 0);
            listing += &String::from("\n");
        }
        listing
    }


    // Helper methods for navigating the tokens:

    fn peek<'a>(&self, tokens: &'a [Token], cursor: usize) -> Option<&'a Token> {
        tokens.get(cursor)
    }

    fn match_tokens(&self, tokens: &[Token], cursor: &mut usize, types: &[TokenType]) -> bool {
        if let Some(token) = self.peek(tokens, *cursor) {
            if types.contains(&token.token_type) {
                *cursor += 1;
                return true;
            }
        }
        false
    }

    pub fn check_type(&self, required_token_type: TokenType, error_type: ErrorType, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> bool {
        if *cursor == tokens.len() {
            self.raise_error(error_type, cursor, error_status);
            return false
        }
        if tokens[*cursor].token_type != required_token_type {
            self.raise_error(error_type, cursor, error_status);
            return false
        }
        true
    }

    pub fn raise_error(&self, error_type: ErrorType, cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        error_status.error_type = error_type;
        error_status.token_sequence = *cursor;
        Stmt::Empty
    }

    pub fn forward_parse_error(&self, cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        error_status.token_sequence = *cursor;
        Stmt::Empty
    }

    pub fn end_of_instruction(&self, cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        error_status.error_type = ErrorType::EndOfInstructionExpected;
        error_status.token_sequence = *cursor;
        Stmt::Empty
    }

    fn parse_subscripts(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Vec<Expr> {
        let mut subscripts = Vec::new();
        if !self.check_type(TokenType::LeftParen, ErrorType::OpeningBracketIsNeeded, tokens, cursor, error_status) {
            return subscripts;
        }
        *cursor += 1;

        // Check for no subscripts, i.e. array reference:
        if *cursor == tokens.len() {
            error_status.error_type = ErrorType::ArrayNeededNoSubscript;
        }
        if tokens[*cursor].token_type == TokenType::RightParen {
            *cursor += 1;
            return subscripts;
        } 
        loop {
            let expr = self.parse_expression(tokens, cursor, error_status);
            if error_status.error_type != ErrorType::Ok {
                break;
            }
            subscripts.push(expr);
            if *cursor == tokens.len() {
                self.raise_error(ErrorType::ClosingBracketNeeded, cursor, error_status);
                break;
            }
            if tokens[*cursor].token_type == TokenType::RightParen {
                *cursor += 1;
                break;
            }
            if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                break;
            }
            *cursor += 1;
        }
        subscripts
    }

    /// Entry point for parsing an expression.
    pub fn parse_expression(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        self.parse_logical_xor(tokens, cursor, error_status)
    }

    fn parse_logical_xor(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        let mut expr = self.parse_logical_or(tokens, cursor, error_status);
        while self.match_tokens(tokens, cursor, &[TokenType::Xor]) {
            let operator = tokens[*cursor - 1].clone();
            let right = self.parse_logical_or(tokens, cursor, error_status);
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        expr
    }

    fn parse_logical_or(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        let mut expr = self.parse_logical_and(tokens, cursor, error_status);
        while self.match_tokens(tokens, cursor, &[TokenType::Or]) {
            let operator = tokens[*cursor - 1].clone();
            let right = self.parse_logical_and(tokens, cursor, error_status);
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        expr
    }

    fn parse_logical_and(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        let mut expr = self.parse_unary_not(tokens, cursor, error_status);
        while self.match_tokens(tokens, cursor, &[TokenType::And]) {
            let operator = tokens[*cursor - 1].clone();
            let right = self.parse_unary_not(tokens, cursor, error_status);
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        expr
    }

    fn parse_unary_not(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        if self.match_tokens(tokens, cursor, &[TokenType::Not]) {
            let operator = tokens[*cursor - 1].clone();
            let right = self.parse_relational(tokens, cursor, error_status);
            return Expr::Unary { operator, right: Box::new(right) };
        }
        self.parse_relational(tokens, cursor, error_status)
    }

    fn parse_relational(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        let mut expr = self.parse_addition_subtraction(tokens, cursor, error_status);
        let operators = [
            TokenType::Equals, TokenType::Inequality,
            TokenType::Less, TokenType::Greater,
            TokenType::LessEqual, TokenType::GreaterEqual,
            TokenType::Interesting,
        ];

        while self.match_tokens(tokens, cursor, &operators) {
            let operator = tokens[*cursor - 1].clone();
            let right = self.parse_addition_subtraction(tokens, cursor, error_status);
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        expr
    }

    fn parse_addition_subtraction(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        let mut expr = self.parse_multiplication_division(tokens, cursor, error_status);

        // This handles BOTH math addition and string concatenation based on runtime types
        while self.match_tokens(tokens, cursor, &[TokenType::Plus, TokenType::Minus]) {
            let operator = tokens[*cursor - 1].clone();
            let right = self.parse_multiplication_division(tokens, cursor, error_status);
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        expr
    }

    fn parse_multiplication_division(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        let mut expr = self.parse_unary_sign(tokens, cursor, error_status);
        while self.match_tokens(tokens, cursor, &[TokenType::Star, TokenType::ForwardSlash, TokenType::BackSlash, TokenType::Mod]) {
            let operator = tokens[*cursor - 1].clone();
            let right = self.parse_unary_sign(tokens, cursor, error_status);
            expr = Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        expr
    }

    fn parse_unary_sign(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        if self.match_tokens(tokens, cursor, &[TokenType::Plus, TokenType::Minus]) {
            let operator = tokens[*cursor - 1].clone();
            let right = self.parse_exponentiation(tokens, cursor, error_status);
            return Expr::Unary { operator, right: Box::new(right) };
        }
        self.parse_exponentiation(tokens, cursor, error_status)
    }

    fn parse_exponentiation(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        // Because string concatenation is handled in addition/subtraction, 
        // we step straight down to parse_primary here.
        let expr = self.parse_primary(tokens, cursor, error_status);

        if self.match_tokens(tokens, cursor, &[TokenType::Caret]) {
            let operator = tokens[*cursor - 1].clone();
            let right = self.parse_exponentiation(tokens, cursor, error_status); 
            return Expr::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }
        expr
    }

    fn parse_primary(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Expr {
        if self.match_tokens(tokens, cursor, &[
            TokenType::NumberLiteral,
            TokenType::StringLiteral,
            TokenType::True,
            TokenType::False,
            TokenType::Pi,
            ]) {
            return Expr::Literal(tokens[*cursor - 1].clone());
        }

        // Handle builtins
        if *cursor < tokens.len() {
            if is_builtin(tokens[*cursor].token_type) {
                let identifier = tokens[*cursor].clone();
                *cursor += 1;
                let subscripts = self.parse_subscripts(tokens, cursor, error_status);
                if error_status.error_type != ErrorType::Ok {
                    return Expr::Empty;
                }
                // Subscripts: If a built-in *requires* n subscripts, we catch that in the VM
                // with "Invalid expression found" if the wrong number of subscripts are sent:
                let mut args = Vec::new();
                if subscripts.len() == 0 { args.push(Expr::Null) } else { args = subscripts }
                return Expr::Builtin { name: identifier.clone(), args: args }
            }
        }

        // Array reference, array access, function call, or variable:
        if self.match_tokens(tokens, cursor, &[TokenType::Identifier]) {
            let identifier = tokens[*cursor - 1].clone(); 
            if identifier.is_array {
                let subscripts = self.parse_subscripts(tokens, cursor, error_status);
                if error_status.error_type != ErrorType::Ok {
                    return Expr::Empty;
                }
                if self.line_number == 0 { self.compile_all = true }
                if subscripts.len() == 0 { return Expr::ArrayReference(identifier.value.clone()) }
                return Expr::ArrayAccess { name: identifier, subscripts: subscripts }
            }
            return Expr::Variable(identifier);
        }

        // Parentheses handling:
        if self.match_tokens(tokens, cursor, &[TokenType::LeftParen]) {
            let expr = self.parse_expression(tokens, cursor, error_status);
            
            // Explicit error checking for the closing parenthesis:
            if !self.match_tokens(tokens, cursor, &[TokenType::RightParen]) {
                error_status.error_type = ErrorType::ClosingBracketNeeded;
                error_status.token_sequence = *cursor;
                return Expr::Empty
            }
            return Expr::Grouping(Box::new(expr));
        }
        
        // Special case of program line that begins with an empty statement and
        // the parser just tries to parse the line number:
        if tokens.len() == 1 && *cursor > 0 {
            if tokens[*cursor-1].token_type == TokenType::LineNumber {
                return Expr::Empty
            }
        }

        error_status.error_type = ErrorType::EndOfInstructionExpected;
        error_status.token_sequence = *cursor;
        Expr::Empty
    }

    pub fn parse_bare_stmt(&self, stmt: Stmt, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return stmt
        }
        self.end_of_instruction(cursor, error_status)
    }

    // This is a v1 of PRINT for debugging only
    pub fn parse_print_stmt(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return Stmt::Print(Expr::Empty);
        }
        let expr =self.parse_expression(tokens, cursor, error_status);
        if error_status.error_type != ErrorType::Ok {
            return self.forward_parse_error(cursor, error_status)
        }
        if *cursor == tokens.len() {
            return Stmt::Print(expr);
        } 
        self.end_of_instruction(cursor, error_status)
    }
    
    pub fn parse_goto_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::LineNumberNeeded, cursor, error_status)
        }
        if tokens[*cursor].token_type != TokenType::LineNumber {
            return self.raise_error(ErrorType::LineNumberNeeded, cursor, error_status)
        } 
        *cursor += 1;
        if *cursor == tokens.len() {
            return Stmt::Goto(tokens[*cursor-1].value.clone());
        }
        self.end_of_instruction(cursor, error_status)
    }

    pub fn parse_gosub_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::SubroutineLabelNeeded, cursor, error_status)
        }
        let arg = tokens[*cursor].clone();
        let mut stmt = Stmt::Empty;
        if arg.token_type == TokenType::Identifier && arg.identifier_or_literal_type == IdentifierOrLiteralType::Float {
            stmt = Stmt::Gosub(arg.value)
        } else {
            return self.raise_error(ErrorType::SubroutineLabelNeeded, cursor, error_status)
        }
        *cursor += 1;
        if *cursor == tokens.len() {
            return stmt;
        }
        self.end_of_instruction(cursor, error_status)    
    }

    pub fn parse_global_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::VariableNameIsNeeded, cursor, error_status)
        }
        let arg = tokens[*cursor].clone();
        let mut stmt = Stmt::Empty;
        if arg.token_type == TokenType::Identifier {
            stmt = Stmt::Global(arg.value);
        } else {
            return self.raise_error(ErrorType::VariableNameIsNeeded, cursor, error_status)
        }
        *cursor += 1;
        if *cursor == tokens.len() {
            return stmt;
        }
        self.end_of_instruction(cursor, error_status)    
    }

    pub fn parse_subroutine_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::SubroutineLabelNeeded, cursor, error_status)
        }
        let arg = tokens[*cursor].clone();
        let mut stmt = Stmt::Empty;
        if arg.token_type == TokenType::Identifier && arg.identifier_or_literal_type == IdentifierOrLiteralType::Float {
            stmt = Stmt::Subroutine(arg.value)
        } else {
            return self.raise_error(ErrorType::SubroutineLabelNeeded, cursor, error_status)
        }
        *cursor += 1;
        if *cursor == tokens.len() {
            return stmt;
        }
        self.end_of_instruction(cursor, error_status)    
    }

    pub fn parse_until_stmt(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::NumericExpressionNeeded, cursor, error_status)
        }
;       let expr =self.parse_expression(tokens, cursor, error_status);
        if error_status.error_type != ErrorType::Ok {
            return self.forward_parse_error(cursor, error_status)
        }
        if *cursor == tokens.len() {
            return Stmt::Until(expr);
        } 
        self.end_of_instruction(cursor, error_status)
    }

    pub fn parse_for_stmt(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        // FOR
        *cursor += 1;
        // v
        if !self.check_type(TokenType::Identifier, ErrorType::VariableNameIsNeeded, tokens, cursor, error_status) {
            return Stmt::Empty
        }
        if tokens[*cursor].identifier_or_literal_type == IdentifierOrLiteralType::String {
            return self.raise_error(ErrorType::NumericVariableNeeded, cursor, error_status)
        }
        let control_var = tokens[*cursor].clone();
        *cursor += 1;
        // :=
        if !self.check_type(TokenType::ColonEquals, ErrorType::NumericExpressionNeeded, tokens, cursor, error_status) {
            return Stmt::Empty
        }
        *cursor += 1;
        // e1
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::NumericExpressionNeeded, cursor, error_status)
        }
        let start = self.parse_expression(tokens, cursor, error_status);
        // TO
        if !self.check_type(TokenType::To, ErrorType::ToIsNeededBeforeValue, tokens, cursor, error_status) {
            return Stmt::Empty
        }
        *cursor += 1;
        // e2
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::NumericExpressionNeeded, cursor, error_status)
        }
        let finish = self.parse_expression(tokens, cursor, error_status);
        // optional STEP
        if *cursor == tokens.len() {
            return Stmt::For{ control_var: control_var, start: start, finish: finish, step: Expr::Empty }
        }
        if !self.check_type(TokenType::Step, ErrorType::EndOfInstructionExpected, tokens, cursor, error_status) {
            return Stmt::Empty
        }
        *cursor += 1;
        // e3
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::NumericExpressionNeeded, cursor, error_status)
        }
        let step = self.parse_expression(tokens, cursor, error_status);
        if *cursor == tokens.len() {
            return Stmt::For{ control_var: control_var, start: start, finish: finish, step: step }
        }
        self.end_of_instruction(cursor, error_status)
    }

    pub fn parse_next_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        // NEXT
        *cursor += 1;
        // optional control var
        if *cursor == tokens.len() {
            // no control var
            return Stmt::Next(Token { token_type:TokenType::Identifier, value: String::from(""), identifier_or_literal_type: IdentifierOrLiteralType::None, is_array: false })
        }
        if !self.check_type(TokenType::Identifier, ErrorType::VariableNameIsNeeded, tokens, cursor, error_status) {
            return Stmt::Empty
        }
        if tokens[*cursor].identifier_or_literal_type == IdentifierOrLiteralType::String {
            return self.raise_error(ErrorType::NumericVariableNeeded, cursor, error_status)
        } else {
            let control_var = tokens[*cursor].clone();
            *cursor += 1;
            return Stmt::Next(control_var)
        }
    }


    pub fn parse_function_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        // FUNCTION
        *cursor += 1;
        // name
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::VariableNameIsNeeded, cursor, error_status)
        }
        let name = tokens[*cursor].clone();
        *cursor += 1;
        // (
        if !self.check_type(TokenType::LeftParen, ErrorType::OpeningBracketIsNeeded, tokens, cursor, error_status) {
            return Stmt::Empty
        }
        *cursor += 1;
        // ) or start of arg list
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::ClosingBracketNeeded, cursor, error_status)
        }
        let mut args = Vec::new();
        if tokens[*cursor].token_type == TokenType::RightParen {
            // no args
        } else {
            // get args
            loop {
                // Required arg
                let arg = tokens[*cursor].clone();
                if arg.token_type != TokenType::Identifier {
                    return self.raise_error(ErrorType::VariableNameIsNeeded, cursor, error_status)
                }
                args.push(arg);
                *cursor += 1;
                // Required ) or ,
                if *cursor == tokens.len() {
                    return self.raise_error(ErrorType::ClosingBracketNeeded, cursor, error_status)
                }
                // Handle array reference:
                if tokens[*cursor].token_type == TokenType::LeftParen {
                    *cursor += 1;
                    if !self.check_type(TokenType::RightParen, ErrorType::ClosingBracketNeeded, tokens, cursor, error_status) {
                        return Stmt::Empty
                    }
                    *cursor += 1;
                }
                if tokens[*cursor].token_type == TokenType::RightParen { break }
                if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                    return Stmt::Empty
                }
                *cursor += 1;
            }
        }
        // )
        if tokens[*cursor].token_type == TokenType::RightParen {
            *cursor += 1;
        } else {
            return self.raise_error(ErrorType::ClosingBracketNeeded, cursor, error_status)
        }
        if *cursor == tokens.len() {
            return Stmt::Function { name, args };
        }
        self.end_of_instruction(cursor, error_status)         
    }

    pub fn parse_result_stmt(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::NumberOrStringNeeded, cursor, error_status)
        }
;       let expr =self.parse_expression(tokens, cursor, error_status);
        if error_status.error_type != ErrorType::Ok {
            return self.forward_parse_error(cursor, error_status)
        }
        if *cursor == tokens.len() {
            return Stmt::Result(expr);
        } 
        self.end_of_instruction(cursor, error_status)
    }

    pub fn parse_dim_stmt(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;

        // Expect an identifier
        if *cursor == tokens.len() || tokens[*cursor].token_type != TokenType::Identifier {
            return self.raise_error(ErrorType::VariableNameIsNeeded, cursor, error_status);
        }
        let name = tokens[*cursor].clone();
        *cursor += 1;

        // Parse the bounds inside the parentheses using our existing helper
        let bounds = self.parse_subscripts(tokens, cursor, error_status);
        if error_status.error_type != ErrorType::Ok {
            return self.forward_parse_error(cursor, error_status);
        }
        
        if bounds.is_empty() {
            return self.raise_error(ErrorType::NumericExpressionNeeded, cursor, error_status);
        }

        if *cursor == tokens.len() {
            return Stmt::Dim { name, bounds };
        }
        self.end_of_instruction(cursor, error_status)
    }

    pub fn parse_procedure_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        // PROCEDURE
        *cursor += 1;
        // name
        if *cursor == tokens.len() {
            return self.raise_error(ErrorType::VariableNameIsNeeded, cursor, error_status)
        }
        let name = tokens[*cursor].clone();
        *cursor += 1;
        // Start of receive args list (if any)
        let mut receive_args = Vec::new();
        let mut return_args = Vec::new();
        if *cursor == tokens.len() { return Stmt::Procedure{ name, receive_args, return_args } }
        loop {
            if !self.check_type(TokenType::Identifier, ErrorType::VariableNameIsNeeded, tokens, cursor, error_status) {
                return Stmt::Empty
            }
            let arg = tokens[*cursor].clone();    
            receive_args.push(arg);
            *cursor += 1;
            if *cursor == tokens.len() { return Stmt::Procedure{ name, receive_args, return_args } }
            // Handle RETURN
            if tokens[*cursor].token_type == TokenType::Return { *cursor += 1; break }
            // Require , before next arg
            if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                return Stmt::Empty
            }
            *cursor += 1;
        }
        // Collect return args (at least one)
        if *cursor == tokens.len() {
            error_status.error_type = ErrorType::VariableNameIsNeeded;
            return Stmt::Empty
        }
        loop {
            if !self.check_type(TokenType::Identifier, ErrorType::VariableNameIsNeeded, tokens, cursor, error_status) {
                return Stmt::Empty
            }
            let arg = tokens[*cursor].clone();    
            return_args.push(arg);
            *cursor += 1;
            if *cursor == tokens.len() { return Stmt::Procedure{ name, receive_args, return_args } }
            // Require , before next arg
            if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                return Stmt::Empty
            }
            *cursor += 1;            
        }
    }

    pub fn parse_procedure_call_stmt(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        let mut args = Vec::new();
        let mut receive_args = Vec::new();
        // Name
        let name = tokens[*cursor].clone();
        *cursor += 1;
        if *cursor == tokens.len() { return Stmt::ProcedureCall { name, args, receive_args } }
        // Collect args
        loop {
            let expr = self.parse_expression(tokens, cursor, error_status);  
            args.push(expr);  
            if *cursor == tokens.len() { return Stmt::ProcedureCall{ name, args, receive_args } }
            // Handle RECEIVE
            if tokens[*cursor].token_type == TokenType::Receive { *cursor += 1; break }
            if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                return Stmt::Empty
            }
            *cursor += 1;        
        }
        // Collect receive args (at least one)
        if *cursor == tokens.len() {
            error_status.error_type = ErrorType::VariableNameIsNeeded;
            return Stmt::Empty
        }
        loop {
            if !self.check_type(TokenType::Identifier, ErrorType::VariableNameIsNeeded, tokens, cursor, error_status) {
                return Stmt::Empty
            }
            let arg = tokens[*cursor].clone();    
            receive_args.push(arg);
            *cursor += 1;
            if *cursor == tokens.len() { return Stmt::ProcedureCall{ name, args, receive_args } }
            // Require , before next arg
            if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                return Stmt::Empty
            }
            *cursor += 1;            
        }
    }

    pub fn parse_if_stmt(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            error_status.error_type = ErrorType::NumericExpressionNeeded; 
            return Stmt::Empty 
        }
        let expr = self.parse_expression(tokens, cursor, error_status);
        if error_status.error_type != ErrorType::Ok { return self.forward_parse_error(cursor, error_status) }
        if *cursor == tokens.len() {
            return Stmt::If(expr)
        } else {
            error_status.error_type = ErrorType::ThenIsRequired;
            return Stmt::Empty
        }
    }

    pub fn parse_data_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        let mut data: Vec<Value> = Vec::new();
        *cursor += 1;
        if *cursor == tokens.len() { return Stmt::Data(data) }
        loop {
            let token = &tokens[*cursor];
            let value = match token.token_type {
                TokenType::StringLiteral => Value::String(token.value.clone()),
                TokenType::NumberLiteral => {
                    match token.identifier_or_literal_type {
                        IdentifierOrLiteralType::Float => Value::Float(token.value.parse::<f64>().unwrap()),
                        IdentifierOrLiteralType::Integer => Value::Integer(token.value.parse::<i32>().unwrap()),
                        _ => panic!("Trying to push the value of {:?} to DATA", token)
                    }
                }
                _ => { error_status.error_type = ErrorType::StringOrNumberExpected; return Stmt::Empty }
            };
            data.push(value);
            *cursor += 1;
            if *cursor == tokens.len() { return Stmt::Data(data) }
            // Require , before next arg
            if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                return Stmt::Empty
            }   
            *cursor += 1;         
        }
    }

    pub fn parse_restore_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return Stmt::Restore(String::from("")); 
        }
        if tokens[*cursor].token_type != TokenType::LineNumber {
            return self.raise_error(ErrorType::LineNumberNeeded, cursor, error_status)
        } 
        *cursor += 1;
        if *cursor == tokens.len() {
            return Stmt::Restore(tokens[*cursor-1].value.clone());
        }
        self.end_of_instruction(cursor, error_status)
    }

    pub fn parse_read_stmt(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        let mut vars = Vec::new();
        *cursor += 1;
        loop {
            if *cursor == tokens.len() {
                return self.raise_error(ErrorType::VariableNameIsNeeded, cursor, error_status)
            }
            if tokens[*cursor].token_type != TokenType::Identifier {
                return self.raise_error(ErrorType::VariableNameIsNeeded, cursor, error_status)
            }
            let arg = self.parse_expression(tokens, cursor, error_status);
            if error_status.error_type != ErrorType::Ok { return self.forward_parse_error(cursor, error_status) }
            vars.push(arg);
            //*cursor += 1;
            if *cursor == tokens.len() { return Stmt::Read(vars) }
            // Require , before next arg
            if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                return Stmt::Empty
            }
            *cursor += 1;
        }
    }

    pub fn parse_line_number_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return Stmt::LineNumber(tokens[*cursor-1].clone()) 
        }
        self.end_of_instruction(cursor, error_status)
    }

    pub fn parse_on_error_stmt(&self, tokens: &[Token], cursor: &mut usize) -> Stmt {
        *cursor += 1;
        if *cursor == tokens.len() {
            return Stmt::OnError(true)
        }
        if tokens[*cursor].token_type == TokenType::Off { return Stmt::OnError(false) }
        Stmt::OnError(true)
    }

    pub fn parse_set_deg_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        let mut v = false;
        if *cursor == tokens.len() {
            error_status.error_type = ErrorType::TrueFalseValueNeeded;
            return Stmt::Empty
        }
        match tokens[*cursor].token_type {
            TokenType::True => v = true,
            TokenType::False => {},
            _ => error_status.error_type = ErrorType::TrueFalseValueNeeded
        }
        *cursor += 1;
        if *cursor == tokens.len() { return Stmt::SetDeg(v) }
        self.end_of_instruction(cursor, error_status)
    }

    pub fn parse_set_rad_stmt(&self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        *cursor += 1;
        let mut v = false;
        if *cursor == tokens.len() {
            error_status.error_type = ErrorType::TrueFalseValueNeeded;
            return Stmt::Empty
        }
        match tokens[*cursor].token_type {
            // because SET RAD has the opposite effect as SET DEG:
            TokenType::True => {}
            TokenType::False => v = true,
            _ => error_status.error_type = ErrorType::TrueFalseValueNeeded
        }
        *cursor += 1;
        if *cursor == tokens.len() { return Stmt::SetDeg(v) }
        self.end_of_instruction(cursor, error_status)
    }

    pub fn parse_on_goto_gosub_stmt(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        let expr = self.parse_expression(tokens, cursor, error_status);
        if error_status.error_type != ErrorType::Ok { return self.forward_parse_error(cursor, error_status) }

        let mut labels = Vec::new();

        match tokens[*cursor].token_type {
            TokenType::Goto => {
                *cursor +=1;
                loop {
                    let label = match tokens[*cursor].token_type {
                        TokenType::LineNumber => tokens[*cursor].value.clone(),
                        TokenType::NumberLiteral => {
                            if tokens[*cursor].identifier_or_literal_type == IdentifierOrLiteralType::Integer {
                                tokens[*cursor].value.clone() 
                            }
                            else {
                                return self.raise_error(ErrorType::LineNumberNeeded, cursor, error_status)
                            }
                        },
                        _ => return self.raise_error(ErrorType::LineNumberNeeded, cursor, error_status)
                    };
                    labels.push(label);
                    *cursor += 1;
                    if *cursor == tokens.len() {
                        return Stmt::OnGoto { expr, labels }
                    }
                    if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                        return Stmt::Empty
                    }
                    *cursor += 1;
                }
            }
            TokenType::Gosub => {
                *cursor +=1;
                loop {
                    let label = match tokens[*cursor].token_type {
                        TokenType::Identifier => {
                            if tokens[*cursor].identifier_or_literal_type == IdentifierOrLiteralType::Float {
                                tokens[*cursor].value.clone() 
                            }
                            else {
                                return self.raise_error(ErrorType::SubroutineLabelNeeded, cursor, error_status)
                            }
                        },
                        _ => return self.raise_error(ErrorType::SubroutineLabelNeeded, cursor, error_status)
                    };
                    labels.push(label);
                    *cursor += 1;
                    if *cursor == tokens.len() {
                        return Stmt::OnGosub { expr, labels }
                    }
                    if !self.check_type(TokenType::Comma, ErrorType::CommaSeparatorIsNeeded, tokens, cursor, error_status) {
                        return Stmt::Empty
                    }
                    *cursor += 1;
                }
            }
            _ => return self.raise_error(ErrorType::GotoOrGosubIsMissing, cursor, error_status)
        }


        //*cursor += 1;
        //if *cursor == tokens.len() {
        //    return Stmt::OnError(true)
        //}
        //if tokens[*cursor].token_type == TokenType::Off { return Stmt::OnError(false) }
        //Stmt::OnError(true)
    }

    /// Entry point for statement parsing.
    pub fn parse_statement(&mut self, tokens: &[Token], cursor: &mut usize, error_status: &mut ErrorStatus) -> Stmt {
        
        if tokens.len() == 0 { return Stmt::Empty };

        // Skip line number:
        if let Some(first_token) = tokens.first() {
            if first_token.token_type == TokenType::LineNumber {
                *cursor += 1;
            }
        }

        // Assignment? (Identifier followed by ColonEquals)
        if let Some(first) = self.peek(tokens, *cursor) {
            match first.token_type {
                TokenType::Identifier => {
                    let name = tokens[*cursor].clone();
                    *cursor += 1;

                    if name.is_array {
                        let subscripts = self.parse_subscripts(tokens, cursor, error_status);
                        if error_status.error_type != ErrorType::Ok {
                            return self.forward_parse_error(cursor, error_status);
                        }
                        // Is array assignment:
                        if !self.check_type(TokenType::ColonEquals, ErrorType::EndOfInstructionExpected, tokens, cursor, error_status) {
                            return Stmt::Empty;
                        }
                        *cursor += 1;
                        let value = self.parse_expression(tokens, cursor, error_status);
                        return Stmt::ArrayAssignment { name, subscripts, value };
                    } 
                    
                    if self.match_tokens(tokens, cursor, &[TokenType::ColonEquals]) {
                        // Standard variable assignment:
                        let value = self.parse_expression(tokens, cursor, error_status);
                        return Stmt::Assignment { name, value };
                    }
                    *cursor -= 1;
                    if name.identifier_or_literal_type == IdentifierOrLiteralType::Float {
                        // might be a procedure call:
                        if self.line_number == 0 { self.compile_all = true }
                        return self.parse_procedure_call_stmt(tokens, cursor, error_status);
                    } else {
                        error_status.error_type = ErrorType::AssignmentExpected;
                    }
                },                
                TokenType::List => return self.parse_bare_stmt(Stmt::List, tokens, cursor, error_status), // TODO: Fully implement LIST
                TokenType::Run => {
                    if self.line_number == 0 { self.compile_all = true }
                    return self.parse_bare_stmt(Stmt::Run, tokens, cursor, error_status)
                },
                TokenType::Bye => return self.parse_bare_stmt(Stmt::Bye, tokens, cursor, error_status),
                TokenType::Break => return self.parse_bare_stmt(Stmt::Break, tokens, cursor, error_status),
                TokenType::End => return self.parse_bare_stmt(Stmt::End, tokens, cursor, error_status),
                TokenType::Stop => return self.parse_bare_stmt(Stmt::End, tokens, cursor, error_status), // STOP and END now have the same effect
                TokenType::Print => return self.parse_print_stmt(tokens, cursor, error_status),
                TokenType::Goto => return self.parse_goto_stmt(tokens, cursor, error_status),
                TokenType::Subroutine => return self.parse_subroutine_stmt(tokens, cursor, error_status),
                TokenType::Return => return self.parse_bare_stmt(Stmt::Return, tokens, cursor, error_status),
                TokenType::Gosub => {
                    if self.line_number == 0 { self.compile_all = true }
                    return self.parse_gosub_stmt(tokens, cursor, error_status)
                },
                TokenType::Repeat => return self.parse_bare_stmt(Stmt::Repeat, tokens, cursor, error_status),
                TokenType::Until => return self.parse_until_stmt(tokens, cursor, error_status),
                TokenType::For => return self.parse_for_stmt(tokens, cursor, error_status),
                TokenType::Next => return self.parse_next_stmt(tokens, cursor, error_status),
                TokenType::Function => return self.parse_function_stmt(tokens, cursor, error_status),
                TokenType::Result => return self.parse_result_stmt(tokens, cursor, error_status),
                TokenType::Endfun => return self.parse_bare_stmt(Stmt::Endfun, tokens, cursor, error_status),
                TokenType::Dim => return self.parse_dim_stmt(tokens, cursor, error_status),
                TokenType::Global => return self.parse_global_stmt(tokens, cursor, error_status),
                TokenType::Procedure => return self.parse_procedure_stmt(tokens, cursor, error_status),
                TokenType::Leave => return self.parse_bare_stmt(Stmt::Leave, tokens, cursor, error_status),
                TokenType::Endproc => return self.parse_bare_stmt(Stmt::Endproc, tokens, cursor, error_status),
                TokenType::If => return self.parse_if_stmt(tokens, cursor, error_status),
                TokenType::Then => return self.parse_bare_stmt(Stmt::Then, tokens, cursor, error_status),
                TokenType::Else => return self.parse_bare_stmt(Stmt::Else, tokens, cursor, error_status),
                TokenType::Data => return self.parse_data_stmt(tokens, cursor, error_status),
                TokenType::Restore => return self.parse_restore_stmt(tokens, cursor, error_status),
                TokenType::Read => return self.parse_read_stmt(tokens, cursor, error_status),
                TokenType::NumberLiteral => return self.parse_line_number_stmt(tokens, cursor, error_status),
                TokenType::On => {
                    *cursor += 1;
                    if *cursor == tokens.len() { return self.raise_error(ErrorType::NumericExpressionNeeded, cursor, error_status) }
                    match tokens[*cursor].token_type {
                        TokenType::Error => return self.parse_on_error_stmt(tokens, cursor),
                        _ => return self.parse_on_goto_gosub_stmt(tokens, cursor, error_status)
                    }
                    
                }
                TokenType::Set => {
                    *cursor += 1;
                    if *cursor == tokens.len() { return self.raise_error(ErrorType::UnknownSetAskAttribute, cursor, error_status) }
                    match tokens[*cursor].token_type {
                        TokenType::Deg => return self.parse_set_deg_stmt(tokens, cursor, error_status),
                        TokenType::Rad => return self.parse_set_rad_stmt(tokens, cursor, error_status),
                        _ => return self.raise_error(ErrorType::UnknownSetAskAttribute, cursor, error_status)
                    }
                }
                _ => return self.raise_error(ErrorType::VariableNameIsNeeded, cursor, error_status)
            }
        }

        // If it's not an assignment, it's just an expression
        Stmt::Expression(self.parse_expression(tokens, cursor, error_status))
    }
}

/// Instantiate a new Parser
pub fn new_parser() -> Parser {
    Parser{
        program: HashMap::new(),
        compile_all: false,
        line_number: 0,
    }
}