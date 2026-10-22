use std::io;
use std::io::Write;
use std::process;

use rrmbasic::compiler::parser::new_parser;
use rrmbasic::scanner::scan;
use rrmbasic::vm::VM;




fn main() {
    let p = new_parser();
    let mut vm = VM::new(p);

    println!("");
    println!("   ___  __  ______           _     _  ________");
    println!("  / _ \\/  |/  / _ )___ ____ (_)___| |/_/_  __/");
    println!(" / , _/ /|_/ / _  / _ `(_-</ / __/>  <  / /   ");
    println!("/_/|_/_/  /_/____/\\_,_/___/_/\\__/_/|_| /_/     **  PROTOTYPE **");
    println!("");
    println!("Independent project. \"RM\" and \"RM Nimbus\" are trademarks of RM plc.");
    println!("");
    println!("This is a prototype REPL for debugging purposes only. Check out");
    println!("https://github.com/adamstimb/rmbasicxt-prototype for details.");
    println!("");
    println!("Type BYE or hit CTRL-C to exit.");
    println!("");

    loop {
        print!(":");
        let _ = io::stdout().flush();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        let stmts = scan(&input);
        //println!("Scanner returned these statements:");
        //for (sequence, stmt) in stmts.iter().enumerate() {
        //    println!("Statement #{}: {:#?}", sequence, stmt);
        //}
        vm.handle_repl_input(stmts);

        // Handle shutdown signal
        if vm.shutdown {
            process::exit(1);
        }

        print!("{}", vm.get_error_msg());
    }
}
