use std::default::Default;
use vm::Node;

pub struct OptimizationOptions {
    collapsed_operators: bool,
    collapsed_assignments: bool,
    collapsed_offsets: bool,
    collapsed_loops: bool
}

impl Default for OptimizationOptions {
    fn default() -> Self {
        return OptimizationOptions {
            collapsed_operators: true,
            collapsed_assignments: true,
            collapsed_offsets: true,
            collapsed_loops: true
        };
    }
}

trait OptimizationStep {
    fn apply(&self, code: Vec<Node>) -> Vec<Node>;
}

struct FilterComments;

impl OptimizationStep for FilterComments {
    fn apply(&self, code: Vec<Node>) -> Vec<Node> {
        code
            .into_iter()
            .flat_map(move |n| match n {
                Node::Comment(_) => None,
                Node::Conditional(body) => Some(Node::Conditional(self.apply(body))),
                n => Some(n)
            })
            .collect()
    }
}

struct MergeRepeatedOperators;

impl OptimizationStep for MergeRepeatedOperators {
    fn apply(&self, code: Vec<Node>) -> Vec<Node> {
        code.into_iter().fold(vec!(), move |mut acc, node| {
            let last = acc.pop();

            let merged = match (&last, &node) {
                (Some(Node::Right(x)), Node::Right(y)) => {
                    if *x as u16 + *y as u16 > 255 {
                        None
                    } else {
                        Some(Node::Right(x + y))
                    }
                },
                (Some(Node::Left(x)), Node::Left(y)) => {
                    if *x as u16 + *y as u16 > 255 {
                        None
                    } else {
                        Some(Node::Left(x + y))
                    }
                },
                (Some(Node::Inc(x, offset1, false)), Node::Inc(y, offset2, false)) => {
                    if *x as u16 + *y as u16 > 255 || offset1 != offset2 {
                        None
                    } else {
                        Some(Node::Inc(x + y, *offset1, false))
                    }
                },
                (Some(Node::Dec(x, offset1, false)), Node::Dec(y, offset2, false)) => {
                    if *x as u16 + *y as u16 > 255 || offset1 != offset2 {
                        None
                    } else {
                        Some(Node::Dec(x + y, *offset1, false))
                    }
                },
                _ => None
            };

            if let Some(n) = merged {
                acc.push(n);
            } else {
                if let Some(l) = last {
                    acc.push(l);
                }
                match node {
                    Node::Conditional(body) => acc.push(Node::Conditional(self.apply(body))),
                    n => acc.push(n)
                };
            }

            acc
        })
    }
}

struct CollapseAssignments;

impl OptimizationStep for CollapseAssignments {
    fn apply(&self, code: Vec<Node>) -> Vec<Node> {
        code
            .into_iter()
            .map(move |n| match n {
                Node::Conditional(body) => {
                    if body == vec!(Node::Dec(1, 0, false)) {
                        Node::Assign(0, 0, false)
                    } else {
                        Node::Conditional(self.apply(body))
                    }
                },
                n => n
            })
            .fold(vec!(), move |mut acc, c| {
                let last = acc.pop();
                let value = match (&last, &c) {
                    (Some(Node::Assign(0, offset1, false)), Node::Inc(inc_val, offset2, false)) => {
                        if offset1 == offset2 {
                            Some(Node::Assign(*inc_val, *offset1, false))
                        } else {
                            None
                        }
                    },
                    (Some(Node::Assign(0, offset1, false)), Node::Dec(dec_val, offset2, false)) => {
                        if offset1 == offset2 {
                            Some(Node::Assign(0u8.wrapping_sub(*dec_val), *offset1, false))
                        } else {
                            None
                        }
                    },
                    _ => None
                };

                if let Some(v) = value {
                    acc.push(v);
                } else {
                    if let Some(last) = last {
                        acc.push(last);
                    }
                    acc.push(c);
                }

                acc
            })
    }
}

struct CollapseOffsets;

