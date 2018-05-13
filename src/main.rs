use std::env;
use std::fs::File;
use std::io::{self, Write, Read, BufRead, BufReader};

const NUMBER_OF_CELLS: usize = 32768;

#[derive(Clone)]
struct State {
    pos: usize,
    cells: [u8; NUMBER_OF_CELLS]
}

#[derive(Clone)]
enum Node {
    Instruction(char),
    Conditional(Vec<Node>)
}

fn run_block<R: Read, W: Write>(stdin: &mut R, stdout: &mut W, block: &Vec<Node>, s: &mut State) -> Result<(), String> {
    for node in block {
        node.execute(stdin, stdout, s)?;
    }
    Ok(())
}


impl Node {
    fn execute<R: Read, W: Write>(&self, stdin: &mut R, stdout: &mut W, s: &mut State) -> Result<(), String> {
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
                stdout.write(&[ s.cells[s.pos] ]).expect("Error writing to stdout");
                Ok(())
            },
            Node::Instruction(',') => {
                let v = stdin.bytes().next().expect("Error reading stdin");
                s.cells[s.pos] = v.expect("Error reading stdin");
                Ok(())
            },
            _ => Ok(())
        }
    }
}

fn parse_code<F: BufRead>(code: &mut F) -> Vec<Node> {
    let parsed = vec!();
    let mut stack = vec!(parsed);

    for c in code.bytes() {
        let next_char = c.expect("Error reading source file") as char;

        match next_char {
            '[' => {
                stack.push(vec!())
            },
            ']' => {
                let body = stack.pop().unwrap();
                stack.last_mut().expect("Unmatched closing delimiter").push(Node::Conditional(body))
            },
            c => stack.last_mut().expect("Unmatched closing delimiter").push(Node::Instruction(c))
        }
    }
    (*stack.last().unwrap()).clone()
}

fn run_code<F: BufRead, R: Read, W: Write>(code: &mut F, stdin: &mut R, stdout: &mut W, s: &mut State) -> Result<(), String> {
    let parsed = parse_code(code);
    return run_block(stdin, stdout, &parsed, s);
}

fn main() {
    let mut state = State { pos: 0, cells: [0; NUMBER_OF_CELLS] };
    let src_path = env::args().skip(1).next().expect("Please provide a source file");
    let mut src_input = BufReader::new(File::open(src_path).expect("Error opening source file"));
    let stdin = io::stdin();
    let stdout = io::stdout();

    run_code(&mut src_input, &mut stdin.lock(), &mut stdout.lock(), &mut state).expect("Error interpreting");
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
}
