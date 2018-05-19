use vm::Node;

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
        code.into_iter().fold(vec!(), move |mut acc, c| {
            let last = acc.pop();

            match (last, c) {
                (Some(Node::Right(x)), Node::Right(y)) => {
                    if x as u16 + y as u16 > 255 {
                        acc.push(Node::Right(x));
                        acc.push(Node::Right(y));
                    } else {
                        acc.push(Node::Right(x + y));
                    }
                },
                (Some(Node::Left(x)), Node::Left(y)) => {
                    if x as u16 + y as u16 > 255 {
                        acc.push(Node::Left(x));
                        acc.push(Node::Left(y));
                    } else {
                        acc.push(Node::Left(x + y));
                    }
                },
                (Some(Node::Inc(x, offset1, false)), Node::Inc(y, offset2, false)) => {
                    if x as u16 + y as u16 > 255 || offset1 != offset2 {
                        acc.push(Node::Inc(x, offset1, false));
                        acc.push(Node::Inc(y, offset2, false));
                    } else {
                        acc.push(Node::Inc(x + y, offset1, false));
                    }
                },
                (Some(Node::Dec(x, offset1, false)), Node::Dec(y, offset2, false)) => {
                    if x as u16 + y as u16 > 255 || offset1 != offset2 {
                        acc.push(Node::Dec(x, offset1, false));
                        acc.push(Node::Dec(y, offset2, false));
                    } else {
                        acc.push(Node::Dec(x + y, offset1, false));
                    }
                },
                (l, Node::Conditional(body)) => {
                    match l {
                        Some(c) => acc.push(c),
                        None => {}
                    }

                    acc.push(Node::Conditional(self.apply(body)));
                },
                (l, c) => {
                    match l {
                        Some(c) => acc.push(c),
                        None => {}
                    }
                    acc.push(c);
                }
            };

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
                let mut handled = false;

                match (&last, &c) {
                    (Some(Node::Assign(0, offset1, false)), Node::Inc(inc_val, offset2, false)) => {
                        if offset1 == offset2 {
                            acc.push(Node::Assign(*inc_val, *offset1, false));
                            handled = true;
                        }
                    },
                    (Some(Node::Assign(0, offset1, false)), Node::Dec(dec_val, offset2, false)) => {
                        if offset1 == offset2 {
                            acc.push(Node::Assign(0u8.wrapping_sub(*dec_val), *offset1, false));
                            handled = true;
                        }
                    },
                    _ => {}
                };

                if !handled {
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

            match last {
                Some(Node::Right(offset)) => {
                    match new_node {
                        Node::Inc(v, 0, false) => acc.push(Node::Inc(v, offset as i32, true)),
                        Node::Dec(v, 0, false) => acc.push(Node::Dec(v, offset as i32, true)),
                        Node::Assign(v, 0, false) => acc.push(Node::Assign(v, offset as i32, true)),
                        n => {
                            acc.push(last.unwrap());
                            acc.push(n);
                        }
                    }
                },
                Some(Node::Left(offset)) => {
                    match new_node {
                        Node::Inc(v, 0, false) => acc.push(Node::Inc(v, -(offset as i32), true)),
                        Node::Dec(v, 0, false) => acc.push(Node::Dec(v, -(offset as i32), true)),
                        Node::Assign(v, 0, false) => acc.push(Node::Assign(v, -(offset as i32), true)),
                        n => {
                            acc.push(last.unwrap());
                            acc.push(n);
                        }
                    }
                },
                Some(old_node) => {
                    match new_node {
                        Node::Right(right) => {
                            match old_node {
                                Node::Inc(value, offset, true) | Node::Dec(value, offset, true) | Node::Assign(value, offset, true) => {
                                    if offset < 0 {
                                        let diff = offset.abs() - right as i32;
                                        let build_node = match old_node {
                                            Node::Inc(_, _, _) => Node::Inc,
                                            Node::Dec(_, _, _) => Node::Dec,
                                            Node::Assign(_, _, _) => Node::Assign,
                                            _ => unreachable!()
                                        };

                                        if diff > 0 {
                                            acc.push(Node::Left(diff as u8));
                                            acc.push(build_node(value, -(right as i32), false));
                                        } else if diff == 0 {
                                            acc.push(build_node(value, offset, false));
                                        } else {
                                            acc.push(build_node(value, offset, false));
                                            acc.push(Node::Right(diff.abs() as u8));
                                        }
                                    } else {
                                        acc.push(old_node);
                                        acc.push(new_node);
                                    }
                                },
                                _ => {
                                    acc.push(old_node);
                                    acc.push(new_node);
                                }
                            }
                        },
                        Node::Left(left) => {
                            match old_node {
                                Node::Inc(value, offset, true) | Node::Dec(value, offset, true) | Node::Assign(value, offset, true) => {
                                    if offset > 0 {
                                        let diff = offset - left as i32;
                                        let build_node = match old_node {
                                            Node::Inc(_, _, _) => Node::Inc,
                                            Node::Dec(_, _, _) => Node::Dec,
                                            Node::Assign(_, _, _) => Node::Assign,
                                            _ => unreachable!()
                                        };

                                        if diff < 0 {
                                            acc.push(build_node(value, offset, false));
                                            acc.push(Node::Left((-diff) as u8));
                                        } else if diff == 0 {
                                            acc.push(build_node(value, offset, false));
                                        } else {
                                            acc.push(Node::Right(diff as u8));
                                            acc.push(build_node(value, left as i32, false));
                                        }
                                    } else {
                                        acc.push(old_node);
                                        acc.push(new_node);
                                    }
                                },
                                _ => {
                                    acc.push(old_node);
                                    acc.push(new_node);
                                }
                            }
                        },
                        _ => {
                            acc.push(old_node);
                            acc.push(new_node);
                        }
                    }
                },
                None => {
                    acc.push(new_node);
                }
            }

            acc
        })
    }
}

pub fn optimize_code(code: &Vec<Node>) -> Vec<Node> {
    let without_comments: Vec<Node> = FilterComments.apply(code.clone());
    let joined_operators = MergeRepeatedOperators.apply(without_comments);
    let without_zero_loops = CollapseAssignments.apply(joined_operators);
    let with_offsets = CollapseOffsets.apply(without_zero_loops);

    with_offsets
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
        let result = optimize_code(&code);

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
        let result = optimize_code(&code);

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
        let result = optimize_code(&code);

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
        let result = optimize_code(&code);

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
        let result = optimize_code(&code);

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
            Node::Conditional(vec!(
                Node::Right(5),
                Node::Inc(1, 0, false),
                Node::Right(5),
                Node::Dec(1, 0, false),
                Node::Right(5),
                Node::Assign(1, 0, false),
            ))
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Inc(1, 5, true),
            Node::Dec(1, 5, true),
            Node::Assign(1, 5, true),
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
            Node::Conditional(vec!(
                Node::Left(5),
                Node::Inc(1, 0, false),
                Node::Left(5),
                Node::Dec(1, 0, false),
                Node::Left(5),
                Node::Assign(1, 0, false),
            ))
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Inc(1, -5, true),
            Node::Dec(1, -5, true),
            Node::Assign(1, -5, true),
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
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Inc(1, -5, false),
            Node::Dec(1, -5, false),
            Node::Assign(1, 5, false),
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
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Left(2),
            Node::Inc(1, -5, false),
            Node::Inc(1, -5, false),
            Node::Right(2)
        ));
    }
}
