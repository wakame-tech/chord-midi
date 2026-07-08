use crate::{
    model::ast::{Ast, ChordNode, Node},
    model::pitch::Pitch,
    model::scale::Scale,
};

fn chordnode_into_degree(chord: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
    ChordNode {
        key: chord.key.into_degree(tonic, scale),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_degree(tonic, scale)),
    }
}

fn chordnode_into_pitch(chord: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
    ChordNode {
        key: chord.key.into_pitch(tonic, scale),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_pitch(tonic, scale)),
    }
}

impl Ast {
    pub fn into_degree(self, tonic: Pitch, scale: Scale) -> Ast {
        match self {
            Ast::Score(nodes) => Ast::Score(
                nodes
                    .into_iter()
                    .map(|ast| Box::new((*ast).into_degree(tonic, scale)))
                    .collect::<Vec<_>>(),
            ),
            Ast::Measure(nodes, br) => Ast::Measure(
                nodes
                    .into_iter()
                    .map(|node| match node {
                        Node::Chord(c) => Node::Chord(chordnode_into_degree(c, tonic, scale)),
                        Node::Arpeggio(c) => Node::Arpeggio(chordnode_into_degree(c, tonic, scale)),
                        other => other,
                    })
                    .collect::<Vec<_>>(),
                br,
            ),
            other => other,
        }
    }

    pub fn into_pitch(self, tonic: Pitch, scale: Scale) -> Ast {
        match self {
            Ast::Score(nodes) => Ast::Score(
                nodes
                    .into_iter()
                    .map(|ast| Box::new((*ast).into_pitch(tonic, scale)))
                    .collect::<Vec<_>>(),
            ),
            Ast::Measure(nodes, br) => Ast::Measure(
                nodes
                    .into_iter()
                    .map(|node| match node {
                        Node::Chord(c) => Node::Chord(chordnode_into_pitch(c, tonic, scale)),
                        Node::Arpeggio(c) => Node::Arpeggio(chordnode_into_pitch(c, tonic, scale)),
                        other => other,
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
        use crate::model::pitch::Accidental;
        use crate::model::scale::{Degree, Scale};
        let i = Node::Chord(ChordNode::relative(Degree(1, Accidental::Natural)));
        let c = Node::Chord(ChordNode::absolute(Pitch::C));
        assert_eq!(
            Ast::Measure(vec![i], false).into_pitch(Pitch::C, Scale::Major),
            Ast::Measure(vec![c], false)
        );
        let iv = Node::Chord(ChordNode::relative(Degree(4, Accidental::Natural)));
        let f = Node::Chord(ChordNode::absolute(Pitch::F));
        assert_eq!(
            Ast::Measure(vec![iv], false).into_pitch(Pitch::C, Scale::Major),
            Ast::Measure(vec![f], false)
        );
    }

    #[test]
    fn test_into_degree_is_inverse() {
        use crate::model::pitch::Accidental;
        use crate::model::scale::{Degree, Scale};
        let g = Node::Chord(ChordNode::absolute(Pitch::G));
        let v = Node::Chord(ChordNode::relative(Degree(5, Accidental::Natural)));
        assert_eq!(
            Ast::Measure(vec![g], false).into_degree(Pitch::C, Scale::Major),
            Ast::Measure(vec![v], false)
        );
    }
}