impl OptimizationStep for CollapseOffsets {
    fn apply(&self, code: Vec<Node>) -> Vec<Node> {
        code.into_iter().fold(vec!(), move |mut acc, node| {
            let last = acc.pop();
            let new_node = match node {
                Node::Conditional(body) => Node::Conditional(self.apply(body)),
                n => n
            };
            let modified = match &last {
                Some(Node::Right(offset)) => {
                    match new_node {
                        Node::Inc(v, 0, false) => Some(vec!(Node::Inc(v, *offset as i32, true))),
                        Node::Dec(v, 0, false) => Some(vec!(Node::Dec(v, *offset as i32, true))),
                        Node::Assign(v, 0, false) => Some(vec!(Node::Assign(v, *offset as i32, true))),
                        Node::Out(0, false) => Some(vec!(Node::Out(*offset as i32, true))),
                        Node::In(0, false) => Some(vec!(Node::In(*offset as i32, true))),
                        _ => None
                    }
                },
                Some(Node::Left(offset)) => {
                    match new_node {
                        Node::Inc(v, 0, false) => Some(vec!(Node::Inc(v, -(*offset as i32), true))),
                        Node::Dec(v, 0, false) => Some(vec!(Node::Dec(v, -(*offset as i32), true))),
                        Node::Assign(v, 0, false) => Some(vec!(Node::Assign(v, -(*offset as i32), true))),
                        Node::Out(0, false) => Some(vec!(Node::Out(-(*offset as i32), true))),
                        Node::In(0, false) => Some(vec!(Node::In(-(*offset as i32), true))),
                        _ => None
                    }
                },
                Some(old_node) => {
                    match new_node {
                        Node::Right(right) => {
                            match old_node {
                                Node::Inc(value, offset, true) |
                                Node::Dec(value, offset, true) |
                                Node::Assign(value, offset, true) => {
                                    if *offset < 0 {
                                        let diff = offset.abs() - right as i32;
                                        let build_node = match old_node {
                                            Node::Inc(_, _, _) => Node::Inc,
                                            Node::Dec(_, _, _) => Node::Dec,
                                            Node::Assign(_, _, _) => Node::Assign,
                                            _ => unreachable!()
                                        };

                                        if diff > 0 {
                                            Some(vec!(
                                                Node::Left(diff as u8),
                                                build_node(*value, -(right as i32), false)
                                            ))
                                        } else if diff == 0 {
                                            Some(vec!(build_node(*value, *offset, false)))
                                        } else {
                                            Some(vec!(
                                                build_node(*value, *offset, false),
                                                Node::Right(diff.abs() as u8)
                                            ))
                                        }
                                    } else {
                                        None
                                    }
                                },
                                Node::In(offset, true) |
                                Node::Out(offset, true) => {
                                    if *offset < 0 {
                                        let diff = offset.abs() - right as i32;
                                        let build_node = match old_node {
                                            Node::In(_, _) => Node::In,
                                            Node::Out(_, _) => Node::Out,
                                            _ => unreachable!()
                                        };

                                        if diff > 0 {
                                            Some(vec!(
                                                Node::Left(diff as u8),
                                                build_node(-(right as i32), false)
                                            ))
                                        } else if diff == 0 {
                                            Some(vec!(build_node(*offset, false)))
                                        } else {
                                            Some(vec!(
                                                build_node(*offset, false),
                                                Node::Right(diff.abs() as u8)
                                            ))
                                        }
                                    } else {
                                        None
                                    }
                                },
                                _ => None
                            }
                        },
                        Node::Left(left) => {
                            match old_node {
                                Node::Inc(value, offset, true) |
                                Node::Dec(value, offset, true) |
                                Node::Assign(value, offset, true) => {
                                    if *offset > 0 {
                                        let diff = offset - left as i32;
                                        let build_node = match old_node {
                                            Node::Inc(_, _, _) => Node::Inc,
                                            Node::Dec(_, _, _) => Node::Dec,
                                            Node::Assign(_, _, _) => Node::Assign,
                                            _ => unreachable!()
                                        };

                                        if diff < 0 {
                                            Some(vec!(
                                                build_node(*value, *offset, false),
                                                Node::Left((-diff) as u8)
                                            ))
                                        } else if diff == 0 {
                                            Some(vec!(build_node(*value, *offset, false)))
                                        } else {
                                            Some(vec!(
                                                Node::Right(diff as u8),
                                                build_node(*value, left as i32, false)
                                            ))
                                        }
                                    } else {
                                        None
                                    }
                                },
                                Node::In(offset, true) |
                                Node::Out(offset, true) => {
                                    if *offset > 0 {
                                        let diff = offset - left as i32;
                                        let build_node = match old_node {
                                            Node::In(_, _) => Node::In,
                                            Node::Out(_, _) => Node::Out,
                                            _ => unreachable!()
                                        };

                                        if diff < 0 {
                                            Some(vec!(
                                                build_node(*offset, false),
                                                Node::Left((-diff) as u8)
                                            ))
                                        } else if diff == 0 {
                                            Some(vec!(build_node(*offset, false)))
                                        } else {
                                            Some(vec!(
                                                Node::Right(diff as u8),
                                                build_node(left as i32, false)
                                            ))
                                        }
                                    } else {
                                        None
                                    }
                                },
                                _ => None
                            }
                        },
                        _ => None
                    }
                },
                None => None
            };

            if let Some(v) = modified {
                for n in v {
                    acc.push(n);
                }
            } else {
                if let Some(n) = last {
                    acc.push(n);
                }
                acc.push(new_node);
            }

            acc
        })
    }
}

