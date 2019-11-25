extern crate rustyline;

pub mod analyzer;
pub mod optimizer;
pub mod parser;
pub mod vm;

use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};

use parser::ParserError;
use vm::{RuntimeError, State};

#[derive(Debug, PartialEq)]
pub enum ExecutionError {
    Parse(ParserError),
    Run(RuntimeError),
}

/// Run some brainfuck code
pub fn run_code<F: BufRead, R: Read, W: Write>(
    code: &mut F,
    stdin: &mut R,
    stdout: &mut W,
    s: &mut State,
) -> Result<(), ExecutionError> {
    let parsed = parser::parse_code(code).map_err(ExecutionError::Parse)?;
    let optimized = optimizer::optimize_code(&parsed, &optimizer::OptimizationOptions::default());

    // println!("Unoptimized: {:?}", (analyzer::SimpleAnalyzer {}).analyze(&parsed));
    // println!("Optimized: {:?}", (analyzer::SimpleAnalyzer {}).analyze(&optimized));
    // println!("Code: {:?}", optimized);

    vm::run_block(stdin, stdout, &optimized, s).map_err(ExecutionError::Run)
}

fn start_script(path: &str) -> Result<(), ExecutionError> {
    let mut state = State::default();
    let mut src_input = BufReader::new(File::open(path).map_err(|e| {
        ExecutionError::Parse(ParserError::Io(format!(
            "Could not open source file: {:?}",
            e
        )))
    })?);
    let stdin = io::stdin();
    let stdout = io::stdout();

    run_code(
        &mut src_input,
        &mut stdin.lock(),
        &mut stdout.lock(),
        &mut state,
    )
    .expect("Error interpreting");

    Ok(())
}

fn start_repl() {
    let mut rl = Editor::<()>::new();
    let mut state = State::default();
    let stdin = io::stdin();
    let stdout = io::stdout();

    loop {
        println!("{}", state);
        let readline = rl.readline("rf# ");

        match readline {
            Ok(line) => {
                rl.add_history_entry(&line);
                match run_code(
                    &mut line.as_bytes(),
                    &mut stdin.lock(),
                    &mut stdout.lock(),
                    &mut state,
                ) {
                    Ok(()) => {}
                    Err(e) => println!("{:?}", e),
                };
            }
            Err(ReadlineError::Interrupted) => {
                println!("Exiting");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("Exiting");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
}

fn main() {
    let first_arg = env::args().nth(1);

    if let Some(path) = first_arg {
        start_script(&path).map_err(|e| format!("{:?}", e)).unwrap();
    } else {
        start_repl();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_return_parser_errors_when_running_code() {
        let stdin = vec![];
        let mut stdout = vec![];
        let mut s = State::default();

        let code = "[[]";
        let result = run_code(
            &mut code.as_bytes(),
            &mut stdin.as_slice(),
            &mut stdout,
            &mut s,
        );

        assert_eq!(
            result,
            Err(ExecutionError::Parse(ParserError::MissingDelimiter))
        );
    }
}
