use std::default::Default;
use vm::Node;

/// Which optimizations to enable.
pub struct OptimizationOptions {
    collapsed_operators: bool,
    collapsed_assignments: bool,
    collapsed_offsets: bool,
    collapsed_loops: bool,
    collapsed_scan_loops: bool,
}

impl Default for OptimizationOptions {
    fn default() -> Self {
        OptimizationOptions {
            collapsed_operators: true,
            collapsed_assignments: true,
            collapsed_offsets: true,
            collapsed_loops: true,
            collapsed_scan_loops: true,
        }
    }
}

/// The trait implemented by every optimization step
pub trait OptimizationStep {
    fn apply(&self, code: &[Node]) -> Vec<Node>;
}

/// The "Filter Comments" Optimization
///
/// Removes characters that are not matching any brainfuck operators
pub struct FilterComments;

impl OptimizationStep for FilterComments {
    fn apply(&self, code: &[Node]) -> Vec<Node> {
        code.into_iter()
            .flat_map(move |n| match n {
                Node::Comment(_) => None,
                Node::Conditional(body) => Some(Node::Conditional(self.apply(body))),
                n => Some(n.clone()),
            }).collect()
    }
}

/// The "Merge Repeated Operators" Optimization
///
/// Merges repeated operators into a single instruction
///
/// For example `++++` becomes `Inc(4)`
pub struct MergeRepeatedOperators;

impl OptimizationStep for MergeRepeatedOperators {
    fn apply(&self, code: &[Node]) -> Vec<Node> {
        code.into_iter().fold(vec![], move |mut acc, node| {
            let last = acc.pop();

            let merged = match (&last, &node) {
                (Some(Node::Shift(x)), Node::Shift(y)) => {
                    let diff = *x as i64 + *y as i64;
                    if diff >= i32::min_value() as i64 && diff <= i32::max_value() as i64 {
                        Some(Node::Shift(x + y))
                    } else {
                        None
                    }
                }
                (Some(Node::Inc(x, offset1, false)), Node::Inc(y, offset2, false)) => {
                    if *x as u16 + *y as u16 > 255 || offset1 != offset2 {
                        None
                    } else {
                        Some(Node::Inc(x + y, *offset1, false))
                    }
                }
                (Some(Node::Dec(x, offset1, false)), Node::Dec(y, offset2, false)) => {
                    if *x as u16 + *y as u16 > 255 || offset1 != offset2 {
                        None
                    } else {
                        Some(Node::Dec(x + y, *offset1, false))
                    }
                }
                _ => None,
            };

            if let Some(n) = merged {
                acc.push(n);
            } else {
                if let Some(l) = last {
                    acc.push(l);
                }
                match node {
                    Node::Conditional(body) => acc.push(Node::Conditional(self.apply(body))),
                    n => acc.push(n.clone()),
                };
            }

            acc
        })
    }
}

/// The "Collapse Assignments" Optimization
///
/// Collapses `[-]` into `Assign(0)` instructions. This particular loop only decrements until the current cell is 0.
/// It then subsequently collapses `Assign(0), Inc(x)` instructions into `Assign(x)` instructions.
///
/// For example `[-]+++` becomes `Assign(3)`.
pub struct CollapseAssignments;

