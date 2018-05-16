use std::fmt;
use std::default::Default;
use std::io::{Write, Read};

const NUMBER_OF_CELLS: u16 = u16::max_value();

#[derive(Clone)]
pub struct State {
    pub pos: u16,
    pub cells: [u8; NUMBER_OF_CELLS as usize]
}

impl Default for State {
    fn default() -> Self {
        State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS as usize]
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum RuntimeError {
    WriteError(String),
    ReadError(String)
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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Node {
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

pub fn run_block<R: Read, W: Write>(stdin: &mut R, stdout: &mut W, block: &Vec<Node>, s: &mut State) -> Result<(), RuntimeError> {
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
        let code = vec!(
            Node::Conditional(vec!(
                Node::Right(2),
                Node::Assign(0),
                Node::Left(2),
                Node::Conditional(vec!(
                    Node::Dec(1),
                    Node::Right(2),
                    Node::Inc(1),
                    Node::Left(2)
                ))
            ))
        );

        run_block(&mut stdin.as_slice(), &mut stdout, &code, &mut s).unwrap();

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
        let code = vec!(
            Node::Conditional(vec!(
                Node::Right(2),
                Node::Assign(0),
                Node::Left(2),
                Node::Conditional(vec!(
                    Node::Dec(1),
                    Node::Right(2),
                    Node::Inc(1),
                    Node::Left(2)
                ))
            ))
        );

        run_block(&mut stdin.as_slice(), &mut stdout, &code, &mut s).unwrap();

        assert_eq!(s.pos, 0);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
    }
}
