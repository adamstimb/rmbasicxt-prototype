pub mod token;
pub mod keyword_map;
pub mod stylize_variable_name;


pub use token::{Token, TokenType, Statement, IdentifierOrLiteralType};
pub use keyword_map::get_keyword_map;
pub use stylize_variable_name::stylize;

#[derive(Debug, Copy, Clone)]
struct Scanner<'a> {
    source_str: &'a str, 
    source: &'a [u8],
    current: usize
}

impl<'a> Scanner<'a> {
    /// Hard-protection against invalid input strings containing Unicode and missing
    /// the termination char.  Will panic if an invalid string is passed.
    pub fn validate(&self) {
        if !self.source_str.is_ascii() {
            panic!("Scanner.validate: Cannot accept Unicode in source string");
        }
        if self.source[self.source.len()-1] != b'\n' {
            panic!("Scanner.validation: Termination char missing from source string");
        }
    }

    pub fn advance(&mut self) -> u8 {
        self.current+=1;
        return self.source[self.current-1]
    }

    pub fn retreat(&mut self) {
        self.current-=1;
    }

    pub fn peek(&self) -> u8 {
        self.source[self.current]
    }

    pub fn peek_next(&self) -> u8 {
        if self.current >= self.source.len() {
            self.source[self.current]
        } else {
            self.source[self.current+1]
        }
    }

    pub fn get_string(&self, from: usize, to: usize) -> String {
        self.source_str[from..to].to_string()
    }

    pub fn get_1_char_token(&self, token_type: TokenType) -> Token {
        Token{
            token_type: token_type, 
            value: self.get_string(self.current-1, self.current),
            identifier_or_literal_type: IdentifierOrLiteralType::None,
            is_array: false,
        }
    }

    pub fn get_2_char_token(&mut self, token_type: TokenType) -> Token {
        let t = Token{
            token_type: token_type, 
            value: self.get_string(self.current-1, self.current+1),
            identifier_or_literal_type: IdentifierOrLiteralType::None,
            is_array: false,
        };
        self.advance();
        t
    }

    /// Handles string literals between a pair of double-quote chars, include
    /// escaped double-quotes ("").
    pub fn get_string_literal_token(&mut self) -> Token {
        // Handle special cases of empty literal "" or empty unterminated literal "\n
        if (self.peek() == b'"' && self.peek_next() != b'"') || self.peek() == b'\n' {
            // And consume next " if it's a correctly-terminated empty string:
            if self.peek() == b'"' && self.peek_next() != b'"' { self.advance(); }
            return Token{
                token_type: TokenType::StringLiteral, 
                value: String::from(""),
                identifier_or_literal_type: IdentifierOrLiteralType::String,    // Don't really need to set this but ok
                is_array: false,
            }
        }

        let mut value: Vec<u8> = Vec::new();
        loop {
            let c = self.advance();
            if c == b'"' && self.peek() == b'"' {   // Handled escaped double-quote
                value.push(c);
                self.advance();
                continue;
            }
            if c == b'"' { break; }                 // Properly terminated string
            if c == b'\n' {                         // Handle special case of non-empty unterminated string "hello\n
                self.retreat();
                return Token{
                    token_type: TokenType::StringLiteral, 
                    value: String::from_utf8(value).expect("Invalid chars in string literal"),
                    identifier_or_literal_type: IdentifierOrLiteralType::String,
                    is_array: false,    
                }
            }
            value.push(c);
        }
        
        Token{
            token_type: TokenType::StringLiteral, 
            value: String::from_utf8(value).expect("Invalid chars in string literal"),
            identifier_or_literal_type: IdentifierOrLiteralType::String,
            is_array: false,
        }
    }

    /// Handles numeric literals including integers, floats and floats written
    /// in scientific notation (e.g. 123E04, 1.23E99, 1.23e-2).
    pub fn get_numeric_literal_token(&mut self) -> Token {
        let mut is_float = false;
        let mut is_scientific = false;
        let mut is_negative_exponent = false;
        let from = self.current-1;
        loop {
            let c = self.advance();
            // Allow decimal point in mantissa
            if c == b'.' && !is_float && !is_scientific {
                is_float = true;
                continue;
            }
            // Allow E in scientific notation
            if (c == b'e' || c == b'E') && !is_scientific {
                is_scientific = true;
                is_float = true;
                continue;
            }
            // Allow negative exponent in scientific notation
            if c == b'-' && is_scientific && !is_negative_exponent {
                is_negative_exponent = true;
                continue;
            }
            // Otherwise terminate at any non-numerical char
            if c < b'0' || c > b'9' {
                self.retreat();
                break;
            }
        }
        if is_float {
            Token{
                token_type: TokenType::NumberLiteral, 
                value: self.get_string(from, self.current), 
                identifier_or_literal_type: IdentifierOrLiteralType::Float,
                is_array: false
            }
        } else {
            Token{
                token_type: TokenType::NumberLiteral, 
                value: self.get_string(from, self.current), 
                identifier_or_literal_type: IdentifierOrLiteralType::Integer,
                is_array: false
            }
        }
    }

