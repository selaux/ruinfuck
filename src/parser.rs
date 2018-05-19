use std::io::{Read, BufRead};

use vm::Node;

#[derive(Debug, PartialEq)]
pub enum ParserError {
    UnmatchedDelimiter,
    MissingDelimiter,
    Io(String),
    Internal
}

pub fn parse_code<F: BufRead>(code: &mut F) -> Result<Vec<Node>, ParserError> {
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

    let res = nested.pop().ok_or(ParserError::Internal)?;

    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_parse_instructions() {
        let code = "<>+-.,";
        let result = parse_code(&mut code.as_bytes());

        assert_eq!(result, Ok(vec!(
            Node::Left(1),
            Node::Right(1),
            Node::Inc(1, 0),
            Node::Dec(1, 0),
            Node::Out(0),
            Node::In(0)
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
}
