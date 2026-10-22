# RM BasicXT Prototype

_It's an RM Basic bytecode compiler written in Rust!_

DISCLAIMER: RM BasicXT is an independent open-source preservation project and is not affiliated with, sponsored by, or endorsed by RM plc. "RM", "RM BASIC", and "RM Nimbus" are trademarks of RM plc.

## Overview

RM BasicXT is a re-implementation of the **RM Basic** language developed for the [RM Nimbus PC](https://en.wikipedia.org/wiki/RM_Nimbus) in 1985.  Unlike the original 16-bit tree-walking interpreter, **RM BasicXT is a 64-bit [Rust](https://rust-lang.org/)-powered bytecode compiler**.

This app is a **prototype** for developing and testing the RM BasicXT compiler.  You can write, compile and execute programs but the output is strictly for **debugging use only**; no fancy 3-channel sounds or 16-colour graphics just yet!

While every effort has been made to stick to the original specification for RM Basic, it turns out **there is no specification**.  Some design choices have been made therefore to account for undocumented features and behaviour, and also to account for the changeover to a **fully compiled language** and **64-bit architecture**.

## Major Differences from RM Basic

### Evaluation Depth

RM Basic &mdash; like all BASIC interpreters of the time &mdash; had a shallow draught when it came to evaluating deep expressions or performing recursive calls.  With its Rust-powered implementation and 64-bit architecture, RM BasicXT is **practically unlimited** in this respect.

One obvious impact is that **floating point numbers** in RM BasicXT are evaluated to an accuracy of **15 decimal places**, whereas in RM Basic they were evaluated to 7 decimal places.

### Error Handling

The original RM Basic implementation was notable for its **intuitive error handling** and warning system.  **Useful error messages** and **token highlighting** were provided that made debugging a less mysterious experience than other languages of the day.

RM BasicXT **preserves the original error handling formula** with a few subtle differences:

- The entire program is compiled **before** it is executed.  The first error raised during compilation halts the process, the error is yielded and program execution itself does not start.  This means **a lot of errors will be revealed immediately**, without having to manually plough through the runtime looking for them.
- **New error messages** have been introduced to account for RM BasicXT's strictness regarding program structure.
- Some error messages are **no longer used** as they became redundant (e.g. "Expression too complicated")

### ON ERROR [ OFF ]

RM Basic enabled the user to **trap errors** and execute **alternative code** instead of halting execution and reporting an error message.  This feature is supported in RM BasicXT with several modifications:

- ON ERROR applies only to **runtime errors**.  Compilation-time errors will still get reported.
- Traps set by ON ERROR in RM BasicXT are **scope-dependent**:
    - The original interpreter did not seem able to handle **unsafe jumps** from within procedure or function calls or loops to the alternate code if ON ERROR was defined in a different scope, causing **unpredictable behaviour**.  Therefore:
    - In RM BasicXT, **ON ERROR traps are passed "up"** when a FOR or REPEAT loop is started, or a Procedure, Function or Subroutine is called.  If an error is caught, the stack in unwound until it reaches the scope where the ON ERROR was defined and _then_ it executes the alternate code.
    - This also means that if an ON ERROR trap was defined in a callable or a loop, **it is no longer active** once that callable or loop returns.
- Error trapping in RM Basic was always a **tricky business**.  RM BasicXT retains that characteristic, but hopefully it has become a little **safer** and a tiny bit more **flexible**.

### IF _expression_ THEN _statements_ [ ELSE _statements_ ]

IF statements are implemented according to RM Basic such that **THEN is required** before the statements to be run.  It turns out there was an undocumented nuance in the original language that made THEN _optional_.  This would raise absolute hell in a compiled language and so RM BasicXT implementes IF THEN ELSE as originally documented: THEN is required.  

### RUN

The RUN command in RM BasicXT **does not accept a line number**.  Starting program execution from an **arbitrary line number** is always going to be a bit flakey in any language, but in a compiled language this is **practically impossible** to achieve reliably.

As a compromise, any procedure, subroutine or function stored in a program can be called with a direct command in the REPL (see [below](#key-features)).   This should making testing and bugfixing much easier if you write your programs in the modern, modular way that the original RM Basic tried to encourage.

### (Deprecated) RESUME

RM Basic provided the RESUME command to **resume program execution** from the last program line to be run before an error or BREAK interrupt.  This feature is **not supported** in RM BasicXT for the simple reason that it would be **an absolute cluster** to do in a compiled language.

### Variable Assignment

Variables in RM BasicXT are assigned like this:

_var_ := _expr_

No doubt in an attempt to placate **bearded die-hardists** of various BASIC dialects of the time, RM Basic also supported 3 **alternative assignement statements**:

- (deprecated) _var_ = _expr_
- (deprecated) LET _var_ = _expr_
- (deprecated) LET _var_ := _expr_

For the sake of clarity, and since no-one seems to care anymore, **RM BasicXT supports only _var_ := _expr_**

LET always seemed redundant to me, even at the time, and using = instead of := is confusing due it being syntactically identical to a boolean expression.

_var_ := _expr_ is the assignment used most in the RM Basic manual and example programs, and feels more intentional and in-keeping with the rest of the language.  [Go](https://go.dev/) uses this exact same assignment syntax today, although &mdash; somewhat ironically &mdash; Rust (the language this compiler is implemented in) uses LET... 🤔 

### Comparitors

- RM Basic accepted both <> and >< as inequality comparitors. For simplicity, **RM BasicXT accepts only <>**.

### Callables

Subroutines, procedures and functions (callables) are defined as per RM Basic with the following constraints:

- A callable definition **cannot be nested** inside another callable or a loop.  RM Basic allowed this as an undocumented feature but with unpredictable outcomes.
- Otherwise callable definitions **can be written anywhere** in the program.  In RM Basic the program had to "skip" callable definitions with a GOTO, or have them written after an END or STOP statement.  RM BasicXT searches for and compiles all callables first regardless of their order of appearance in a program, and sets the entry point after the final callable, so they are skipped by default.
- Callables defined in a stored program can be called from a direct command, just like in RM Basic, but **the entire program must be compiled first** and, if errors are yielded even outside the callable, execution will not proceed.  Otherwise the callable is executed after compilation and control returned to the user.
- **GOSUB** only supports **subroutine calls**.  Unlike RM Basic, GOSUB to a line number is not supported.  Similarly RETURN outside of a subroutine is not supported.

### Inline DATA

RM BasicXT supports inline program data in much the same way as RM Basic with one subtle difference: **String data won't be inferred** - it has to be written in the form of **string literals**.  What does that mean?  Well, the original RM Basic interpreter would infer data from the following statements like this:

| Statement | Inferred data |
| --------- | ------------- |
| DATA "Tinker", "Tailor" | String("Tinker"), String("Tailor") |
| DATA "Tinker, Tailor" | String("Tinker, Tailor") |
| DATA Tinker, Tailor | String("Tinker"), String("Tailor") |
| DATA Tinker , Tailor | String("Tinker "), String("Tailor") |

For the sake of saving a few double-quote characters I decided it wouldn't be worth patching RM BasicXT's scanner to support whatever's going on here.  Therefore, **string data must be written as string literals**, like this:

```
DATA "Tinker", "Tailor", "Soldier", "Spy"
```

Writing it like this:

```
DATA Tinker, Tailor, Soldier, Spy
```

will yield a "String or number expected" error.

Also, **RESTORE cannot be used in a direct command** to restore data from a stored program.  For example:

```
:10 DATA 1,2,3,4,5,6,7,8,9,10
:
:RESTORE 10
No more data to be read
>>> RESTORE 10
:_
```

As with RM Basic, **DATA statements can be written absolutely anywhere** in the program as they are collected during compilation and have no effect during execution.

### Special mention: Safe GOTO

Yes, **of course** GOTO is supported!  There is **nothing wrong with GOTO**.  I mean, under the hood all languages use GOTO, am I right?  

The use of GOTO in RM Basic was generally discouraged because a) RM Basic's comprehensive support for structured programming _should_ have made GOTO redundant, and b) using GOTO to jump in and out of loops and callables would result in all sorts of unpredictable behaviour, although it was _very common practice_.

RM BasicXT provides "Safe GOTO":  During compilation, **all GOTO statements are analyzed** to ensure they do not leave or enter the scope of a loop or callable.  If they do, an "Unsafe GOTO" error is raised and the program will not execute.  For example:

```
:10 FOR I% := 1 TO 1000
:20   PRINT I%
:30   IF I% > 10 THEN GOTO 50
:40 NEXT I%
:50 PRINT "I got bored after 10 iterations!"
:
:RUN
Unsafe GOTO in line 30:
30   IF I% > 10 THEN >>> GOTO 50
:_
```

To break loops safely, see [below](#diversified-break).  "Unsafe GOTO" will also be raised if you try to **GOTO into a stored program** from a direct command, like this:

```
:10 PRINT "Hello world!"
:
:GOTO 10
Unsafe GOTO:
>>> GOTO 10
:_
```

### (Diversified) BREAK

The BREAK keyword in RM Basic was **only used in the context of ON BREAK [ OFF ]** statements to set BREAK interrupt traps; **this will be supported in RM BasicXT** but the BREAK keyword by itself can also be used to **safely exit FOR and REPEAT loops** prematurely.

To break a loop, the RM Basic manual recommends setting the control variable of the loop so the evaluation in the FOR or UNTIL statement will be FALSE, then making a GOTO to the end of the loop.  A side-effect of this was losing the value of the control variable when the loop was broken.  So naturally most people would just **GOTO directly out of the loop** which, as discussed earlier, **isn't great**.

By **re-using the BREAK keyword** to "pop" the loop on demand we can **safely break away from loops**, much like in modern languages, in a way that is far easier to read while **fitting perfectly to RM Basic's structured aesthetic**:

```
:10 FOR I% := 1 TO 1000
:20   IF I% > 5 THEN BREAK ELSE PRINT I%
:30 NEXT I%
:40 PRINT "I got bored after 5 iterations!"
:
:RUN
1
2
3
4
5
I got bored after 5 iterations!
:_
```             

### Speed

It isn't slow.

## REPL

This repo does not provide any emulation of the RM Nimbus sound, graphics or text features; it is purely for testing and developing the compiler.  A REPL has been provided that can be used to write and execute programs and obtain debugging output but otherwise has very limited functionality.  Decompilation results are displayed after compilation and before execution is started.

### Key Features

- A stripped-down PRINT command will print literals or the result of an expression:

```
:PRINT "Hello world!"
---------------------
decompilation results
...
---------------------
Hello world!
:PRINT 100/2
---------------------
decompilation results
...
---------------------
50
:_
```

- Programs can be executed immediately as single-line, direct commands:

```
:Hello_Message : PRINT "Goodbye!" : PROCEDURE Hello_Message : PRINT "Hello" : ENDPROC
---------------------
decompilation results
...
---------------------
Hello
Goodbye!
:_
```

- Multi-line programs can be entered and then executed with the RUN command.  

```
:10 Hello_Message
:20 PRINT "Goodbye!"
:30 PROCEDURE Hello_Message
:40 PRINT "Hello"
:50 ENDPROC
:
:RUN
---------------------
decompilation results
...
---------------------
Hello
Goodbye!
:_
```

- Callables stored in a program can be called from a direct command, just like in RM Basic.  Just keep in mind that the entire program will be compiled first:

```
:10 FUNCTION Adder%(A%, B%)
:20 RESULT A% + B%
:30 ENDFUN
:
:PRINT Adder%(10, 100)
---------------------
decompilation results
...
---------------------
110
:_
```

- Multi-line programs can be listed with a stripped-down LIST command.
- Errors are displayed along with the offending program line in the same way as RM Basic.
- And that's it.

Despair ye not, an application providing the complete RM Basic experience is in the works!

## Building

### Pre-requisities

- Linux or macOS (it should work on Windows but not tested!)
- [Rust](https://rust-lang.org/tools/install/)
- [Git](https://git-scm.com/install/)
- Electricity

### Clone this repo

```bash
git clone https://github.com/adamstimb/rmbasicxt-prototype
cd rmbasicxt-prototype
```

### Run tests

```bash
cargo test
```

### Start the REPL

```bash
cargo run
```