    pub fn get_identifier_or_keyword_token(&mut self) -> Token {
        let mut is_array = false;
        let from = self.current-1;
        let mut identifier_type = IdentifierOrLiteralType::Float;

        loop {
            let c = self.advance();
            if c == b'%' {
                identifier_type = IdentifierOrLiteralType::Integer;
                break;
            }
            if c == b'$' {
                identifier_type = IdentifierOrLiteralType::String;
                break;
            }
            if (c >= b'A' && c <= b'Z') || 
                (c >= b'a' && c <= b'z') || 
                (c >= b'0' && c <= b'9') || 
                c == b'_' {
                continue;
            } else {
                self.retreat();
                break;
            }
        }

        // Is it an array?
        if self.peek() == b'(' { is_array = true }

        let value_str = self.source_str[from..self.current].to_string();
        
        // Check if actually a keyword
        let km = get_keyword_map();
        let result = km.get(&value_str.to_uppercase());
        match result {
            Some(token_type) => return Token{
                token_type: *token_type, 
                value: value_str.to_uppercase(), 
                identifier_or_literal_type: IdentifierOrLiteralType::None,
                is_array: false,
            },
            None => {},
        }

        let mut stylized= String::new();
        if is_array {
            // we prefix array names with () because users can't do that and it makes
            // things much simpler in the VM particularly when handling GLOBAL arrays:
            stylized.push_str("()");
        }
        stylized.push_str(&stylize(&value_str));

        Token{
            token_type: TokenType::Identifier, 
            value: stylized, 
            identifier_or_literal_type: identifier_type,
            is_array: is_array,
        }
        
    }
}

