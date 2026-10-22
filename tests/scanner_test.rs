use rrmbasic::scanner::{TokenType, scan};


#[test]
fn multiple_statements() {
    let tests = [
        ("\n", 1),
        ("1+2\n", 1), 
        ("1+2:3+4\n", 2), 
        ("1+2:3+4:\n", 3),
        ("1+2:3+4::::::::\n", 10)
    ];

    for (source, len) in tests {
        let stmts = scan(&source);
        assert_eq!(stmts.len(), len);
    }
}

#[test]
fn integer_literals_vs_line_numbers() {
    let tests = [
        ("0\n", TokenType::NumberLiteral, "0"),
        ("10\n", TokenType::LineNumber, "10"),
    ];

    for (source, token_type, value) in tests {
        let stmts = scan(&source);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].tokens.len(), 1);
        assert_eq!(stmts[0].tokens[0].token_type, token_type);
        assert_eq!(stmts[0].tokens[0].value, value);
    }
}

#[test]
fn float_literals() {
    let tests = [
        ("0.123\n", TokenType::NumberLiteral, "0.123"),
        ("1.2\n", TokenType::NumberLiteral, "1.2"), 
        ("123.400\n", TokenType::NumberLiteral, "123.400"), 
    ];

    for (source, token_type, value) in tests {
        let stmts = scan(&source);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].tokens.len(), 1);
        assert_eq!(stmts[0].tokens[0].token_type, token_type);
        assert_eq!(stmts[0].tokens[0].value, value);
    }
}

#[test]
fn string_literals() {
    let tests = [
        ("\"\"\n", TokenType::StringLiteral, ""),
        ("\"hello fluffy!\"\n", TokenType::StringLiteral, "hello fluffy!"),
        ("\"hello\n", TokenType::StringLiteral, "hello"), 
        ("\"I said \"\"Hello!\"\"\"\n", TokenType::StringLiteral, "I said \"Hello!\""),
        ("\"I said \"\"Hello!\"\"\n", TokenType::StringLiteral, "I said \"Hello!\""), 
    ];

    for (source, token_type, value) in tests {
        let stmts = scan(&source);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].tokens.len(), 1);
        assert_eq!(stmts[0].tokens[0].token_type, token_type);
        assert_eq!(stmts[0].tokens[0].value, value);
    }
}

#[test]
fn variable_names() {
    let tests = [
        ("bobrito\n", TokenType::Identifier, "Bobrito"),
        ("bobrito_bandito\n", TokenType::Identifier, "Bobrito_Bandito"),
        ("bobrito_bandito%\n", TokenType::Identifier, "Bobrito_Bandito%"),
        ("strange_67var_name\n", TokenType::Identifier, "Strange_67var_Name"),
        ("bobrito$\n", TokenType::Identifier, "Bobrito$"),
        ("bobrito%\n", TokenType::Identifier, "Bobrito%"), 
    ];

    for (source, token_type, value) in tests {
        let stmts = scan(&source);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].tokens.len(), 1);
        assert_eq!(stmts[0].tokens[0].token_type, token_type);
        assert_eq!(stmts[0].tokens[0].value, value);
    }
}

#[test]
fn keyword_names() {
    let tests = [
        ("PRINT\n", TokenType::Print, "PRINT"),
        ("CLS\n", TokenType::Cls, "CLS"), 
        ("DATE$\n", TokenType::DateStr, "DATE$"),
        ("brush\n", TokenType::Brush, "BRUSH"),
        ("bUtToNs\n", TokenType::Buttons, "BUTTONS"),
    ];

    for (source, token_type, value) in tests {
        let stmts = scan(&source);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].tokens.len(), 1);
        assert_eq!(stmts[0].tokens[0].token_type, token_type);
        assert_eq!(stmts[0].tokens[0].value, value);
    }
}

#[test]
fn one_char_tokens() {
    let tests = [
        ("-\n", vec![
            (TokenType::Minus, "-")
        ]),
        ("+-\n", vec![
            (TokenType::Plus, "+"),
            (TokenType::Minus, "-")
        ]), 
        ("*-=\n", vec![
            (TokenType::Star, "*"),
            (TokenType::Minus, "-"),
            (TokenType::Equals, "=")
        ]), 
    ];

    for (source, expected) in tests {
        let stmts = scan(&source);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].tokens.len(), expected.len());
        for (i, t) in stmts[0].tokens.iter().enumerate() {
            let (token_type, value) = expected[i];
            assert_eq!(t.token_type, token_type);
            assert_eq!(t.value, value);
        }
    }
}

#[test]
fn two_char_tokens() {
    let tests = [
        (":=\n", vec![
            (TokenType::ColonEquals, ":=")
        ])
    ];

    for (source, expected) in tests {
        let stmts = scan(&source);
        assert_eq!(stmts.len(), 1);
        assert_eq!(stmts[0].tokens.len(), expected.len());
        for (i, t) in stmts[0].tokens.iter().enumerate() {
            let (token_type, value) = expected[i];
            assert_eq!(t.token_type, token_type);
            assert_eq!(t.value, value);
        }
    }
}

