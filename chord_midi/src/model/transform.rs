use crate::{
    model::ast::{Ast, ChordNode, Directive, Node},
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
                        Node::Transform(t, c) => {
                            Node::Transform(t, chordnode_into_degree(c, tonic, scale))
                        }
                        Node::Brush(c) => Node::Brush(chordnode_into_degree(c, tonic, scale)),
                        other => other,
                    })
                    .collect::<Vec<_>>(),
                br,
            ),
            Ast::Apply(func, items) => Ast::Apply(
                func,
                items
                    .into_iter()
                    .map(|ast| Box::new((*ast).into_degree(tonic, scale)))
                    .collect(),
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
                        Node::Transform(t, c) => {
                            Node::Transform(t, chordnode_into_pitch(c, tonic, scale))
                        }
                        Node::Brush(c) => Node::Brush(chordnode_into_pitch(c, tonic, scale)),
                        other => other,
                    })
                    .collect::<Vec<_>>(),
                br,
            ),
            Ast::Apply(func, items) => Ast::Apply(
                func,
                items
                    .into_iter()
                    .map(|ast| Box::new((*ast).into_pitch(tonic, scale)))
                    .collect(),
            ),
            other => other,
        }
    }

    /// Apply key/scale directives: transpose degrees to pitches (including the
    /// inner chord of Transform/Arpeggio nodes) and strip Directive/Comment nodes,
    /// yielding a playback-ready AST. Transform nodes are expanded later, at
    /// MIDI interpret time (`Score::interpret_node`).
    ///
    /// When no `key:` directive is present the tonic defaults to C, so a
    /// degree-notation progression (e.g. a preset) always resolves to concrete
    /// pitches and plays, rather than leaving unplayable relative keys.
    pub fn resolve(self) -> Ast {
        let mut tonic = Pitch::C;
        let mut scale = Scale::Major;
        if let Ast::Score(items) = &self {
            for it in items {
                match it.as_ref() {
                    Ast::Directive(Directive::Key(p)) => tonic = *p,
                    Ast::Directive(Directive::Scale(s)) => scale = *s,
                    _ => {}
                }
            }
        }
        let transposed = self.into_pitch(tonic, scale);
        match transposed {
            Ast::Score(items) => Ast::Score(
                items
                    .into_iter()
                    .filter(|it| !matches!(it.as_ref(), Ast::Directive(_) | Ast::Comment(_)))
                    .collect(),
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
    fn test_resolve_transposes_degrees() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Ast, Node};
        let ast = RechordImporter.import("key: C\nV7 |\n").unwrap();
        let resolved = ast.resolve();
        let Ast::Score(items) = &resolved else { panic!() };
        let mut found = false;
        for it in items {
            // directives stripped: only Measure should remain
            assert!(!matches!(it.as_ref(), Ast::Directive(_)));
            if let Ast::Measure(nodes, _) = it.as_ref() {
                for n in nodes {
                    if let Node::Chord(c) = n {
                        assert!(matches!(c.key, crate::model::key::Key::Absolute(_)));
                        found = true;
                    }
                }
            }
        }
        assert!(found);
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