struct DeferMovements;

impl OptimizationStep for DeferMovements {
    fn apply(&self, code: Vec<Node>) -> Vec<Node> {
        let (mut memo, rest) = code
            .into_iter()
            .fold((vec!(), vec!()), move |memo, new_node| {
                let (mut memo, mut current_block) = memo;

                match new_node {
                    Node::Left(_) |
                    Node::Right(_) |
                    Node::Inc(_, _, _) |
                    Node::Dec(_, _, _) |
                    Node::Assign(_, _, _) |
                    Node::In(_, _) |
                    Node::Out(_, _) |
                    Node::Comment(_) => {
                        current_block.push(new_node);
                    },
                    Node::Conditional(body) => {
                        memo.push(current_block);
                        memo.push(vec!(Node::Conditional(self.apply(body))));
                        current_block = vec!();
                    }
                }
                (memo, current_block)
            });

        memo.push(rest);

        memo.into_iter().fold(vec!(), move |mut memo, group| {
            if group.len() == 1 {
                memo.push(group.first().unwrap().clone());
            } else {
                let mut current_offset: i32 = 0;

                for node in group {
                    match node {
                        Node::Left(v) => current_offset -= v as i32,
                        Node::Right(v) => current_offset += v as i32,
                        Node::Dec(v, offset, move_pointer) |
                        Node::Inc(v, offset, move_pointer) |
                        Node::Assign(v, offset, move_pointer) => {
                            let new_node = match node {
                                Node::Dec(_, _, _) => Node::Dec,
                                Node::Inc(_, _, _) => Node::Inc,
                                Node::Assign(_, _, _) => Node::Assign,
                                _ => unreachable!()
                            };

                            memo.push(new_node(v, current_offset + offset, false));
                            if move_pointer {
                                current_offset += offset;
                            }
                        },
                        Node::In(offset, move_pointer) |
                        Node::Out(offset, move_pointer) => {
                            let new_node = match node {
                                Node::In(_, _) => Node::In,
                                Node::Out(_, _) => Node::Out,
                                _ => unreachable!()
                            };

                            memo.push(new_node(current_offset + offset, false));
                            if move_pointer {
                                current_offset += offset;
                            }
                        },
                        Node::Comment(_) => {},
                        Node::Conditional(_) => {}
                    }
                }

                if current_offset > 0 {
                    while current_offset > 255 {
                        memo.push(Node::Right(255));
                        current_offset -= 255;
                    }
                    memo.push(Node::Right(current_offset as u8));
                } else if current_offset < 0 {
                    while current_offset < -255 {
                        memo.push(Node::Left(255));
                        current_offset += 255;
                    }
                    memo.push(Node::Left((-current_offset) as u8));
                }
            }

            memo
        })
    }
}

