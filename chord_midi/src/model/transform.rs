use crate::{
    model::ast::{Ast, ChordNode, Node},
    model::pitch::Pitch,
};

impl Ast {
    pub fn into_degree(self, key: Pitch) -> Ast {
        match self {
            Ast::Score(nodes) => Ast::Score(
                nodes
                    .into_iter()
                    .map(|ast| Box::new(Ast::into_degree(*ast, key)))
                    .collect::<Vec<_>>(),
            ),
            Ast::Measure(nodes, br) => Ast::Measure(
                nodes
                    .into_iter()
                    .map(|node| match node {
                        Node::Chord(chord) => Node::Chord(ChordNode {
                            key: chord.key.into_degree(key),
                            modifiers: chord.modifiers,
                            on: chord.on.map(|on| on.into_degree(key)),
                        }),
                        _ => node,
                    })
                    .collect::<Vec<_>>(),
                br,
            ),
            other => other,
        }
    }

    pub fn into_pitch(self, pitch: Pitch) -> Ast {
        match self {
            Ast::Score(nodes) => Ast::Score(
                nodes
                    .into_iter()
                    .map(|ast| Box::new(Ast::into_pitch(*ast, pitch)))
                    .collect::<Vec<_>>(),
            ),
            Ast::Measure(nodes, br) => Ast::Measure(
                nodes
                    .into_iter()
                    .map(|node| match node {
                        Node::Chord(chord) => Node::Chord(ChordNode {
                            key: chord.key.into_pitch(pitch),
                            modifiers: chord.modifiers,
                            on: chord.on.map(|on| on.into_pitch(pitch)),
                        }),
                        _ => node,
                    })
                    .collect::<Vec<_>>(),
                br,
            ),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{
        ast::{Ast, ChordNode, Node},
        pitch::Pitch,
    };

    #[test]
    fn test_transpose() {
        let i = Node::Chord(ChordNode::relative(1));
        let c = Node::Chord(ChordNode::absolute(Pitch::C));
        assert_eq!(
            Ast::Measure(vec![i], false).into_pitch(Pitch::C),
            Ast::Measure(vec![c], false)
        );

        let iv = Node::Chord(ChordNode::relative(6));
        let f = Node::Chord(ChordNode::absolute(Pitch::F));
        assert_eq!(
            Ast::Measure(vec![iv], false).into_pitch(Pitch::C),
            Ast::Measure(vec![f], false)
        );
    }
}
