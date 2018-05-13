extern crate rustyline;

use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, Write, Read, BufRead, BufReader};
use rustyline::error::ReadlineError;
use rustyline::Editor;

const NUMBER_OF_CELLS: usize = 131072;

#[derive(Clone)]
struct State {
    pos: usize,
    cells: [u8; NUMBER_OF_CELLS]
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let cell_count = 25;
        let cells_to_show: Vec<usize> = (0..25).into_iter().map(|i| {
            let offset = cell_count / 2;
            let pos: i64 = self.pos as i64 + i - offset;

            if pos < 0 {
                (NUMBER_OF_CELLS as i64 + pos) as usize
            } else if pos >= NUMBER_OF_CELLS as i64 {
                (pos - NUMBER_OF_CELLS as i64) as usize
            } else {
                pos as usize
            }
        }).collect();

        f.write_str("Brainfuck state:\n")?;
        f.write_str("|")?;
        for cell in &cells_to_show {
            f.write_str(&format!("{:6}", cell))?;
            f.write_str("|")?;
        }
        f.write_str("\n|")?;
        for cell in &cells_to_show {
            f.write_str(&format!("{:6}", self.cells[*cell]))?;
            f.write_str("|")?;
        }
        f.write_str("\n|")?;
        for cell in &cells_to_show {
            if *cell == self.pos {
                f.write_str("******|")?;

            } else {
                f.write_str("      |")?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Instruction(char),
    Conditional(Vec<Node>)
}

#[derive(Debug, PartialEq)]
enum ParserError {
    UnmatchedDelimiter,
    MissingDelimiter,
    Io(String),
    Internal
}

#[derive(Debug, PartialEq)]
enum RuntimeError {
    WriteError(String),
    ReadError(String)
}

#[derive(Debug, PartialEq)]
enum ExecutionError {
    Parse(ParserError),
    Run(RuntimeError)
}

fn run_block<R: Read, W: Write>(stdin: &mut R, stdout: &mut W, block: &Vec<Node>, s: &mut State) -> Result<(), RuntimeError> {
    for node in block {
        node.execute(stdin, stdout, s)?;
    }
    Ok(())
}


impl Node {
    fn execute<R: Read, W: Write>(&self, stdin: &mut R, stdout: &mut W, s: &mut State) -> Result<(), RuntimeError> {
        match self {
            Node::Conditional(body) => {
                while s.cells[s.pos] != 0 {
                    run_block(stdin, stdout, body, s)?;
                }
                Ok(())
            },
            Node::Instruction('>') => {
                s.pos = if s.pos + 1 == NUMBER_OF_CELLS { 0 } else { s.pos + 1 };
                Ok(())
            },
            Node::Instruction('<') => {
                s.pos = if s.pos == 0 { NUMBER_OF_CELLS - 1 } else { s.pos - 1 };
                Ok(())
            },
            Node::Instruction('+') => {
                let v = s.cells[s.pos];
                s.cells[s.pos] = if v == 255 { 0 } else { v + 1 };
                Ok(())
            },
            Node::Instruction('-') => {
                let v = s.cells[s.pos];
                s.cells[s.pos] = if v == 0 { 255 } else { v - 1 };
                Ok(())
            },
            Node::Instruction('.') => {
                stdout.write(&[ s.cells[s.pos] ]).map_err(|e| RuntimeError::WriteError(format!("{:?}", e)))?;
                Ok(())
            },
            Node::Instruction(',') => {
                let v = stdin.bytes().next().ok_or(RuntimeError::ReadError("No data from stdin".to_string()))?;
                s.cells[s.pos] = v.map_err(|e| RuntimeError::ReadError(format!("{:?}", e)))?;
                Ok(())
            },
            _ => Ok(())
        }
    }
}

fn parse_code<F: BufRead>(code: &mut F) -> Result<Vec<Node>, ParserError> {
    let parsed = vec!();
    let mut nested = vec!(parsed);

    for c in code.bytes() {
        let next_char = c.map_err(|e| ParserError::Io(format!("{}", e)))? as char;

        match next_char {
            '[' => {
                nested.push(vec!())
            },
            ']' => {
                if nested.len() < 2 {
                    return Err(ParserError::UnmatchedDelimiter);
                }

                let body = nested.pop().ok_or(ParserError::Internal)?;
                nested.last_mut().ok_or(ParserError::Internal)?.push(Node::Conditional(body))
            },
            c => nested.last_mut().ok_or(ParserError::Internal)?.push(Node::Instruction(c))
        }
    }

    if nested.len() > 1 {
        return Err(ParserError::MissingDelimiter);
    }
    if nested.len() != 1 {
        return Err(ParserError::Internal);
    }

    let res = nested.last().ok_or(ParserError::Internal)?;

    Ok(res.clone())
}

fn run_code<F: BufRead, R: Read, W: Write>(code: &mut F, stdin: &mut R, stdout: &mut W, s: &mut State) -> Result<(), ExecutionError> {
    let parsed = parse_code(code).map_err(ExecutionError::Parse)?;
    return run_block(stdin, stdout, &parsed, s).map_err(ExecutionError::Run);
}

fn start_script(path: &str) -> Result<(), ExecutionError> {
    let mut state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
    let mut src_input = BufReader::new(File::open(path)
        .map_err(|e| ExecutionError::Parse(ParserError::Io(format!("Could not open source file: {:?}", e))))?);
    let stdin = io::stdin();
    let stdout = io::stdout();

    run_code(&mut src_input, &mut stdin.lock(), &mut stdout.lock(), &mut state).expect("Error interpreting");

    Ok(())
}

fn start_repl() {
    let mut rl = Editor::<()>::new();
    let mut state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
    let stdin = io::stdin();
    let stdout = io::stdout();

    loop {
        println!("{}", state);
        let readline = rl.readline("rf# ");

        match readline {
            Ok(line) => {
                rl.add_history_entry(&line);
                match run_code(&mut line.as_bytes(), &mut stdin.lock(), &mut stdout.lock(), &mut state) {
                    Ok(()) => {},
                    Err(e) => println!("{:?}", e)
                };
            },
            Err(ReadlineError::Interrupted) => {
                println!("Exiting");
                break
            },
            Err(ReadlineError::Eof) => {
                println!("Exiting");
                break
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break
            }
        }
    }
}

fn main() {
    let first_arg = env::args().skip(1).next();

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
    fn it_should_increment_the_data_pointer() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        Node::Instruction('>').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, 1);
    }

