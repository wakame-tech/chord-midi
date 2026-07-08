use crate::{
    model::ast::{Ast, ChordNode, Node},
    model::pitch::Pitch,
};

fn chordnode_into_degree(chord: ChordNode, key: Pitch) -> ChordNode {
    ChordNode {
        key: chord.key.into_degree(key),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_degree(key)),
    }
}

fn chordnode_into_pitch(chord: ChordNode, pitch: Pitch) -> ChordNode {
    ChordNode {
        key: chord.key.into_pitch(pitch),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_pitch(pitch)),
    }
}

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
                        Node::Chord(chord) => Node::Chord(chordnode_into_degree(chord, key)),
                        Node::Arpeggio(chord) => {
                            Node::Arpeggio(chordnode_into_degree(chord, key))
                        }
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
                        Node::Chord(chord) => Node::Chord(chordnode_into_pitch(chord, pitch)),
                        Node::Arpeggio(chord) => {
                            Node::Arpeggio(chordnode_into_pitch(chord, pitch))
                        }
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
        // Key::Relative stores SEMITONES above the key root, not a scale degree.
        // I (root, 0 semitones) in key C == C
        let i = Node::Chord(ChordNode::relative(0));
        let c = Node::Chord(ChordNode::absolute(Pitch::C));
        assert_eq!(
            Ast::Measure(vec![i], false).into_pitch(Pitch::C),
            Ast::Measure(vec![c], false)
        );

        // IV (perfect 4th, 5 semitones) in key C == F
        let iv = Node::Chord(ChordNode::relative(5));
        let f = Node::Chord(ChordNode::absolute(Pitch::F));
        assert_eq!(
            Ast::Measure(vec![iv], false).into_pitch(Pitch::C),
            Ast::Measure(vec![f], false)
        );
    }

    #[test]
    fn test_into_degree_is_inverse() {
        // absolute -> degree: V (G) in key C == 7 semitones
        let g = Node::Chord(ChordNode::absolute(Pitch::G));
        let v = Node::Chord(ChordNode::relative(7));
        assert_eq!(
            Ast::Measure(vec![g], false).into_degree(Pitch::C),
            Ast::Measure(vec![v], false)
        );
    }
}
