extern crate rustyline;

use std::env;
use std::fmt;
use std::fs::File;
use std::io::{self, Write, Read, BufRead, BufReader};
use rustyline::error::ReadlineError;
use rustyline::Editor;

const NUMBER_OF_CELLS: u16 = u16::max_value();

#[derive(Clone)]
struct State {
    pos: u16,
    cells: [u8; NUMBER_OF_CELLS as usize]
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let cell_count = 25;
        let cells_to_show: Vec<u16> = (0..25).into_iter().map(|i| {
            let offset = cell_count / 2;
            let pos: i64 = self.pos as i64 + i - offset;

            if pos < 0 {
                (NUMBER_OF_CELLS as i64 + pos) as u16
            } else if pos >= NUMBER_OF_CELLS as i64 {
                (pos - NUMBER_OF_CELLS as i64) as u16
            } else {
                pos as u16
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
            f.write_str(&format!("{:6}", self.cells[*cell as usize]))?;
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
    Right(u8),
    Left(u8),
    Inc(u8),
    Dec(u8),
    Assign(u8),
    Out,
    In,
    Conditional(Vec<Node>),
    Comment(char),
}

impl From<char> for Node {
    fn from(c: char) -> Node {
        match c {
            '>' => Node::Right(1),
            '<' => Node::Left(1),
            '+' => Node::Inc(1),
            '-' => Node::Dec(1),
            '.' => Node::Out,
            ',' => Node::In,
            '[' => unreachable!(),
            ']' => unreachable!(),
            c => Node::Comment(c)
        }
    }
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
                while s.cells[s.pos as usize] != 0 {
                    run_block(stdin, stdout, body, s)?;
                }
                Ok(())
            },
            Node::Right(i) => {
                s.pos = s.pos.wrapping_add(*i as u16);
                Ok(())
            },
            Node::Left(i) => {
                s.pos = s.pos.wrapping_sub(*i as u16);
                Ok(())
            },
            Node::Inc(i) => {
                let v = s.cells[s.pos as usize];
                s.cells[s.pos as usize] = v.wrapping_add(*i);
                Ok(())
            },
            Node::Dec(i) => {
                let v = s.cells[s.pos as usize];
                s.cells[s.pos as usize] = v.wrapping_sub(*i);
                Ok(())
            },
            Node::Assign(i) => {
                s.cells[s.pos as usize] = *i;
                Ok(())
            },
            Node::Out => {
                stdout.write(&[ s.cells[s.pos as usize] ]).map_err(|e| RuntimeError::WriteError(format!("{:?}", e)))?;
                Ok(())
            },
            Node::In => {
                let v = stdin.bytes().next().ok_or(RuntimeError::ReadError("No data from stdin".to_string()))?;
                s.cells[s.pos as usize] = v.map_err(|e| RuntimeError::ReadError(format!("{:?}", e)))?;
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
            c => nested.last_mut().ok_or(ParserError::Internal)?.push(Node::from(c))
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

fn filter_comments(n: &Node) -> Option<Node> {
    match n {
        Node::Comment(_) => None,
        Node::Conditional(body) => {
            let v: Vec<Node> = body
                .into_iter()
                .flat_map(filter_comments)
                .collect();
            Some(Node::Conditional(v))
        },
        _ => Some(n.clone())
    }
}

fn join_repeated_operators(code_without_comments: &Vec<Node>) -> Vec<Node> {
    code_without_comments.into_iter().fold(vec!(), |acc, c| {
        let mut acc_new: Vec<Node> = acc.clone();
        let last = acc_new.pop();

        match (&last, c) {
            (Some(Node::Right(x)), Node::Right(y)) => {
                if *x as u16 + *y as u16 > 255 {
                    acc_new.push(Node::Right(*x));
                    acc_new.push(Node::Right(*y));
                } else {
                    acc_new.push(Node::Right(x + y));
                }
            },
            (Some(Node::Left(x)), Node::Left(y)) => {
                if *x as u16 + *y as u16 > 255 {
                    acc_new.push(Node::Left(*x));
                    acc_new.push(Node::Left(*y));
                } else {
                    acc_new.push(Node::Left(x + y));
                }
            },
            (Some(Node::Inc(x)), Node::Inc(y)) => {
                if *x as u16 + *y as u16 > 255 {
                    acc_new.push(Node::Inc(*x));
                    acc_new.push(Node::Inc(*y));
                } else {
                    acc_new.push(Node::Inc(x + y));
                }
            },
            (Some(Node::Dec(x)), Node::Dec(y)) => {
                if *x as u16 + *y as u16 > 255 {
                    acc_new.push(Node::Dec(*x));
                    acc_new.push(Node::Dec(*y));
                } else {
                    acc_new.push(Node::Dec(x + y));
                }
            },
            (l, Node::Conditional(body)) => {
                match l {
                    Some(c) => acc_new.push(c.clone()),
                    None => {}
                }

                acc_new.push(Node::Conditional(join_repeated_operators(body)));
            },
            (l, c) => {
                match l {
                    Some(c) => acc_new.push(c.clone()),
                    None => {}
                }
                acc_new.push(c.clone());
            }
        };

        acc_new
    })
}

fn replace_zero_loops(code_without_comments: &Vec<Node>) -> Vec<Node> {
    return code_without_comments
        .into_iter()
        .map(|n| match n {
            Node::Conditional(body) => {
                if *body == vec!(Node::Dec(1)) {
                    Node::Assign(0)
                } else {
                    Node::Conditional(body.clone())
                }
            },
            n => n.clone()
        })
        .collect()
}

fn optimize_code(code: &Vec<Node>) -> Vec<Node> {
    let without_comments: Vec<Node> = code
        .into_iter()
        .flat_map(filter_comments)
        .collect();
    let joined_operators = join_repeated_operators(&without_comments);
    let without_zero_loops = replace_zero_loops(&joined_operators);

    without_zero_loops
}

fn run_code<F: BufRead, R: Read, W: Write>(code: &mut F, stdin: &mut R, stdout: &mut W, s: &mut State) -> Result<(), ExecutionError> {
    let parsed = parse_code(code).map_err(ExecutionError::Parse)?;
    let optimized = optimize_code(&parsed);
    return run_block(stdin, stdout, &optimized, s).map_err(ExecutionError::Run);
}

fn start_script(path: &str) -> Result<(), ExecutionError> {
    let mut state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
    let mut src_input = BufReader::new(File::open(path)
        .map_err(|e| ExecutionError::Parse(ParserError::Io(format!("Could not open source file: {:?}", e))))?);
    let stdin = io::stdin();
    let stdout = io::stdout();

    run_code(&mut src_input, &mut stdin.lock(), &mut stdout.lock(), &mut state).expect("Error interpreting");

    Ok(())
}

fn start_repl() {
    let mut rl = Editor::<()>::new();
    let mut state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
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
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::Right(1).execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, 1);
    }

    #[test]
    fn it_should_overflow_the_data_pointer() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: NUMBER_OF_CELLS - 1, cells: [0; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::Right(3).execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, 1);
    }

    #[test]
    fn it_should_decrement_the_data_pointer() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 1, cells: [0; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::Left(1).execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, 0);
    }