    #[test]
    fn it_should_overflow_the_data_pointer() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: NUMBER_OF_CELLS - 1, cells: [0; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        Node::Instruction('>').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, 0);
    }

    #[test]
    fn it_should_decrement_the_data_pointer() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 1, cells: [0; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        Node::Instruction('<').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, 0);
    }

    #[test]
    fn it_should_underflow_the_data_pointer() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        Node::Instruction('<').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, NUMBER_OF_CELLS - 1);
    }

    #[test]
    fn it_should_increment_cells() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        Node::Instruction('+').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 1);
    }

    #[test]
    fn it_should_overflow_cells() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        s.cells[0] = 255;
        Node::Instruction('+').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 0);
    }

    #[test]
    fn it_should_decrement_cells() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [1; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        Node::Instruction('-').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 0);
    }

    #[test]
    fn it_should_underflow_cells() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        Node::Instruction('-').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 255);
    }

    #[test]
    fn it_should_read_from_stdin() {
        let stdin = vec!( 'b' as u8 );
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: ['a' as u8; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        Node::Instruction(',').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 'b' as u8);
    }

    #[test]
    fn it_should_write_to_stdout() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: ['a' as u8; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        Node::Instruction('.').execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(stdout.len(), 1);
        assert_eq!(stdout.get(0), Some(&('a' as u8)));
    }

    #[test]
    fn it_should_run_nested_code_if_condition_is_true() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        s.cells[0] = 255;

        // This code piece moves the value of the current cell (cell0) two cells to the right (cell2)
        let code = "[>>[-]<<[->>+<<]]";

        run_code(&mut code.as_bytes(), &mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.pos, 0);
        assert_eq!(s.cells[0], initial_state.cells[0]);
        assert_eq!(s.cells[1], initial_state.cells[1]);
        assert_eq!(s.cells[2], 255);
        assert_eq!(s.cells[3..], initial_state.cells[3..]);
    }

    #[test]
    fn it_should_not_run_nested_code_if_condition_is_false() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
        let mut s = initial_state.clone();

        // This code piece moves the value of the current cell (cell0) two cells to the right (cell2)
        let code = "[>>[-]<<[->>+<<]]";

        run_code(&mut code.as_bytes(), &mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.pos, 0);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
    }

    #[test]
    fn it_should_return_parser_errors_when_running_code() {
        let stdin = vec!();
        let mut stdout = vec!();
        let mut s = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };

        let code = "[[]";
        let result = run_code(&mut code.as_bytes(), &mut stdin.as_slice(), &mut stdout, &mut s);

        assert_eq!(result, Err(ExecutionError::Parse(ParserError::MissingDelimiter)));
    }

    #[test]
    fn it_should_parse_instructions() {
        let code = "<>+-.,";
        let result = parse_code(&mut code.as_bytes());

        assert_eq!(result, Ok(vec!(
            Node::Instruction('<'),
            Node::Instruction('>'),
            Node::Instruction('+'),
            Node::Instruction('-'),
            Node::Instruction('.'),
            Node::Instruction(',')
        )));
    }

    #[test]
    fn it_should_parse_an_empty_conditional() {
        let code = "[]";
        let result = parse_code(&mut code.as_bytes());

        assert_eq!(result, Ok(vec!(
            Node::Conditional(vec!())
        )));
    }

    #[test]
    fn it_should_parse_an_conditional_with_instructions() {
        let code = "[<>]";
        let result = parse_code(&mut code.as_bytes());

        assert_eq!(result, Ok(vec!(
            Node::Conditional(vec!(
                Node::Instruction('<'),
                Node::Instruction('>')
            ))
        )));
    }

    #[test]
    fn it_should_parse_nested_conditionals() {
        let code = "[<[>]]";
        let result = parse_code(&mut code.as_bytes());

        assert_eq!(result, Ok(vec!(
            Node::Conditional(vec!(
                Node::Instruction('<'),
                Node::Conditional(vec!(
                    Node::Instruction('>')
                ))
            ))
        )));
    }

    #[test]
    fn it_should_return_a_unmatched_delimiter_error() {
        let code = "[]]";
        let result = parse_code(&mut code.as_bytes());

        assert_eq!(result, Err(ParserError::UnmatchedDelimiter));
    }

    #[test]
    fn it_should_return_a_missing_delimiter_error() {
        let code = "[[]";
        let result = parse_code(&mut code.as_bytes());

        assert_eq!(result, Err(ParserError::MissingDelimiter));
    }
}
