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
                (Some(Node::Inc(x, offset1)), Node::Inc(y, offset2)) => {
                    if x as u16 + y as u16 > 255 || offset1 != offset2 {
                        acc.push(Node::Inc(x, offset1));
                        acc.push(Node::Inc(y, offset2));
                    } else {
                        acc.push(Node::Inc(x + y, offset1));
                    }
                },
                (Some(Node::Dec(x, offset1)), Node::Dec(y, offset2)) => {
                    if x as u16 + y as u16 > 255 || offset1 != offset2 {
                        acc.push(Node::Dec(x, offset1));
                        acc.push(Node::Dec(y, offset2));
                    } else {
                        acc.push(Node::Dec(x + y, offset1));
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

struct ReplaceZeroAssignments;

impl OptimizationStep for ReplaceZeroAssignments {
    fn apply(&self, code: Vec<Node>) -> Vec<Node> {
        code
            .into_iter()
            .map(move |n| match n {
                Node::Conditional(body) => {
                    if body == vec!(Node::Dec(1, 0)) {
                        Node::Assign(0, 0)
                    } else {
                        Node::Conditional(self.apply(body))
                    }
                },
                n => n
            })
            .collect()
    }
}

pub fn optimize_code(code: &Vec<Node>) -> Vec<Node> {
    let without_comments: Vec<Node> = FilterComments.apply(code.clone());
    let joined_operators = MergeRepeatedOperators.apply(without_comments);
    let without_zero_loops = ReplaceZeroAssignments.apply(joined_operators);

    without_zero_loops
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
                Node::Inc(1, 0),
                Node::Comment('a'),
                Node::Inc(1, 0),
                Node::Conditional(vec!(
                    Node::Comment('a'),
                    Node::Right(1),
                    Node::Dec(1, 0),
                    Node::Right(1),
                    Node::Dec(1, 0),
                    Node::Dec(1, 0),
                ))
            ))
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Right(3),
            Node::Left(2),
            Node::Right(1),
            Node::Conditional(vec!(
                Node::Inc(2, 0),
                Node::Conditional(vec!(
                    Node::Right(1),
                    Node::Dec(1, 0),
                    Node::Right(1),
                    Node::Dec(2, 0)
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
            Node::Inc(1, 0),
            Node::Inc(1, 1),
            Node::Dec(1, 0),
            Node::Dec(1, 1),
            Node::Assign(1, 0),
            Node::Assign(1, 1),
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Inc(1, 0),
            Node::Inc(1, 1),
            Node::Dec(1, 0),
            Node::Dec(1, 1),
            Node::Assign(1, 0),
            Node::Assign(1, 1),
        ));
    }

    #[test]
    fn it_should_optimize_zero_loops() {
        let code = vec!(
            Node::Conditional(vec!(Node::Dec(1, 0))),
            Node::Conditional(vec!(
                Node::Conditional(vec!(Node::Dec(1, 0)))
            ))
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Assign(0, 0),
            Node::Conditional(vec!(
                Node::Assign(0, 0),
            ))
        ));
    }
}
