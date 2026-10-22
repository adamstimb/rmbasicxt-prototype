use std::collections::HashMap;

use crate::scanner::IdentifierOrLiteralType;
use crate::vm::CallableType;
use crate::vm::ErrorStatus;
use crate::vm::Compiler;
use crate::vm::ErrorType;
use crate::vm::Opcode;

use crate::vm::Stmt;
use crate::vm::Parser;

// These enums are just for the simple structure testing and are not public
// because they might clash or be conflated with similar defintions elsewhere:
#[derive(Debug)]
enum TestLoopType {
    For,
    Repeat
}

struct TestLoop {
    name: String,
    test_loop_type: TestLoopType
}

enum TestCallableType {
    None,
    Subroutine,
    Function,
    Procedure
}



/// Parse the entire program and test the structure so that callable definitions
/// and loops are compliant with the following rules:
/// 
/// - No nested callables <- OK
/// - No callables defined in loops <- OK
/// - RESULT can only be used inside a FUNCTION block
/// - Callables terminated with the appropriate terminator (e.g. ENDFUN for FUNCTION etc) <- OK
/// - Loops terminated with the approriate terminator (e.g. NEXT for FOR etc) <- OK
/// - Program cannot end with unterminated loops or callables
/// 
/// This is to simplify integrity checking at Runtime, which will still catch a lot
/// of stuff, e.g. functions without RESULT.
/// 
fn structure_test(parser: &mut Parser, compiler: &mut Compiler, error_status: &mut ErrorStatus) {

    let mut line_numbers: Vec<_> = parser.program.keys().copied().collect();
    line_numbers.sort();

    let mut scope_map: HashMap<usize, Vec<usize>> = HashMap::new();       // Map of line numbers and statements by their "degrees of scope" - for the safe GOTO check
    let mut scope: usize = 0;

    let mut loop_stack = Vec::new();                       // Support nested loops, so we need a stack
    let mut current_callable = TestCallableType::None;  // Nested callables are not supported 

    let mut registered_callable_labels: HashMap<String, TestCallableType> = HashMap::new();

    for (.., line_number) in line_numbers.iter().enumerate() {
        let mut scopes = Vec::new();

        if *line_number > 0 && !compiler.compile_all { break }

        let parsed_line = parser.program.get(line_number).unwrap().clone();
        for (stmt_number, stmt) in parsed_line.iter().enumerate() {
            
            
            error_status.statement_number = stmt_number;
            error_status.line_number = *line_number;
            parser.line_number = *line_number;
            let mut cursor = 0;
            let stmt = parser.parse_statement(&stmt.tokens, &mut cursor, error_status);

            // Break on the first-encountered parse error
            if error_status.error_type != ErrorType::Ok { break }

            // Do we need to compile the stored program as well?
            if *line_number == 0 { compiler.compile_all = parser.compile_all }

            // Detect new loop definitions...           
            match stmt {
                Stmt::For { control_var,..} 
                    => {
                        loop_stack.push(TestLoop{ name: control_var.value, test_loop_type: TestLoopType::For}); 
                        scope += 1;
                        scopes.push(scope);
                        continue 
                    }
                Stmt::Repeat => {
                    loop_stack.push(TestLoop{ name: String::from(""), test_loop_type: TestLoopType::Repeat}); 
                    scope += 1;
                    scopes.push(scope);
                    continue 
                }
                // ...and catch out-of-context RESULT... 
                Stmt::Result(_) => {
                    match current_callable {
                        TestCallableType::Function => {}
                        _ => {
                            error_status.error_type = ErrorType::FunctionExitWithoutCall;
                            break
                        }
                    }
                }
                Stmt::Leave => {
                    match current_callable {
                        TestCallableType::Procedure => {}
                        _ => {
                            error_status.error_type = ErrorType::ProcedureExitWithoutCall;
                            break
                        }
                    }
                }
                // ...and don't allow RUN command from within a stored program or callable or loop:
                Stmt::Run => {
                    match current_callable {
                        TestCallableType::None => {},
                        _ => {
                            error_status.error_type = ErrorType::CannotRunAProgramFromHere;
                            break
                        }
                    }
                    if loop_stack.len() > 0 {
                        error_status.error_type = ErrorType::CannotRunAProgramFromHere;
                        break
                    }
                }
                _ => { }
            }

            // Ensure loop structure:
            if let Some(current_loop) = loop_stack.pop() {
                match current_loop.test_loop_type {
                    TestLoopType::For => {
                        match stmt {
                            // No callable definitions in a loop
                            Stmt::Subroutine(_) => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Return => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Function { name: _, args: _} => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Endfun => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Procedure { .. } => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Endproc => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            // Require appropriate loop terminator
                            Stmt::Until(_) => error_status.error_type = ErrorType::UntilWithoutAnyRepeat,
                            Stmt::Next(name) => { 
                                if name.identifier_or_literal_type != IdentifierOrLiteralType::None {
                                    // need to check control vars match
                                    if name.value != current_loop.name {
                                        error_status.error_type = ErrorType::NextWithoutMatchingFor
                                    }
                                }
                                scope -= 1;
                                scopes.push(scope);
                                continue 
                            }
                            // Just part of the loop so repush it:
                            _ => {
                                loop_stack.push(current_loop); 
                                scopes.push(scope);
                                continue 
                            }
                        }
                    }
                    TestLoopType::Repeat => {
                        match stmt {
                            // No callable definitions in a loop
                            Stmt::Subroutine(_) => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Return => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Function { name:_,  args: _ } => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Endfun => error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Procedure { .. }=> error_status.error_type = ErrorType::CannotExecuteDefinition,
                            Stmt::Endproc => error_status.error_type = ErrorType::CannotExecuteDefinition,

                            // Require appropriate loop terminator
                            Stmt::Until(_) => {
                                scope -= 1;
                                scopes.push(scope);
                                continue 
                            }
                            Stmt::Next(_) => error_status.error_type = ErrorType::NextWithoutMatchingFor,

                            // Just part of the loop so repush it:
                            _ => { 
                                loop_stack.push(current_loop); 
                                scopes.push(scope);
                                continue 
                            }
                        }                        
                    }
                }
            } else {
                match stmt {
                    Stmt::Until(_) => error_status.error_type = ErrorType::UntilWithoutAnyRepeat, 
                    Stmt::Next(_) => error_status.error_type = ErrorType::NextWithoutMatchingFor,   
                    _ => {}                 
                }
            }

            if error_status.error_type != ErrorType::Ok { break } 

            // Detect new callable definitions...
            match stmt {
                Stmt::Subroutine(name) => {
                    match current_callable {
                        // New subroutine
                        TestCallableType::None => {                          
                            current_callable = TestCallableType::Subroutine;
                            let label = compiler.infer_callable_label(CallableType::Subroutine, name);
                            registered_callable_labels.insert(label, TestCallableType::Subroutine);
                            scope += 1;
                        }
                        // No nested callables:
                        TestCallableType::Subroutine => error_status.error_type = ErrorType::CannotExecuteDefinition,
                        TestCallableType::Function => error_status.error_type = ErrorType::CannotExecuteDefinition,
                        TestCallableType::Procedure => error_status.error_type = ErrorType::CannotExecuteDefinition,                     
                    }
                }
                Stmt::Return => {
                    match current_callable {
                        // Require appropriate callable terminator:
                        TestCallableType::Subroutine => {
                            current_callable = TestCallableType::None;
                            scope -= 1;
                        }
                        _ => error_status.error_type = ErrorType::ReturnWithoutSubroutineDefinition,
                    }
                }
                Stmt::Function { name, args: _ } => {
                    match current_callable {
                        // New function
                        TestCallableType::None => {
                            current_callable = TestCallableType::Function;
                            let label = compiler.infer_callable_label(CallableType::Function, name.value.clone());
                            registered_callable_labels.insert(label, TestCallableType::Function);
                            scope += 1;
                        }
                        // No nested callables:
                        TestCallableType::Subroutine => error_status.error_type = ErrorType::CannotExecuteDefinition,
                        TestCallableType::Function => error_status.error_type = ErrorType::CannotExecuteDefinition,
                        TestCallableType::Procedure => error_status.error_type = ErrorType::CannotExecuteDefinition,                     
                    }
                }
                Stmt::Endfun => {
                    match current_callable {
                        // Require appropriate callable terminator:
                        TestCallableType::Function => {
                            current_callable = TestCallableType::None;
                            scope -= 1;
                        }
                        _ => error_status.error_type = ErrorType::EndfunWithoutFunctionDefinition,
                    }
                }
                Stmt::Procedure { name, .. } => {
                    match current_callable {
                        // New procedure
                        TestCallableType::None => {
                            current_callable = TestCallableType::Procedure;
                            let label = compiler.infer_callable_label(CallableType::Procedure, name.value.clone());
                            registered_callable_labels.insert(label, TestCallableType::Procedure);
                            scope += 1;
                        }
                        // No nested callables:
                        TestCallableType::Subroutine => error_status.error_type = ErrorType::CannotExecuteDefinition,
                        TestCallableType::Function => error_status.error_type = ErrorType::CannotExecuteDefinition,
                        TestCallableType::Procedure => error_status.error_type = ErrorType::CannotExecuteDefinition,                          
                    }
                }
                Stmt::Endproc => {
                    match current_callable {
                        // Require appropriate callable terminator:
                        TestCallableType::Procedure => {
                            current_callable = TestCallableType::None;
                            scope -= 1;
                        }
                        _ => error_status.error_type = ErrorType::EndprocWithoutProcedureDefinition,
                    }
                }
                _ => {}
            }
            scopes.push(scope);
            if error_status.error_type != ErrorType::Ok { break }
        }
        scope_map.insert(*line_number, scopes);
        if error_status.error_type != ErrorType::Ok { break }
    }
    if error_status.error_type != ErrorType::Ok { return }

    // No unterminated loops or callables:
    if loop_stack.len() > 0 {
        let lp = loop_stack.pop().unwrap();
        match lp.test_loop_type {
            TestLoopType::For => error_status.error_type = ErrorType::ForLoopWithoutNext,
            TestLoopType::Repeat => error_status.error_type = ErrorType::RepeatWhileLoopWithoutWhile,
        }
    }

    if error_status.error_type != ErrorType::Ok { return }

    match current_callable {
        TestCallableType::None => {},
        TestCallableType::Subroutine => error_status.error_type = ErrorType::SubroutineDefinitionWithoutReturn,
        TestCallableType::Procedure => error_status.error_type = ErrorType::ProcedureDefinitionWithoutEndproc,
        TestCallableType::Function => error_status.error_type = ErrorType::FunctionDefinitionWithoutEndfun,
    }

    if !compiler.compile_all { return }

    // Unsafe GOTO detection:
    for (.., source_line) in line_numbers.iter().enumerate() {
        let parsed_line = parser.program.get(source_line).unwrap().clone();
        for (stmt_number, stmt) in parsed_line.iter().enumerate() {
            if *source_line == 0 { continue }
            
            error_status.statement_number = stmt_number;
            error_status.line_number = *source_line;
            compiler.line_number = *source_line;
            compiler.statement_number = stmt_number;
            let mut cursor = 0;
            let stmt = parser.parse_statement(&stmt.tokens, &mut cursor, error_status);

            let source_scope = scope_map.get(source_line).unwrap()[stmt_number];

            match stmt {
                Stmt::Goto(label) => {
                    
                    let line: u32 = label.parse().unwrap();
                    let destination_line = line as usize;
                    let destination_scope = match scope_map.get(&destination_line) {
                       Some(destination_scope) => destination_scope[0],
                       None => { error_status.error_type = ErrorType::SpecifiedLineNotFound; break }
                    };

                    // Shortcut for most likely scenario - GOTO to and from the base scope:
                    if source_scope == 0 && destination_scope == 0 {
                        continue
                    } else {
                        // Traverse the map and ensure scope does not change:
                        for traversing_line in line_numbers.iter() {
                            // Zip ahead to source line:
                            if traversing_line < source_line { continue }
                            let scopes_on_traversing_line = scope_map.get(traversing_line).unwrap();
                            for (traversing_stmt_number, this_scope) in scopes_on_traversing_line.iter().enumerate() {
                                // On hitting the source_line, zip to actual GOTO statement:
                                if traversing_line == source_line {
                                    if traversing_stmt_number < stmt_number { continue }
                                }
                                if *this_scope != source_scope {
                                    error_status.error_type = ErrorType::UnsafeGoto;
                                    break
                                }
                            }
                            if error_status.error_type != ErrorType::Ok { return }
                        }
                    }
                },
                Stmt::OnGoto { expr:_, labels }=> {
                    for label in labels {
                        let line: u32 = label.parse().unwrap();
                        let destination_line = line as usize;
                        let destination_scope = match scope_map.get(&destination_line) {
                           Some(destination_scope) => destination_scope[0],
                           None => { error_status.error_type = ErrorType::SpecifiedLineNotFound; break }
                        };

                        // Shortcut for most likely scenario - GOTO to and from the base scope:
                        if source_scope == 0 && destination_scope == 0 {
                            continue
                        } else {
                            // Traverse the map and ensure scope does not change:
                            for traversing_line in line_numbers.iter() {
                                // Zip ahead to source line:
                                if traversing_line < source_line { continue }
                                let scopes_on_traversing_line = scope_map.get(traversing_line).unwrap();
                                for (traversing_stmt_number, this_scope) in scopes_on_traversing_line.iter().enumerate() {
                                    // On hitting the source_line, zip to actual GOTO statement:
                                    if traversing_line == source_line {
                                        if traversing_stmt_number < stmt_number { continue }
                                    }
                                    if *this_scope != source_scope {
                                        error_status.error_type = ErrorType::UnsafeGoto;
                                        break
                                    }
                                }
                                if error_status.error_type != ErrorType::Ok { return }
                            }
                        }
                    }
                }
                _ => {}
            }
            if error_status.error_type != ErrorType::Ok { return }
        }
        if error_status.error_type != ErrorType::Ok { return }
    }
}


