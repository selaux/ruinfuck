use std::default::Default;
use std::fmt;
use std::io::{Read, Write};

const NUMBER_OF_CELLS: usize = u16::max_value() as usize;

#[derive(Clone)]
pub struct State {
    pub pos: usize,
    pub cells: [u8; NUMBER_OF_CELLS as usize],
}

impl Default for State {
    fn default() -> Self {
        State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS as usize],
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum RuntimeError {
    WriteError(String),
    ReadError(String),
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let cell_count = 25;
        let cells_to_show: Vec<usize> = (0..25)
            .map(|i| {
                let offset = cell_count / 2;
                let pos: i64 = self.pos as i64 + i - offset;

                if pos < 0 {
                    (NUMBER_OF_CELLS as i64 + pos) as usize
                } else if pos >= NUMBER_OF_CELLS as i64 {
                    (pos - NUMBER_OF_CELLS as i64) as usize
                } else {
                    pos as usize
                }
            })
            .collect();

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

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Node {
    Shift(isize),
    // value, (offset_to), offset, move_pointer
    Inc(u8, isize, bool),
    Dec(u8, isize, bool),
    Mul(i16, isize, isize, bool),
    Assign(u8, isize, bool),
    Scan(isize),
    Out(isize, bool),
    In(isize, bool),
    Conditional(Vec<Node>),
    Comment(char),
}

pub fn run_block<R: Read, W: Write>(
    stdin: &mut R,
    stdout: &mut W,
    block: &[Node],
    s: &mut State,
) -> Result<(), RuntimeError> {
    for node in block {
        node.execute(stdin, stdout, s)?;
    }
    Ok(())
}

fn offset_index(pos: usize, offset: isize) -> usize {
    (pos as u16).wrapping_add(offset as u16) as usize
}

impl Node {
    fn execute<R: Read, W: Write>(
        &self,
        stdin: &mut R,
        stdout: &mut W,
        s: &mut State,
    ) -> Result<(), RuntimeError> {
        match *self {
            Node::Conditional(ref body) => {
                while s.cells[s.pos] != 0 {
                    run_block(stdin, stdout, body, s)?;
                }
                Ok(())
            }
            Node::Shift(i) => {
                s.pos = offset_index(s.pos, i);
                Ok(())
            }
            Node::Inc(i, offset, move_pointer) => {
                let pos = offset_index(s.pos, offset);
                let v = &mut s.cells[pos];
                *v = v.wrapping_add(i);
                if move_pointer {
                    s.pos = pos;
                }
                Ok(())
            }
            Node::Dec(i, offset, move_pointer) => {
                let pos = offset_index(s.pos, offset);
                let v = &mut s.cells[pos];
                *v = v.wrapping_sub(i);
                if move_pointer {
                    s.pos = pos;
                }
                Ok(())
            }
            Node::Mul(mul_value, into, offset, move_pointer) => {
                let pos = offset_index(s.pos, offset);
                let into_pos = offset_index(pos, into);
                let v = s.cells[pos];
                let into = &mut s.cells[into_pos];
                let abs = mul_value.abs() as u8;

                if mul_value >= 0 {
                    *into = into.wrapping_add(v.wrapping_mul(abs));
                } else {
                    *into = into.wrapping_sub(v.wrapping_mul(abs));
                }
                if move_pointer {
                    s.pos = pos;
                }
                Ok(())
            }
            Node::Assign(i, offset, move_pointer) => {
                let pos = offset_index(s.pos, offset);
                s.cells[pos] = i;
                if move_pointer {
                    s.pos = pos;
                }
                Ok(())
            }
            Node::Scan(interval) => {
                let mut pos = s.pos;
                while s.cells[pos] != 0 {
                    pos = offset_index(pos, interval);
                }
                s.pos = pos;
                Ok(())
            }
            Node::Out(offset, move_pointer) => {
                let pos = offset_index(s.pos, offset);
                stdout
                    .write(&[s.cells[pos]])
                    .map_err(|e| RuntimeError::WriteError(format!("{:?}", e)))?;

                if move_pointer {
                    s.pos = pos;
                }

                Ok(())
            }
            Node::In(offset, move_pointer) => {
                let pos = offset_index(s.pos, offset);
                let v = stdin
                    .bytes()
                    .next()
                    .ok_or_else(|| RuntimeError::ReadError("No data from stdin".to_string()))?;
                s.cells[pos] = v.map_err(|e| RuntimeError::ReadError(format!("{:?}", e)))?;

                if move_pointer {
                    s.pos = pos;
                }

                Ok(())
            }
            Node::Comment(_) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_increment_the_data_pointer() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Shift(1)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, 1);
    }

    #[test]
    fn it_should_overflow_the_data_pointer() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: NUMBER_OF_CELLS - 1,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Shift(3)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, 2);
    }