/// Scans a raw input string and returns a vector of statements.  Panics if an
/// invalid string containing Unicode or missing a termination char is passed.
pub fn scan(input: &str) -> Vec<Statement> {
    let mut s = Scanner{
        source_str: input,
        source: input.as_bytes(),
        current: 0,
    };

    s.validate();

    let mut statements: Vec<Statement> = Vec::new();
    let mut stmt = Statement{tokens: Vec::new()};

    loop {
        let c = s.advance();

        match c {
            b'\n' => {
                statements.push(stmt.clone());
                break;
            }
            b' ' => continue,
            b'\t' => continue,
            b':' => {
                match s.peek() {
                    b'=' => {
                        stmt.tokens.push(s.get_2_char_token(TokenType::ColonEquals));
                    },
                    _ => {
                        statements.push(stmt.clone());
                        stmt.tokens = Vec::new();
                    },
                }
            },
            b'<' => {
                match s.peek() {
                    b'=' => stmt.tokens.push(s.get_2_char_token(TokenType::LessEqual)),
                    b'>' => stmt.tokens.push(s.get_2_char_token(TokenType::Inequality)),
                    _ => stmt.tokens.push(s.get_1_char_token(TokenType::Less))
                }
            },
            b'>' => {
                match s.peek() {
                    b'=' => stmt.tokens.push(s.get_2_char_token(TokenType::GreaterEqual)),
                    _ => stmt.tokens.push(s.get_1_char_token(TokenType::Greater))
                }
            },
            b'=' => {
                match s.peek() {
                    b'=' => stmt.tokens.push(s.get_2_char_token(TokenType::Interesting)),
                    _ => stmt.tokens.push(s.get_1_char_token(TokenType::Equals)),
                }
            },
            b',' => stmt.tokens.push(s.get_1_char_token(TokenType::Comma)),
            b';' => stmt.tokens.push(s.get_1_char_token(TokenType::Semicolon)),
            b'+' => stmt.tokens.push(s.get_1_char_token(TokenType::Plus)),
            b'*' => stmt.tokens.push(s.get_1_char_token(TokenType::Star)),
            b'-' => stmt.tokens.push(s.get_1_char_token(TokenType::Minus)),
            b'/' => stmt.tokens.push(s.get_1_char_token(TokenType::ForwardSlash)),
            b'\\' => stmt.tokens.push(s.get_1_char_token(TokenType::BackSlash)),
            b'^' => stmt.tokens.push(s.get_1_char_token(TokenType::Caret)),
            b'(' => stmt.tokens.push(s.get_1_char_token(TokenType::LeftParen)),
            b')' => stmt.tokens.push(s.get_1_char_token(TokenType::RightParen)),
            b'"' => stmt.tokens.push(s.get_string_literal_token()),
            b'0'..=b'9' => {
                let mut t = s.get_numeric_literal_token();
                // Handle line number at start of input
                if statements.len() == 0 &&
                    stmt.tokens.len() == 0 &&
                    t.token_type == TokenType::NumberLiteral &&
                    t.identifier_or_literal_type == IdentifierOrLiteralType::Integer &&
                    t.value != String::from("0") {
                    t.token_type = TokenType::LineNumber;
                    stmt.tokens.push(t);
                    continue;
                }
                // Handle line number after GOTO or RESTORE
                if stmt.tokens.len() > 0 && 
                    t.token_type == TokenType::NumberLiteral &&
                    t.identifier_or_literal_type == IdentifierOrLiteralType::Integer &&
                    t.value != String::from("0") {
                    if stmt.tokens[stmt.tokens.len()-1].token_type == TokenType::Goto ||
                        stmt.tokens[stmt.tokens.len()-1].token_type == TokenType::Restore {
                        t.token_type = TokenType::LineNumber;
                    }
                }
                stmt.tokens.push(t);
            },
            b'A'..=b'Z' => {
                stmt.tokens.push(s.get_identifier_or_keyword_token());
                // If we just pushed IF, THEN, ELSE we've started a new statement
                // so pop the last token, push the stmt, create a new stmt and push
                // the popped token on to the new stmt, so the new stmt starts with
                // IF/THEN/ELSE ...
                let last_token_type = stmt.tokens[stmt.tokens.len()-1].token_type;
                if last_token_type == TokenType::If ||
                    last_token_type == TokenType::Then ||
                    last_token_type == TokenType::Else {
                        let t = stmt.tokens.pop().unwrap();
                        statements.push(stmt.clone());
                        let mut stmt = Statement{tokens: Vec::new()};
                        stmt.tokens.push(t);
                }
            },
            b'a'..=b'z' => {
                stmt.tokens.push(s.get_identifier_or_keyword_token())
            },
            _ => stmt.tokens.push(s.get_1_char_token(TokenType::IllegalChar)),
        }

        // Handle IF ... THEN ... ELSE
        if stmt.tokens.len() > 0 {
            let last_token_type = stmt.tokens[stmt.tokens.len()-1].token_type;
            if last_token_type == TokenType::If || last_token_type == TokenType::Then || last_token_type == TokenType::Else {
                // If we just pushed IF we've started a new statement so pop the last token, 
                // push the stmt, create a new stmt and push the popped token on to the new 
                // stmt, so the new stmt starts with IF and then it's up to the parser to find
                // the expression:
                let t = stmt.tokens.pop().unwrap();
                statements.push(stmt.clone());
                stmt = Statement{tokens: Vec::new()};
                stmt.tokens.push(t);
                // If we just push THEN or ELSE the stmt is already completed so push it and
                // start a new stmt:
                if last_token_type == TokenType::Then || last_token_type == TokenType::Else {
                    statements.push(stmt.clone());
                    stmt = Statement{tokens: Vec::new()};
                }
            }
        }

        // Handle ON ERROR statement without OFF option:
        if stmt.tokens.len() > 2 {
            let last = &stmt.tokens[stmt.tokens.len()-1];
            let next_to_last1 = &stmt.tokens[stmt.tokens.len()-2];
            let next_to_last2 = &stmt.tokens[stmt.tokens.len()-3];
            if last.token_type != TokenType::Off 
            && next_to_last1.token_type == TokenType::Error 
            && next_to_last2.token_type == TokenType::On {
                let t = stmt.tokens.pop().unwrap();
                statements.push(stmt.clone());
                stmt = Statement{tokens: Vec::new()};
                stmt.tokens.push(t);
            }
        }


    }

    statements
}