pub fn compile_program(parser: &mut Parser, compiler: &mut Compiler, error_status: &mut ErrorStatus) -> usize {

    // Do structure test before compiling:
    structure_test(parser, compiler, error_status);
    if error_status.error_type != ErrorType::Ok { return 0 }

    let mut entry_ip: usize = 0;
    let mut line_numbers: Vec<_> = parser.program.keys().copied().collect();
    line_numbers.sort();

    // 2-pass compilation. Pass 1 compiles the callables, Pass 2 compiles the
    // rest of the program.  To prevent recompilation we log each statement sequentially
    // in the 'done' vector.  'sequence' is different to statement_number as is it
    // corresponds to the global order of the statement, not the order of the statement
    // in a single program line. It's inefficient to parse the program multiple times but
    // come on it's RM Basic in Rust nobody cares.
    //
    let mut done: Vec<i32> = Vec::new();
    let mut callables_are_registered = false;
    for pass in 1..3 {
        let mut sequence = -1;
        for (_, line_number) in line_numbers.iter().enumerate() {

            if *line_number > 0 && !compiler.compile_all { break }

            // Pass 2: At the start of this pass, register the callables and set the flag:
            if pass == 2 && !callables_are_registered {
                for (l,s, c) in compiler.register_callable_code.code.iter() {
                    compiler.chunk.write_opcode(*l, *s, c.clone());
                }
                callables_are_registered = true
            }

            // Pass 2: Make sure line 0 (immediate mode) finishes with END followed by
            // the run_from label:
            if pass == 2 {
                if *line_number == 0 { compiler.need_opcode_end = true };
                if *line_number != 0 && compiler.need_opcode_end {
                    compiler.need_opcode_end = false;
                    compiler.chunk.write_opcode(0, 0, Opcode::End);
                    let label = String::from("___run_from_here___");
                    compiler.labels.insert(label.clone(), compiler.chunk.code.len());
                    compiler.chunk.write_opcode(compiler.line_number, compiler.statement_number, Opcode::Label(label));
                    compiler.chunk.write_opcode(compiler.line_number, compiler.statement_number, Opcode::ClearVars);
                }
            }

            let parsed_line = parser.program.get(line_number).unwrap().clone();
            for (stmt_number, stmt) in parsed_line.iter().enumerate() {
                sequence += 1;

                error_status.statement_number = stmt_number;
                error_status.line_number = *line_number;
                let mut cursor = 0;
                let stmt = parser.parse_statement(&stmt.tokens, &mut cursor, error_status);
                if error_status.error_type != ErrorType::Ok { break }
                // On the first pass, forward fast to a callable:
                if pass == 1 && !compiler.in_callable {
                    match stmt {
                        Stmt::Subroutine(_) => { }
                        Stmt::Procedure { .. } => {}
                        Stmt::Function { .. } => {}
                            _ => continue,
                        }
                }

                // On the second pass, skip code compiled in the first pass:
                if pass == 2 && done.contains(&sequence) {
                    continue
                }

                compiler.line_number = *line_number;
                compiler.statement_number = stmt_number;
                compiler.compile_statement(&stmt, error_status);
                done.push(sequence);

                // In the first pass, update the entry point:
                if pass == 1 { entry_ip = compiler.chunk.code.iter().count() }
                if error_status.error_type != ErrorType::Ok { break }
            }

            if error_status.error_type != ErrorType::Ok { break }

            // Add ON ERROR OK label if required:
            if compiler.need_on_error_ok {
                let label = compiler.infer_on_error_ok_label();
                compiler.labels.insert(label.clone(), compiler.chunk.code.len());
                compiler.chunk.write_opcode(compiler.line_number, compiler.statement_number, Opcode::Label(label));
                compiler.need_on_error_ok = false;
            }

            // Unpack IF statements without ELSE:
            if compiler.if_stack.len() > 0 {
                while compiler.if_stack.len() > 0 {
                    let label = compiler.infer_else_label();
                    compiler.labels.insert(label.clone(), compiler.chunk.code.len());
                    compiler.chunk.write_opcode(compiler.line_number, compiler.statement_number, Opcode::Label(label));
                    _ = compiler.if_stack.pop();
                }
                let label = compiler.infer_end_if_label();
                compiler.labels.insert(label.clone(), compiler.chunk.code.len());
                compiler.chunk.write_opcode(compiler.line_number, compiler.statement_number, Opcode::Label(label));
            }

            // Add END IF label if required:
            if compiler.need_end_if {
                println!("printing end if label");
                let label = compiler.infer_end_if_label();
                compiler.labels.insert(label.clone(), compiler.chunk.code.len());
                compiler.chunk.write_opcode(compiler.line_number, compiler.statement_number, Opcode::Label(label));
                compiler.need_end_if = false;
            }
        }
        if error_status.error_type != ErrorType::Ok { break }
    }
    compiler.finish();

    if error_status.error_type != ErrorType::Ok { return 0 }

    // Patch jumps and return entry point if the program is valid:
    let code = compiler.chunk.code.clone();
    for (ip, (line_number, statement_number, instruction)) in code.iter().enumerate() {
        error_status.line_number = *line_number;
        error_status.statement_number = *statement_number;
        match instruction {
            Opcode::JumpToLine(line) => {
                let jump_to_ip = compiler.chunk.program_location_to_ip.get(&(*line, 0));
                if let Some(jump_to_ip) = jump_to_ip {
                    compiler.chunk.overwrite_opcode(ip, *line_number, *statement_number, Opcode::Jump(*jump_to_ip));
                } else {
                    error_status.error_type = ErrorType::SpecifiedLineNotFound;
                }
            }
            Opcode::JumpToLabel { label, callable_type } => {
                let jump_to_ip = compiler.labels.get(label);
                if let Some(jump_to_ip) = jump_to_ip {
                    compiler.chunk.overwrite_opcode(ip, *line_number, *statement_number, Opcode::Jump(*jump_to_ip));
                } else {
                    match callable_type { 
                        CallableType::Subroutine => error_status.error_type = ErrorType::UnknownSubroutine,
                        CallableType::Procedure => error_status.error_type = ErrorType::UnknownCommandProcedure,
                        CallableType::Function => error_status.error_type = ErrorType::FunctionArrayNotFound,
                        CallableType::None => panic!("Trying to patch a None callable_type - could be a bad RUN, ELSE, THEN, or BREAK."),
                    }
                }
            }
            Opcode::JumpIfFalseToLabel(label) => {
                let jump_to_ip = compiler.labels.get(label);
                if let Some(jump_to_ip) = jump_to_ip {
                    compiler.chunk.overwrite_opcode(ip, *line_number, *statement_number, Opcode::JumpIfFalse(*jump_to_ip));
                } else {
                    error_status.error_type = ErrorType::SpecifiedLineNotFound;
                }
            }
            _ => {}
        }
        if error_status.error_type != ErrorType::Ok { break }
    }
    entry_ip
}