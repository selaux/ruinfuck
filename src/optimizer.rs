use vm::Node;

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

pub fn optimize_code(code: &Vec<Node>) -> Vec<Node> {
    let without_comments: Vec<Node> = code
        .into_iter()
        .flat_map(filter_comments)
        .collect();
    let joined_operators = join_repeated_operators(&without_comments);
    let without_zero_loops = replace_zero_loops(&joined_operators);

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
        );
        let result = optimize_code(&code);

        assert_eq!(result, vec!(
            Node::Assign(0)
        ));
    }
}
