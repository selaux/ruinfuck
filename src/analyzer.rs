use std::default::Default;
use std::collections::HashMap;

use vm::Node;

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisResults {
    total: u32,
    nodes: HashMap<Node, u32>
}

impl AnalysisResults {
    fn merge(&mut self, nested: &AnalysisResults) {
        self.total += nested.total;

        for (k, v) in &nested.nodes {
            self.nodes
                .entry(k.clone())
                .and_modify(|n| *n += v)
                .or_insert(*v);
        }
    }
}

impl Default for AnalysisResults {
    fn default() -> Self {
        AnalysisResults {
            total: 0,
            nodes: HashMap::new()
        }
    }
}

pub trait Analyzer {
    fn analyze(&self, code: &Vec<Node>) -> AnalysisResults;
}

pub struct SimpleAnalyzer {}

impl Analyzer for SimpleAnalyzer {
    fn analyze(&self, code: &Vec<Node>) -> AnalysisResults {
        code.into_iter().fold(AnalysisResults::default(), move |mut memo, v| {
            memo.total = memo.total + 1;

            {
                let entry = match v {
                    Node::Left(_) => memo.nodes.entry(Node::Left(0)),
                    Node::Right(_) => memo.nodes.entry(Node::Right(0)),
                    Node::Inc(_, _, _) => memo.nodes.entry(Node::Inc(0, 0, false)),
                    Node::Dec(_, _, _) => memo.nodes.entry(Node::Dec(0, 0, false)),
                    Node::Assign(_, _, _) => memo.nodes.entry(Node::Assign(0, 0, false)),
                    Node::Out(_, _) => memo.nodes.entry(Node::Out(0, false)),
                    Node::In(_, _) => memo.nodes.entry(Node::Out(0, false)),
                    Node::Comment(_) => memo.nodes.entry(Node::Comment(' ')),
                    Node::Conditional(_) => memo.nodes.entry(Node::Conditional(vec!())),
                };
                entry.and_modify(|e| { *e += 1 }).or_insert(1);
            }
            if let Node::Conditional(v) = v {
                let nested = self.analyze(v);
                memo.merge(&nested);
            }

            memo
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_should_return_empty_results() {
        let code = vec!();
        let analyzer = SimpleAnalyzer {};
        let result = analyzer.analyze(&code);

        assert_eq!(result, AnalysisResults { total: 0, nodes: HashMap::new() });
    }

    #[test]
    fn it_should_return_results_for_a_single_node() {
        let code = vec!(
            Node::Left(1)
        );
        let analyzer = SimpleAnalyzer {};
        let result = analyzer.analyze(&code);
        let mut expected_nodes = HashMap::new();

        expected_nodes.insert(Node::Left(0), 1);

        assert_eq!(result, AnalysisResults { total: 1, nodes: expected_nodes });
    }

    #[test]
    fn it_should_return_results_for_multiple_different_nodes() {
        let code = vec!(
            Node::Left(1),
            Node::Left(2),
            Node::Right(1),
            Node::Inc(1, 1, true)
        );
        let analyzer = SimpleAnalyzer {};
        let result = analyzer.analyze(&code);
        let mut expected_nodes = HashMap::new();

        expected_nodes.insert(Node::Left(0), 2);
        expected_nodes.insert(Node::Right(0), 1);
        expected_nodes.insert(Node::Inc(0, 0, false), 1);

        assert_eq!(result, AnalysisResults { total: 4, nodes: expected_nodes });
    }

    #[test]
    fn it_should_return_results_for_empty_conditionals() {
        let code = vec!(
            Node::Conditional(vec!())
        );
        let analyzer = SimpleAnalyzer {};
        let result = analyzer.analyze(&code);
        let mut expected_nodes = HashMap::new();

        expected_nodes.insert(Node::Conditional(vec!()), 1);

        assert_eq!(result, AnalysisResults { total: 1, nodes: expected_nodes });
    }

    #[test]
    fn it_should_return_results_for_nested_conditionals() {
        let code = vec!(
            Node::Conditional(vec!(
                Node::Left(5),
                Node::Conditional(vec!(
                    Node::Right(5),
                    Node::Left(2),
                ))
            ))
        );
        let analyzer = SimpleAnalyzer {};
        let result = analyzer.analyze(&code);
        let mut expected_nodes = HashMap::new();

        expected_nodes.insert(Node::Conditional(vec!()), 2);
        expected_nodes.insert(Node::Left(0), 2);
        expected_nodes.insert(Node::Right(0), 1);

        assert_eq!(result, AnalysisResults { total: 5, nodes: expected_nodes });
    }
}



