use rrmbasic::compiler::parser::new_parser;
use rrmbasic::vm::VM;
use rrmbasic::scanner::scan;
use rrmbasic::vm::opcode::Value;
use rrmbasic::errors::ErrorType;


#[test]
fn error_ok_integer_maths_tests() {
    let tests = vec![
        (String::from("A% := 10 : B% := 20 : C% := 30 : A% := A% + B% * -C% : B% := (A% + B%) * -2 : Result% := A% - B% + C%\n"), Value::Integer(-1700)),
        (String::from("A% := 5 : B% := -3 : C% := 7 : Result% := (A% * (B% - C%) + (A% - B%) * C%) * -(A% + B%)\n"), Value::Integer(-12)),
        (String::from("A% := 100 : B% := 50 : C% := 25 : Result% := A% - B% - C% - 10 + A% * B% * 2\n"), Value::Integer(10015)),
        (String::from("A% := 2 : B% := 3 : C% := 4 : A% := -A% * -B% * -C% : Result% := A% * -B% + -C% * A%\n"), Value::Integer(168)),
        (String::from("A% := 1 : B% := 2 : C% := 3 : A% := A% + B% * C% + B% * (C% + A%) : B% := A% * (A% - B%) : Result% := A% * A% - B% * B% + C%\n"), Value::Integer(-37797)),
        (String::from("A% := 10 : B% := 20 : C% := 30 : Result% := A% + B% * C% - A% * B% + C% * A% - B%\n"), Value::Integer(690)),
        (String::from("A% := -10 : B% := -20 : C% := 100 : A% := A% * (B% + C%) - C% * (A% + B%) : Result% := A% + B% + C%\n"), Value::Integer(2280)),
        (String::from("X% := 12 : Y% := 34 : Z% := 56 : Result% := (X% * Y% * Z% - X% * Y%) * (Z% - X%) + Y%\n"), Value::Integer(987394)),
        (String::from("A% := 2 : B% := 3 : C% := 4 : D% := 5 : Result% := A% + B% * C% * D% - (A% + B%) * (C% + D%) * -A% - D% * C% * B% * A%\n"), Value::Integer(32)),
    ];

    let p = new_parser();
    let mut vm = VM::new(p);
    let name = String::from("Result%");

    for (input, test_value) in tests.iter() {
        let stmts = scan(&input);
        vm.handle_repl_input(stmts);

        assert_eq!(vm.call_stack.len(), 1);
        assert_eq!(vm.call_stack[0].loop_stack.len(), 0);
        assert_eq!(vm.error_status.error_type, ErrorType::Ok);
        assert_eq!(vm.stack.len(), 0);
        
        
        match vm.get_var(&name) {
            Some(v) => {
                assert_eq!(v, test_value);
            }
            None => panic!("Variable {} should exist but doesn't", name)
        }
    }
}

#[test]
fn error_ok_general_one_liner_tests() {
    let tests = vec![
        (String::from("A% := 12345\n"), String::from("A%"), Value::Integer(12345)),
        (String::from("A := 1.2345\n"), String::from("A"), Value::Float(1.2345)),
        (String::from("A$ := \"All play and no work makes Fluffy an annoying, gas-lighting little cat.\"\n"), String::from("A$"), Value::String("All play and no work makes Fluffy an annoying, gas-lighting little cat.".to_string())),
        (String::from("FOR I% := 1 to 500000 : NEXT I%\n"), String::from("I%"), Value::Float(500000.0)), // <-- because FOR handles ints as floats internally
        (String::from("FOR I% := 1 to 500000 : NEXT I% : A% := I%\n"), String::from("A%"), Value::Integer(500000)), // <-- and let's make sure they're cast back to ints
        (String::from("A% := 0 : REPEAT : A% := A% + 1 : UNTIL A% > 499999\n"), String::from("A%"), Value::Integer(500000)),
        (String::from("DIM A%(1000) : A%(500) := 12345 : B% := A%(500)\n"), String::from("B%"), Value::Integer(12345)),
        (String::from("DIM A%(1000) : A%(500) := 12345 : B% := A%(500)\n"), String::from("B%"), Value::Integer(12345)),
        (String::from("DIM A%(10, 10, 10, 10, 10) : A%(5, 5, 5, 5, 5) := 12345 : B% := A%(5, 5, 5, 5, 5)\n"), String::from("B%"), Value::Integer(12345)),
        (String::from("Msg$ := Greeting$() : FUNCTION Greeting$() : RESULT \"Ow do mayun?\" : ENDFUN\n"), String::from("Msg$"), Value::String("Ow do mayun?".to_string())),
        (String::from("Result% := Adder%(100000, 100, 1000) : FUNCTION Adder%(A%, B%, C%) : RESULT A% + B% + C% : ENDFUN\n"), String::from("Result%"), Value::Integer(101100)),
    ];

    let p = new_parser();
    let mut vm = VM::new(p);

    for (input, name, test_value) in tests.iter() {
        let stmts = scan(&input);
        vm.handle_repl_input(stmts);

        assert_eq!(vm.call_stack.len(), 1);
        assert_eq!(vm.call_stack[0].loop_stack.len(), 0);
        assert_eq!(vm.error_status.error_type, ErrorType::Ok);
        assert_eq!(vm.stack.len(), 0);
        
        
        match vm.get_var(name) {
            Some(v) => {
                assert_eq!(v, test_value);
            }
            None => panic!("Variable {} should exist but doesn't", name)
        }
    }
}