    #[test]
    fn it_should_underflow_the_data_pointer() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::Left(3).execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, NUMBER_OF_CELLS - 2);
    }

    #[test]
    fn it_should_increment_cells() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::Inc(1).execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 1);
    }

    #[test]
    fn it_should_overflow_cells() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        s.cells[0] = 255;
        Node::Inc(5).execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 4);
    }

    #[test]
    fn it_should_decrement_cells() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [1; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::Dec(1).execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 0);
    }

    #[test]
    fn it_should_underflow_cells() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::Dec(5).execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 251);
    }

    #[test]
    fn it_should_assign_cells() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::Assign(5).execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 5);
    }

    #[test]
    fn it_should_read_from_stdin() {
        let stdin = vec!( 'b' as u8 );
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: ['a' as u8; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::In.execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 'b' as u8);
    }

    #[test]
    fn it_should_write_to_stdout() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: ['a' as u8; NUMBER_OF_CELLS as usize] };
        let mut s = initial_state.clone();

        Node::Out.execute(&mut stdin.as_slice(), &mut stdout, &mut s).unwrap();

        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(stdout.len(), 1);
        assert_eq!(stdout.get(0), Some(&('a' as u8)));
    }

    #[test]
    fn it_should_run_nested_code_if_condition_is_true() {
        let stdin = vec!();
        let mut stdout = vec!();
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
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
        let initial_state = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };
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
        let mut s = State { pos: 0, cells: [0; NUMBER_OF_CELLS as usize] };

        let code = "[[]";
        let result = run_code(&mut code.as_bytes(), &mut stdin.as_slice(), &mut stdout, &mut s);

        assert_eq!(result, Err(ExecutionError::Parse(ParserError::MissingDelimiter)));
    }

    #[test]
    fn it_should_parse_instructions() {
        let code = "<>+-.,";
        let result = parse_code(&mut code.as_bytes());

        assert_eq!(result, Ok(vec!(
            Node::Left(1),
            Node::Right(1),
            Node::Inc(1),
            Node::Dec(1),
            Node::Out,
            Node::In
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
                Node::Left(1),
                Node::Right(1)
            ))
        )));
    }

    #[test]
    fn it_should_parse_nested_conditionals() {
        let code = "[<[>]]";
        let result = parse_code(&mut code.as_bytes());

        assert_eq!(result, Ok(vec!(
            Node::Conditional(vec!(
                Node::Left(1),
                Node::Conditional(vec!(
                    Node::Right(1)
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

    #[test]
    fn it_should_optimize_away_comments() {
        let code = vec!(
            Node::Comment('a'),
            Node::Right(1),
            Node::Comment('b'),
            Node::Conditional(vec!(
                Node::Comment('a'),
                Node::Right(1),
                Node::Conditional(vec!(
                    Node::Comment('a'),
                    Node::Right(1),
                ))
            ))
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Right(1),
            Node::Conditional(vec!(
                Node::Right(1),
                Node::Conditional(vec!(
                    Node::Right(1),
                ))
            ))
        ));
    }

    #[test]
    fn it_should_optimize_away_repeated_operators() {
        let code = vec!(
            Node::Right(1),
            Node::Comment('a'),
            Node::Right(1),
            Node::Right(1),
            Node::Left(1),
            Node::Left(1),
            Node::Right(1),
            Node::Conditional(vec!(
                Node::Inc(1),
                Node::Comment('a'),
                Node::Inc(1),
                Node::Conditional(vec!(
                    Node::Comment('a'),
                    Node::Right(1),
                    Node::Dec(1),
                    Node::Right(1),
                    Node::Dec(1),
                    Node::Dec(1),
                ))
            ))
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Right(3),
            Node::Left(2),
            Node::Right(1),
            Node::Conditional(vec!(
                Node::Inc(2),
                Node::Conditional(vec!(
                    Node::Right(1),
                    Node::Dec(1),
                    Node::Right(1),
                    Node::Dec(2)
                ))
            ))
        ));
    }

    #[test]
    fn it_should_not_optimize_operators_that_would_overflow() {
        let code = vec!(
            Node::Right(254),
            Node::Right(1),
            Node::Right(1),
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Right(255),
            Node::Right(1)
        ));
    }

    #[test]
    fn it_should_optimize_zero_loops() {
        let code = vec!(
            Node::Conditional(vec!(Node::Dec(1))),
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Assign(0)
        ));
    }
}