    #[test]
    fn it_should_decrement_the_data_pointer() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 1,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Shift(-1)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, 0);
    }

    #[test]
    fn it_should_underflow_the_data_pointer() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Shift(-3)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(s.pos, NUMBER_OF_CELLS - 3);
    }

    #[test]
    fn it_should_increment_cells() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Inc(1, 0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 1);
    }

    #[test]
    fn it_should_increment_cells_at_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Inc(1, 1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[2..], initial_state.cells[2..]);
        assert_eq!(s.cells[0], 0);
        assert_eq!(s.cells[1], 1);
    }

    #[test]
    fn it_should_increment_cells_at_offset_and_move_pointer() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Inc(1, 1, true)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 1);
        assert_eq!(s.cells[2..], initial_state.cells[2..]);
        assert_eq!(s.cells[0], 0);
        assert_eq!(s.cells[1], 1);
    }

    #[test]
    fn it_should_increment_cells_at_overflowing_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: NUMBER_OF_CELLS - 1,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Inc(1, 1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 1);
    }

    #[test]
    fn it_should_increment_cells_at_underflowing_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Inc(1, -1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(
            s.cells[0..(NUMBER_OF_CELLS - 2)],
            initial_state.cells[0..(NUMBER_OF_CELLS - 2)]
        );
        assert_eq!(s.cells[(NUMBER_OF_CELLS - 1)], 1);
    }

    #[test]
    fn it_should_multiply_cells() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 1,
            cells: [2; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Mul(2, -1, 0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();
        Node::Mul(3, 1, 0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[4..], initial_state.cells[4..]);
        assert_eq!(s.cells[0], 6);
        assert_eq!(s.cells[1], 2);
        assert_eq!(s.cells[2], 8);
    }

    #[test]
    fn it_should_multiply_cells_at_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [2; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Mul(2, -1, 1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();
        Node::Mul(3, 1, 1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[4..], initial_state.cells[4..]);
        assert_eq!(s.cells[0], 6);
        assert_eq!(s.cells[1], 2);
        assert_eq!(s.cells[2], 8);
    }

    #[test]
    fn it_should_multiply_cells_at_offset_and_move_pointer() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 1,
            cells: [2; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Mul(2, -1, 0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();
        Node::Mul(3, 0, 1, true)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 2);
        assert_eq!(s.cells[4..], initial_state.cells[4..]);
        assert_eq!(s.cells[0], 6);
        assert_eq!(s.cells[1], 2);
        assert_eq!(s.cells[2], 8);
    }

    #[test]
    fn it_should_overflow_cells() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        s.cells[0] = 255;
        Node::Inc(5, 0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 4);
    }

    #[test]
    fn it_should_decrement_cells() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [1; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Dec(1, 0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 0);
    }

    #[test]
    fn it_should_decrement_cells_at_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [1; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Dec(1, 1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[2..], initial_state.cells[2..]);
        assert_eq!(s.cells[0], 1);
        assert_eq!(s.cells[1], 0);
    }

    #[test]
    fn it_should_decrement_cells_at_offset_and_move_pointer() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Dec(1, 1, true)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 1);
        assert_eq!(s.cells[2..], initial_state.cells[2..]);
        assert_eq!(s.cells[0], 0);
        assert_eq!(s.cells[1], 255);
    }

    #[test]
    fn it_should_decrement_cells_at_overflowing_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: NUMBER_OF_CELLS - 1,
            cells: [1; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Dec(1, 1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 0);
    }

    #[test]
    fn it_should_decrement_cells_at_underflowing_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [1; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Dec(1, -1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(
            s.cells[0..(NUMBER_OF_CELLS - 2)],
            initial_state.cells[0..(NUMBER_OF_CELLS - 2)]
        );
        assert_eq!(s.cells[(NUMBER_OF_CELLS - 1)], 0);
    }

    #[test]
    fn it_should_underflow_cells() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Dec(5, 0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 251);
    }

    #[test]
    fn it_should_assign_cells() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Assign(5, 0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 5);
    }

    #[test]
    fn it_should_assign_cells_at_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Assign(5, 1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[2..], initial_state.cells[2..]);
        assert_eq!(s.cells[0], 0);
        assert_eq!(s.cells[1], 5);
    }

    #[test]
    fn it_should_assign_cells_at_offset_and_move_pointer() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Assign(5, 1, true)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 1);
        assert_eq!(s.cells[2..], initial_state.cells[2..]);
        assert_eq!(s.cells[0], 0);
        assert_eq!(s.cells[1], 5);
    }

    #[test]
    fn it_should_assign_cells_at_overflowing_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: NUMBER_OF_CELLS - 1,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Assign(5, 1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], 5);
    }

    #[test]
    fn it_should_assign_cells_at_underflowing_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [1; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Assign(5, -1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(
            s.cells[0..(NUMBER_OF_CELLS - 2)],
            initial_state.cells[0..(NUMBER_OF_CELLS - 2)]
        );
        assert_eq!(s.cells[(NUMBER_OF_CELLS - 1)], 5);
    }

    #[test]
    fn it_should_read_from_stdin() {
        let stdin = vec![b'b'];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [b'a'; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::In(0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[1..], initial_state.cells[1..]);
        assert_eq!(s.cells[0], b'b');
    }

    #[test]
    fn it_should_read_from_stdin_with_offset() {
        let stdin = vec![b'b'];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [b'a'; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::In(1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[2..], initial_state.cells[2..]);
        assert_eq!(s.cells[0], b'a');
        assert_eq!(s.cells[1], b'b');
    }

    #[test]
    fn it_should_read_from_stdin_with_offset_and_move_pointer() {
        let stdin = vec![b'b'];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [b'a'; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::In(1, true)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 1);
        assert_eq!(s.cells[2..], initial_state.cells[2..]);
        assert_eq!(s.cells[0], b'a');
        assert_eq!(s.cells[1], b'b');
    }

    #[test]
    fn it_should_write_to_stdout() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [b'a'; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        Node::Out(0, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(stdout.len(), 1);
        assert_eq!(stdout.get(0), Some(&(b'a')));
    }

    #[test]
    fn it_should_write_to_stdout_with_offset() {
        let stdin = vec![];
        let mut stdout = vec![];
        let mut initial_state = State {
            pos: 0,
            cells: [b'a'; NUMBER_OF_CELLS],
        };
        initial_state.cells[1] = b'b';

        let mut s = initial_state.clone();

        Node::Out(1, false)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, initial_state.pos);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(stdout.len(), 1);
        assert_eq!(stdout.get(0), Some(&(b'b')));
    }

    #[test]
    fn it_should_write_to_stdout_with_offset_and_move_pointer() {
        let stdin = vec![];
        let mut stdout = vec![];
        let mut initial_state = State {
            pos: 0,
            cells: [b'a'; NUMBER_OF_CELLS],
        };
        initial_state.cells[1] = b'b';

        let mut s = initial_state.clone();

        Node::Out(1, true)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 1);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
        assert_eq!(stdout.len(), 1);
        assert_eq!(stdout.get(0), Some(&(b'b')));
    }

    #[test]
    fn it_should_scan_left() {
        let stdin = vec![];
        let mut stdout = vec![];
        let mut initial_state = State {
            pos: 21,
            cells: [1 as u8; NUMBER_OF_CELLS],
        };
        initial_state.cells[10] = 0;

        let mut s = initial_state.clone();

        Node::Scan(-1)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 10);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
    }

    #[test]
    fn it_should_scan_left_with_interval() {
        let stdin = vec![];
        let mut stdout = vec![];
        let mut initial_state = State {
            pos: 10,
            cells: [1 as u8; NUMBER_OF_CELLS],
        };
        initial_state.cells[9] = 0;
        initial_state.cells[8] = 0;

        let mut s = initial_state.clone();

        Node::Scan(-2)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 8);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
    }

    #[test]
    fn it_should_scan_right() {
        let stdin = vec![];
        let mut stdout = vec![];
        let mut initial_state = State {
            pos: 0,
            cells: [1 as u8; NUMBER_OF_CELLS],
        };
        initial_state.cells[9] = 0;

        let mut s = initial_state.clone();

        Node::Scan(1)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 9);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
    }

    #[test]
    fn it_should_scan_right_with_interval() {
        let stdin = vec![];
        let mut stdout = vec![];
        let mut initial_state = State {
            pos: 0,
            cells: [1 as u8; NUMBER_OF_CELLS],
        };
        initial_state.cells[1] = 0;
        initial_state.cells[2] = 0;

        let mut s = initial_state.clone();

        Node::Scan(2)
            .execute(&mut stdin.as_slice(), &mut stdout, &mut s)
            .unwrap();

        assert_eq!(s.pos, 2);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
    }

    #[test]
    fn it_should_run_nested_code_if_condition_is_true() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        s.cells[0] = 255;

        // This code piece moves the value of the current cell (cell0) two cells to the right (cell2)
        let code = vec![Node::Conditional(vec![
            Node::Shift(2),
            Node::Assign(0, 0, false),
            Node::Shift(-2),
            Node::Conditional(vec![
                Node::Dec(1, 0, false),
                Node::Shift(2),
                Node::Inc(1, 0, false),
                Node::Shift(-2),
            ]),
        ])];

        run_block(&mut stdin.as_slice(), &mut stdout, &code, &mut s).unwrap();

        assert_eq!(s.pos, 0);
        assert_eq!(s.cells[0], initial_state.cells[0]);
        assert_eq!(s.cells[1], initial_state.cells[1]);
        assert_eq!(s.cells[2], 255);
        assert_eq!(s.cells[3..], initial_state.cells[3..]);
    }

    #[test]
    fn it_should_not_run_nested_code_if_condition_is_false() {
        let stdin = vec![];
        let mut stdout = vec![];
        let initial_state = State {
            pos: 0,
            cells: [0; NUMBER_OF_CELLS],
        };
        let mut s = initial_state.clone();

        // This code piece moves the value of the current cell (cell0) two cells to the right (cell2)
        let code = vec![Node::Conditional(vec![
            Node::Shift(2),
            Node::Assign(0, 0, false),
            Node::Shift(-2),
            Node::Conditional(vec![
                Node::Dec(1, 0, false),
                Node::Shift(2),
                Node::Inc(1, 0, false),
                Node::Shift(2),
            ]),
        ])];

        run_block(&mut stdin.as_slice(), &mut stdout, &code, &mut s).unwrap();

        assert_eq!(s.pos, 0);
        assert_eq!(s.cells[0..], initial_state.cells[0..]);
    }
}