pub fn optimize_code(code: &Vec<Node>, options: &OptimizationOptions) -> Vec<Node> {
    let mut optimizations: Vec<Box<OptimizationStep>> = vec!();

    optimizations.push(Box::new(FilterComments));
    if options.collapsed_operators {
        optimizations.push(Box::new(MergeRepeatedOperators));
    }
    if options.collapsed_assignments {
        optimizations.push(Box::new(CollapseAssignments));
    }
    if options.collapsed_offsets {
        optimizations.push(Box::new(CollapseOffsets));
    }
    if options.collapsed_loops {
        optimizations.push(Box::new(DeferMovements));

    }

    let mut c = code.clone();
    for o in optimizations {
        c = o.apply(c);
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let result = optimize_code(&code, &OptimizationOptions::default());

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
                Node::Inc(1, 1, false),
                Node::Comment('a'),
                Node::Inc(1, 1, false),
                Node::Conditional(vec!(
                    Node::Comment('a'),
                    Node::Right(1),
                    Node::Dec(1, 1, false),
                    Node::Right(1),
                    Node::Dec(1, 1, false),
                    Node::Dec(1, 1, false),
                ))
            ))
        );
        let result = optimize_code(&code, &OptimizationOptions {
            collapsed_operators: true,
            collapsed_loops: false,
            collapsed_assignments: false,
            collapsed_offsets: false
        });

        assert_eq!(result, vec!(
            Node::Right(3),
            Node::Left(2),
            Node::Right(1),
            Node::Conditional(vec!(
                Node::Inc(2, 1, false),
                Node::Conditional(vec!(
                    Node::Right(1),
                    Node::Dec(1, 1, false),
                    Node::Right(1),
                    Node::Dec(2, 1, false)
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
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(result, vec!(
            Node::Right(255),
            Node::Right(1)
        ));
    }

    #[test]
    fn it_should_not_optimize_operators_with_different_offsets() {
        let code = vec!(
            Node::Inc(1, 0, false),
            Node::Inc(1, 1, false),
            Node::Dec(1, 0, false),
            Node::Dec(1, 1, false),
            Node::Assign(1, 0, false),
            Node::Assign(1, 1, false),
        );
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(result, vec!(
            Node::Inc(1, 0, false),
            Node::Inc(1, 1, false),
            Node::Dec(1, 0, false),
            Node::Dec(1, 1, false),
            Node::Assign(1, 0, false),
            Node::Assign(1, 1, false),
        ));
    }

    #[test]
    fn it_should_optimize_zero_loops() {
        let code = vec!(
            Node::Conditional(vec!(Node::Dec(1, 0, false))),
            Node::Conditional(vec!(
                Node::Conditional(vec!(Node::Dec(1, 0, false)))
            ))
        );
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(result, vec!(
            Node::Assign(0, 0, false),
            Node::Conditional(vec!(
                Node::Assign(0, 0, false),
            ))
        ));
    }

    #[test]
    fn it_should_optimize_assignment_loops() {
        let code = vec!(
            Node::Conditional(vec!(Node::Dec(1, 0, false))),
            Node::Inc(100, 0, false),
            Node::Conditional(vec!(Node::Dec(1, 0, false))),
            Node::Dec(1, 0, false),
            Node::Conditional(vec!(
                Node::Conditional(vec!(Node::Dec(1, 0, false))),
                Node::Inc(100, 0, false),
            ))
        );
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(result, vec!(
            Node::Assign(100, 0, false),
            Node::Assign(255, 0, false),
            Node::Conditional(vec!(
                Node::Assign(100, 0, false),
            ))
        ));
    }

    #[test]
    fn it_should_collapse_to_positive_offsets() {
        let code = vec!(
            Node::Right(5),
            Node::Inc(1, 0, false),
            Node::Right(5),
            Node::Dec(1, 0, false),
            Node::Right(5),
            Node::Assign(1, 0, false),
            Node::Right(5),
            Node::In(0, false),
            Node::Right(5),
            Node::Out(0, false),
            Node::Conditional(vec!(
                Node::Right(5),
                Node::Inc(1, 0, false),
                Node::Right(5),
                Node::Dec(1, 0, false),
                Node::Right(5),
                Node::Assign(1, 0, false),
            ))
        );
        let result = optimize_code(&code, &OptimizationOptions {
            collapsed_operators: false,
            collapsed_loops: false,
            collapsed_assignments: false,
            collapsed_offsets: true
        });

        assert_eq!(result, vec!(
            Node::Inc(1, 5, true),
            Node::Dec(1, 5, true),
            Node::Assign(1, 5, true),
            Node::In(5, true),
            Node::Out(5, true),
            Node::Conditional(vec!(
                Node::Inc(1, 5, true),
                Node::Dec(1, 5, true),
                Node::Assign(1, 5, true),
            ))
        ));
    }

    #[test]
    fn it_should_collapse_to_negative_offsets() {
        let code = vec!(
            Node::Left(5),
            Node::Inc(1, 0, false),
            Node::Left(5),
            Node::Dec(1, 0, false),
            Node::Left(5),
            Node::Assign(1, 0, false),
            Node::Left(5),
            Node::In(0, false),
            Node::Left(5),
            Node::Out(0, false),
            Node::Conditional(vec!(
                Node::Left(5),
                Node::Inc(1, 0, false),
                Node::Left(5),
                Node::Dec(1, 0, false),
                Node::Left(5),
                Node::Assign(1, 0, false),
            ))
        );
        let result = optimize_code(&code, &OptimizationOptions {
            collapsed_operators: false,
            collapsed_loops: false,
            collapsed_assignments: false,
            collapsed_offsets: true
        });

        assert_eq!(result, vec!(
            Node::Inc(1, -5, true),
            Node::Dec(1, -5, true),
            Node::Assign(1, -5, true),
            Node::In(-5, true),
            Node::Out(-5, true),
            Node::Conditional(vec!(
                Node::Inc(1, -5, true),
                Node::Dec(1, -5, true),
                Node::Assign(1, -5, true),
            ))
        ));
    }

    #[test]
    fn it_should_collapse_non_moving_nodes() {
        let code = vec!(
            Node::Left(5),
            Node::Inc(1, 0, false),
            Node::Right(5),
            Node::Left(5),
            Node::Dec(1, 0, false),
            Node::Right(5),
            Node::Right(5),
            Node::Assign(1, 0, false),
            Node::Left(5),
            Node::Right(5),
            Node::In(0, false),
            Node::Left(5),
            Node::Right(5),
            Node::Out(0, false),
            Node::Left(5),
            Node::Conditional(vec!(
                Node::Left(5),
                Node::Inc(1, 0, false),
                Node::Right(5),
                Node::Left(5),
                Node::Dec(1, 0, false),
                Node::Right(5),
                Node::Right(5),
                Node::Assign(1, 0, false),
                Node::Left(5),
            ))
        );
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(result, vec!(
            Node::Inc(1, -5, false),
            Node::Dec(1, -5, false),
            Node::Assign(1, 5, false),
            Node::In(5, false),
            Node::Out(5, false),
            Node::Conditional(vec!(
                Node::Inc(1, -5, false),
                Node::Dec(1, -5, false),
                Node::Assign(1, 5, false),
            ))
        ));
    }

    #[test]
    fn it_should_collapse_imbalanced_non_moving_nodes() {
        let code = vec!(
            Node::Left(7),
            Node::Inc(1, 0, false),
            Node::Right(5),
            Node::Left(5),
            Node::Inc(1, 0, false),
            Node::Right(7)
        );
        let result = optimize_code(&code, &OptimizationOptions {
            collapsed_operators: false,
            collapsed_loops: false,
            collapsed_assignments: false,
            collapsed_offsets: true
        });

        assert_eq!(result, vec!(
            Node::Left(2),
            Node::Inc(1, -5, false),
            Node::Inc(1, -5, false),
            Node::Right(2)
        ));
    }

    #[test]
    fn it_should_defer_movement() {
        let code = vec!(
            Node::Left(1),
            Node::Right(6),

            Node::Inc(1, 5, true),
            Node::Inc(1, 5, false),

            Node::Conditional(vec!(
                Node::Dec(1, 5, true),
                Node::Out(-5, true)
            )),

            Node::Left(10),
            Node::Inc(1, 5, true),
        );
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(result, vec!(
            Node::Inc(1, 10, false),
            Node::Inc(1, 15, false),
            Node::Right(10),

            Node::Conditional(vec!(
                Node::Dec(1, 5, false),
                Node::Out(0, false)
            )),

            Node::Inc(1, -5, false),
            Node::Left(5),
        ));
    }
}
