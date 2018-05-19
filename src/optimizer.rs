use vm::Node;

fn filter_comments(code: Vec<Node>) -> Vec<Node> {
    code
        .into_iter()
        .flat_map(move |n| match n {
            Node::Comment(_) => None,
            Node::Conditional(body) => Some(Node::Conditional(filter_comments(body))),
            n => Some(n)
        })
        .collect()
}

fn join_repeated_operators(code_without_comments: Vec<Node>) -> Vec<Node> {
    code_without_comments.into_iter().fold(vec!(), move |mut acc, c| {
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
            (Some(Node::Inc(x)), Node::Inc(y)) => {
                if x as u16 + y as u16 > 255 {
                    acc.push(Node::Inc(x));
                    acc.push(Node::Inc(y));
                } else {
                    acc.push(Node::Inc(x + y));
                }
            },
            (Some(Node::Dec(x)), Node::Dec(y)) => {
                if x as u16 + y as u16 > 255 {
                    acc.push(Node::Dec(x));
                    acc.push(Node::Dec(y));
                } else {
                    acc.push(Node::Dec(x + y));
                }
            },
            (l, Node::Conditional(body)) => {
                match l {
                    Some(c) => acc.push(c),
                    None => {}
                }

                acc.push(Node::Conditional(join_repeated_operators(body)));
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

fn replace_zero_loops(code_without_comments: Vec<Node>) -> Vec<Node> {
    return code_without_comments
        .into_iter()
        .map(move |n| match n {
            Node::Conditional(body) => {
                if body == vec!(Node::Dec(1)) {
                    Node::Assign(0)
                } else {
                    Node::Conditional(replace_zero_loops(body))
                }
            },
            n => n
        })
        .collect()
}

pub fn optimize_code(code: &Vec<Node>) -> Vec<Node> {
    let without_comments: Vec<Node> = filter_comments(code.clone());
    let joined_operators = join_repeated_operators(without_comments);
    let without_zero_loops = replace_zero_loops(joined_operators);

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
            Node::Conditional(vec!(
                Node::Conditional(vec!(Node::Dec(1)))
            ))
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Assign(0),
            Node::Conditional(vec!(
                Node::Assign(0),
            ))
        ));
    }

    #[test]
    fn it_should_optimize_offset_assignments() {
        let code = vec!(
            Node::Conditional(vec!(
                Node::Conditional(vec!(Node::Dec(1)))
            ))
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Conditional(vec!(
                Node::Assign(0),
            ))
        ));
    }
}