impl OptimizationStep for CollapseAssignments {
    fn apply(&self, code: &[Node]) -> Vec<Node> {
        code.into_iter()
            .map(move |n| match n {
                Node::Conditional(body) => {
                    if body == &[Node::Dec(1, 0, false)] {
                        Node::Assign(0, 0, false)
                    } else {
                        Node::Conditional(self.apply(body))
                    }
                }
                n => n.clone(),
            }).fold(vec![], move |mut acc, c| {
                let last = acc.pop();
                let value = match (&last, &c) {
                    (Some(Node::Assign(0, offset1, false)), Node::Inc(inc_val, offset2, false)) => {
                        if offset1 == offset2 {
                            Some(Node::Assign(*inc_val, *offset1, false))
                        } else {
                            None
                        }
                    }
                    (Some(Node::Assign(0, offset1, false)), Node::Dec(dec_val, offset2, false)) => {
                        if offset1 == offset2 {
                            Some(Node::Assign(0u8.wrapping_sub(*dec_val), *offset1, false))
                        } else {
                            None
                        }
                    }
                    _ => None,
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

/// The "Collapse Offsets" Optimization
///
/// Adds movement information to each instruction. Joins the operation and the adjacent movement into
/// a single instruction.
///
/// For example `>>+++<<` becomes `Inc(3, 2, false)`. Where `3` is the value to increment, `2` the offset
/// of the data pointer where to increment and `false` the flag to determine whether to move the pointer
/// to the position at the offset
pub struct CollapseOffsets;

impl OptimizationStep for CollapseOffsets {
    fn apply(&self, code: &[Node]) -> Vec<Node> {
        code.into_iter().fold(vec![], move |mut acc, node| {
            let last = acc.pop();
            let new_node = match node {
                Node::Conditional(body) => Node::Conditional(self.apply(body)),
                n => n.clone(),
            };
            let modified = match &last {
                Some(Node::Shift(offset)) => match new_node {
                    Node::Inc(v, 0, false) => Some(vec![Node::Inc(v, *offset as i32, true)]),
                    Node::Dec(v, 0, false) => Some(vec![Node::Dec(v, *offset as i32, true)]),
                    Node::Assign(v, 0, false) => Some(vec![Node::Assign(v, *offset as i32, true)]),
                    Node::Out(0, false) => Some(vec![Node::Out(*offset as i32, true)]),
                    Node::In(0, false) => Some(vec![Node::In(*offset as i32, true)]),
                    _ => None,
                },
                Some(old_node) => match new_node {
                    Node::Shift(shift_offset) => match old_node {
                        Node::Inc(value, offset, true)
                        | Node::Dec(value, offset, true)
                        | Node::Assign(value, offset, true) => {
                            if offset.signum() != shift_offset.signum() {
                                let diff = offset.abs() - shift_offset.abs();
                                let build_node = match old_node {
                                    Node::Inc(_, _, _) => Node::Inc,
                                    Node::Dec(_, _, _) => Node::Dec,
                                    Node::Assign(_, _, _) => Node::Assign,
                                    _ => unreachable!(),
                                };
                                let weighted_diff = offset.signum() * diff;
                                let shift = Node::Shift(weighted_diff);

                                if diff == 0 {
                                    Some(vec![build_node(*value, *offset, false)])
                                } else if diff > 0 {
                                    Some(vec![
                                        shift,
                                        build_node(
                                            *value,
                                            *offset - offset.signum() * diff,
                                            false,
                                        ),
                                    ])
                                } else {
                                    Some(vec![build_node(*value, *offset, false), shift])
                                }
                            } else {
                                None
                            }
                        }
                        Node::In(offset, true) | Node::Out(offset, true) => {
                            if offset.signum() != shift_offset.signum() {
                                let diff = offset.abs() - shift_offset.abs();
                                let build_node = match old_node {
                                    Node::In(_, _) => Node::In,
                                    Node::Out(_, _) => Node::Out,
                                    _ => unreachable!(),
                                };

                                let weighted_diff = offset.signum() * diff;
                                let shift = Node::Shift(weighted_diff);

                                if diff == 0 {
                                    Some(vec![build_node(*offset, false)])
                                } else if diff > 0 {
                                    Some(vec![
                                        shift,
                                        build_node(*offset - offset.signum() * diff, false),
                                    ])
                                } else {
                                    Some(vec![build_node(*offset, false), shift])
                                }
                            } else {
                                None
                            }
                        }
                        _ => None,
                    },
                    _ => None,
                },
                None => None,
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

/// The "Defer Movements" Optimization
///
/// Defers movement of the data pointer within a the body or loops until the end of the loop. This just
/// saves a little bit of data pointer movement and will result in a more predictably structured instruction
/// list.
///
/// For example `Inc(3, 2, true), Inc(3, 2, false)` becomes `Inc(3, 2, false), Inc(3, 2, false), MoveDataPointer(2)`.
pub struct DeferMovements;

impl OptimizationStep for DeferMovements {
    fn apply(&self, code: &[Node]) -> Vec<Node> {
        let (mut memo, rest) = code
            .into_iter()
            .fold((vec![], vec![]), move |memo, new_node| {
                let (mut memo, mut current_block) = memo;

                match new_node {
                    Node::Shift(_)
                    | Node::Inc(_, _, _)
                    | Node::Dec(_, _, _)
                    | Node::Mul(_, _, _, _)
                    | Node::Assign(_, _, _)
                    | Node::In(_, _)
                    | Node::Out(_, _)
                    | Node::Comment(_) => {
                        current_block.push(new_node.clone());
                    }
                    Node::Scan(i) => {
                        memo.push(current_block);
                        memo.push(vec![Node::Scan(*i)]);
                        current_block = vec![];
                    }
                    Node::Conditional(body) => {
                        memo.push(current_block);
                        memo.push(vec![Node::Conditional(self.apply(body))]);
                        current_block = vec![];
                    }
                }
                (memo, current_block)
            });

        memo.push(rest);

        memo.into_iter().fold(vec![], move |mut memo, group| {
            if group.len() == 1 {
                memo.push(group.first().unwrap().clone());
            } else {
                let mut current_offset: i32 = 0;

                for node in group {
                    match node {
                        Node::Shift(v) => {
                            let sum = current_offset as i64 + v as i64;

                            if sum >= i32::min_value() as i64 && sum <= i32::max_value() as i64 {
                                current_offset = sum as i32;
                            } else {
                                memo.push(Node::Shift(current_offset));
                                current_offset = v;
                            }
                        }
                        Node::Dec(v, offset, move_pointer)
                        | Node::Inc(v, offset, move_pointer)
                        | Node::Assign(v, offset, move_pointer) => {
                            let new_node = match node {
                                Node::Dec(_, _, _) => Node::Dec,
                                Node::Inc(_, _, _) => Node::Inc,
                                Node::Assign(_, _, _) => Node::Assign,
                                _ => unreachable!(),
                            };

                            memo.push(new_node(v, current_offset + offset, false));
                            if move_pointer {
                                current_offset += offset;
                            }
                        }
                        Node::Mul(value, into_offset, offset, move_pointer) => {
                            memo.push(Node::Mul(
                                value,
                                into_offset,
                                current_offset + offset,
                                false,
                            ));
                            if move_pointer {
                                current_offset += offset;
                            }
                        }
                        Node::In(offset, move_pointer) | Node::Out(offset, move_pointer) => {
                            let new_node = match node {
                                Node::In(_, _) => Node::In,
                                Node::Out(_, _) => Node::Out,
                                _ => unreachable!(),
                            };

                            memo.push(new_node(current_offset + offset, false));
                            if move_pointer {
                                current_offset += offset;
                            }
                        }
                        Node::Comment(_) => {}
                        Node::Conditional(_) => {}
                        Node::Scan(_) => {}
                    }
                }

                if current_offset != 0 {
                    memo.push(Node::Shift(current_offset));
                }
            }

            memo
        })
    }
}

/// The "Collapse Simple Loops" Optimization
///
/// Introduces the multiplication instruction which is based on what is called a multiplication loop.
/// When a loop fulfills the following conditions:
///
/// - only contains incrementation and decrementation of the data pointer
/// - does not actually move the data pointer within its body
/// - and substracts 1 from the data pointer at the begining or end
///
/// Then it is actually multiplying the current cell into one ore more other cells.
///
/// A brainfuck example: `[>>+++<<-]` becomes `Mul(3, 2, false), Assign(0, 0)`
pub struct CollapseSimpleLoops;

impl CollapseSimpleLoops {
    fn is_collapsible_loop(body: &Vec<Node>) -> bool {
        let has_only_allowed_elements = body.into_iter().fold(true, |memo, node| match node {
            Node::Inc(_, _, false) => memo,
            Node::Dec(_, _, false) => memo,
            _ => false,
        });
        let contains_iterator = body
            .into_iter()
            .any(|x| x == &Node::Dec(1, 0, false));
        !body.is_empty() && has_only_allowed_elements && contains_iterator
    }
}

impl OptimizationStep for CollapseSimpleLoops {
    fn apply(&self, code: &[Node]) -> Vec<Node> {
        code.into_iter()
            .map(|node| match node {
                Node::Conditional(body) => {
                    if Self::is_collapsible_loop(&body) {
                        let mut moves: Vec<Node> = body
                            .into_iter()
                            .flat_map(|node| match node {
                                Node::Dec(1, 0, false) => None,
                                Node::Inc(value, offset, false) => {
                                    Some(Node::Mul(*value as i16, *offset, 0, false))
                                }
                                Node::Dec(value, offset, false) => {
                                    Some(Node::Mul(-(*value as i16), *offset, 0, false))
                                }
                                _ => None,
                            }).collect();

                        moves.push(Node::Assign(0, 0, false));

                        moves
                    } else {
                        vec![Node::Conditional(self.apply(&body))]
                    }
                }
                n => vec![n.clone()],
            }).fold(vec![], |mut memo, new| {
                for n in new {
                    memo.push(n);
                }
                memo
            })
    }
}

/// The "Collapse Scan Loops" Optimization
///
/// Introduces the scan instruction which searches for the next zero to the left or right of the data pointer.
/// When loops only contain movements of the data pointer they are actually looking for the next 0 to the left
/// or the right of the data pointer.
///
/// A brainfuck example: `[>>]` becomes `Scan(2)`
pub struct CollapseScanLoops;

impl OptimizationStep for CollapseScanLoops {
    fn apply(&self, code: &[Node]) -> Vec<Node> {
        code.into_iter()
            .map(|n| match n {
                Node::Conditional(body) => match body.as_slice() {
                    [Node::Shift(i)] => Node::Scan(*i),
                    body => Node::Conditional(self.apply(body)),
                },
                c => c.clone(),
            }).collect()
    }
}

pub fn optimize_code(code: &[Node], options: &OptimizationOptions) -> Vec<Node> {
    let mut optimizations: Vec<Box<OptimizationStep>> = vec![];

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
        optimizations.push(Box::new(CollapseSimpleLoops));
        if options.collapsed_offsets {
            optimizations.push(Box::new(CollapseOffsets));
        }
        optimizations.push(Box::new(DeferMovements));
    }
    if options.collapsed_scan_loops {
        optimizations.push(Box::new(CollapseScanLoops));
    }

    let mut c = code.to_owned();
    for o in optimizations {
        c = o.apply(&c);
    }

    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_optimize_away_comments() {
        let code = vec![
            Node::Comment('a'),
            Node::Shift(1),
            Node::Comment('b'),
            Node::Conditional(vec![
                Node::Comment('a'),
                Node::Shift(1),
                Node::Conditional(vec![Node::Comment('a'), Node::Inc(1, 0, false)]),
            ]),
        ];
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(
            result,
            vec!(
                Node::Shift(1),
                Node::Conditional(vec!(
                    Node::Shift(1),
                    Node::Conditional(vec!(Node::Inc(1, 0, false),))
                ))
            )
        );
    }

    #[test]
    fn it_should_optimize_away_repeated_operators() {
        let code = vec![
            Node::Shift(1),
            Node::Comment('a'),
            Node::Shift(1),
            Node::Shift(1),
            Node::Shift(-1),
            Node::Shift(-1),
            Node::Shift(1),
            Node::Conditional(vec![
                Node::Inc(1, 1, false),
                Node::Comment('a'),
                Node::Inc(1, 1, false),
                Node::Conditional(vec![
                    Node::Comment('a'),
                    Node::Shift(1),
                    Node::Dec(1, 1, false),
                    Node::Shift(1),
                    Node::Dec(1, 1, false),
                    Node::Dec(1, 1, false),
                ]),
            ]),
        ];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: true,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: false,
            },
        );

        assert_eq!(
            result,
            vec!(
                Node::Shift(2),
                Node::Conditional(vec!(
                    Node::Inc(2, 1, false),
                    Node::Conditional(vec!(
                        Node::Shift(1),
                        Node::Dec(1, 1, false),
                        Node::Shift(1),
                        Node::Dec(2, 1, false)
                    ))
                ))
            )
        );
    }

    #[test]
    fn it_should_not_optimize_operators_that_would_overflow() {
        let code = vec![
            Node::Shift(i32::max_value() - 1),
            Node::Shift(1),
            Node::Shift(1),
        ];
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(result, vec!(Node::Shift(i32::max_value()), Node::Shift(1)));
    }

    #[test]
    fn it_should_not_optimize_operators_with_different_offsets() {
        let code = vec![
            Node::Inc(1, 0, false),
            Node::Inc(1, 1, false),
            Node::Dec(1, 0, false),
            Node::Dec(1, 1, false),
            Node::Assign(1, 0, false),
            Node::Assign(1, 1, false),
        ];
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(
            result,
            vec!(
                Node::Inc(1, 0, false),
                Node::Inc(1, 1, false),
                Node::Dec(1, 0, false),
                Node::Dec(1, 1, false),
                Node::Assign(1, 0, false),
                Node::Assign(1, 1, false),
            )
        );
    }

    #[test]
    fn it_should_optimize_zero_loops() {
        let code = vec![
            Node::Conditional(vec![Node::Dec(1, 0, false)]),
            Node::Conditional(vec![Node::Conditional(vec![Node::Dec(1, 0, false)])]),
        ];
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(
            result,
            vec!(
                Node::Assign(0, 0, false),
                Node::Conditional(vec!(Node::Assign(0, 0, false),))
            )
        );
    }

    #[test]
    fn it_should_optimize_assignment_loops() {
        let code = vec![
            Node::Conditional(vec![Node::Dec(1, 0, false)]),
            Node::Inc(100, 0, false),
            Node::Conditional(vec![Node::Dec(1, 0, false)]),
            Node::Dec(1, 0, false),
            Node::Conditional(vec![
                Node::Conditional(vec![Node::Dec(1, 0, false)]),
                Node::Inc(100, 0, false),
            ]),
        ];
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(
            result,
            vec!(
                Node::Assign(100, 0, false),
                Node::Assign(255, 0, false),
                Node::Conditional(vec!(Node::Assign(100, 0, false),))
            )
        );
    }

    #[test]
    fn it_should_collapse_to_positive_offsets() {
        let code = vec![
            Node::Shift(5),
            Node::Inc(1, 0, false),
            Node::Shift(5),
            Node::Dec(1, 0, false),
            Node::Shift(5),
            Node::Assign(1, 0, false),
            Node::Shift(5),
            Node::In(0, false),
            Node::Shift(5),
            Node::Out(0, false),
            Node::Conditional(vec![
                Node::Shift(5),
                Node::Inc(1, 0, false),
                Node::Shift(5),
                Node::Dec(1, 0, false),
                Node::Shift(5),
                Node::Assign(1, 0, false),
            ]),
        ];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(
            result,
            vec!(
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
            )
        );
    }

    #[test]
    fn it_should_collapse_to_negative_offsets() {
        let code = vec![
            Node::Shift(-5),
            Node::Inc(1, 0, false),
            Node::Shift(-5),
            Node::Dec(1, 0, false),
            Node::Shift(-5),
            Node::Assign(1, 0, false),
            Node::Shift(-5),
            Node::In(0, false),
            Node::Shift(-5),
            Node::Out(0, false),
            Node::Conditional(vec![
                Node::Shift(-5),
                Node::Inc(1, 0, false),
                Node::Shift(-5),
                Node::Dec(1, 0, false),
                Node::Shift(-5),
                Node::Assign(1, 0, false),
            ]),
        ];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(
            result,
            vec!(
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
            )
        );
    }

    #[test]
    fn it_should_collapse_non_moving_inc_nodes() {
        let code = vec![
            Node::Shift(-5),
            Node::Inc(1, 0, false),
            Node::Shift(5),
            Node::Shift(5),
            Node::Inc(2, 0, false),
            Node::Shift(-5),
        ];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(
            result,
            vec!(Node::Inc(1, -5, false), Node::Inc(2, 5, false),)
        );
    }

    #[test]
    fn it_should_collapse_imbalanced_inc_nodes() {
        let code = vec![Node::Shift(-5), Node::Inc(1, 0, false), Node::Shift(7)];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(result, vec!(Node::Inc(1, -5, false), Node::Shift(2),));
    }

    #[test]
    fn it_should_collapse_imbalanced_inc_nodes_2() {
        let code = vec![Node::Shift(-7), Node::Inc(1, 0, false), Node::Shift(5)];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(result, vec!(Node::Shift(-2), Node::Inc(1, -5, false),));
    }

    #[test]
    fn it_should_collapse_imbalanced_inc_nodes_3() {
        let code = vec![Node::Shift(7), Node::Inc(1, 0, false), Node::Shift(-5)];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(result, vec!(Node::Shift(2), Node::Inc(1, 5, false),));
    }

    #[test]
    fn it_should_collapse_imbalanced_inc_nodes_4() {
        let code = vec![Node::Shift(5), Node::Inc(1, 0, false), Node::Shift(-9)];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(result, vec!(Node::Inc(1, 5, false), Node::Shift(-4),));
    }

    #[test]
    fn it_should_collapse_non_moving_in_nodes() {
        let code = vec![
            Node::Shift(-5),
            Node::In(0, false),
            Node::Shift(5),
            Node::Shift(5),
            Node::In(0, false),
            Node::Shift(-5),
        ];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(result, vec!(Node::In(-5, false), Node::In(5, false),));
    }

    #[test]
    fn it_should_collapse_non_moving_nodes() {
        let code = vec![
            Node::Shift(-5),
            Node::Inc(1, 0, false),
            Node::Shift(5),
            Node::Shift(-5),
            Node::Dec(1, 0, false),
            Node::Shift(5),
            Node::Shift(5),
            Node::Assign(1, 0, false),
            Node::Shift(-5),
            Node::Shift(5),
            Node::In(0, false),
            Node::Shift(-5),
            Node::Shift(5),
            Node::Out(0, false),
            Node::Shift(-5),
            Node::Conditional(vec![
                Node::Shift(-5),
                Node::Inc(1, 0, false),
                Node::Shift(5),
                Node::Shift(-5),
                Node::Dec(1, 0, false),
                Node::Shift(5),
                Node::Shift(5),
                Node::Assign(1, 0, false),
                Node::Shift(-5),
            ]),
        ];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(
            result,
            vec!(
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
            )
        );
    }

    #[test]
    fn it_should_collapse_imbalanced_non_moving_nodes() {
        let code = vec![
            Node::Shift(-7),
            Node::Inc(1, 0, false),
            Node::Shift(5),
            Node::Shift(-5),
            Node::Inc(1, 0, false),
            Node::Shift(7),
        ];
        let result = optimize_code(
            &code,
            &OptimizationOptions {
                collapsed_scan_loops: false,
                collapsed_operators: false,
                collapsed_loops: false,
                collapsed_assignments: false,
                collapsed_offsets: true,
            },
        );

        assert_eq!(
            result,
            vec!(
                Node::Shift(-2),
                Node::Inc(1, -5, false),
                Node::Inc(1, -5, false),
                Node::Shift(2)
            )
        );
    }

    #[test]
    fn it_should_defer_movement() {
        let code = vec![
            Node::Shift(-1),
            Node::Shift(6),
            Node::Inc(1, 5, true),
            Node::Inc(1, 5, false),
            Node::Mul(1, -5, 5, false),
            Node::Conditional(vec![Node::Dec(1, 5, true), Node::Out(-5, true)]),
            Node::Shift(-10),
            Node::Inc(1, 5, true),
        ];
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(
            result,
            vec!(
                Node::Inc(1, 10, false),
                Node::Inc(1, 15, false),
                Node::Mul(1, -5, 15, false),
                Node::Shift(10),
                Node::Conditional(vec!(Node::Dec(1, 5, false), Node::Out(0, false))),
                Node::Inc(1, -5, false),
                Node::Shift(-5),
            )
        );
    }

    #[test]
    fn it_should_collapse_simple_loops() {
        let code = vec![
            Node::Conditional(vec![
                Node::Inc(2, 5, false),
                Node::Inc(4, -5, false),
                Node::Dec(4, -5, false),
                Node::Dec(1, 0, false),
            ]),
            Node::Conditional(vec![Node::Conditional(vec![
                Node::Inc(2, 5, false),
                Node::Dec(1, 0, false),
                Node::Inc(4, -5, false),
            ])]),
        ];
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(
            result,
            vec!(
                Node::Mul(2, 5, 0, false),
                Node::Mul(4, -5, 0, false),
                Node::Mul(-4, -5, 0, false),
                Node::Assign(0, 0, false),
                Node::Conditional(vec!(
                    Node::Mul(2, 5, 0, false),
                    Node::Mul(4, -5, 0, false),
                    Node::Assign(0, 0, false),
                )),
            )
        );
    }

    #[test]
    fn it_should_collapse_scan_loops() {
        let code = vec![
            Node::Conditional(vec![Node::Shift(-1)]),
            Node::Conditional(vec![Node::Shift(3)]),
            Node::Conditional(vec![
                Node::Conditional(vec![Node::Shift(-1)]),
                Node::Conditional(vec![Node::Shift(3)]),
            ]),
        ];
        let result = optimize_code(&code, &OptimizationOptions::default());

        assert_eq!(
            result,
            vec!(
                Node::Scan(-1),
                Node::Scan(3),
                Node::Conditional(vec!(Node::Scan(-1), Node::Scan(3),)),
            )
        );
    }
}
