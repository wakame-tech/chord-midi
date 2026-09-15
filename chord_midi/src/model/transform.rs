use crate::{
    model::ast::{Ast, ChordNode, Directive, Func, Node},
    model::modifier::Modifier,
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
            Ast::Apply(func, items, br) => Ast::Apply(
                func,
                items
                    .into_iter()
                    .map(|ast| Box::new((*ast).into_degree(tonic, scale)))
                    .collect(),
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
                        Node::Transform(t, c) => {
                            Node::Transform(t, chordnode_into_pitch(c, tonic, scale))
                        }
                        Node::Brush(c) => Node::Brush(chordnode_into_pitch(c, tonic, scale)),
                        other => other,
                    })
                    .collect::<Vec<_>>(),
                br,
            ),
            Ast::Apply(func, items, br) => Ast::Apply(
                func,
                items
                    .into_iter()
                    .map(|ast| Box::new((*ast).into_pitch(tonic, scale)))
                    .collect(),
                br,
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
        // pow は音程の話なのでここで潰す。pat はリズムの話なので残す
        let desugared = desugar_pow(transposed)
            .into_iter()
            .next()
            .expect("desugar_pow always yields at least one node for a Score");
        match desugared {
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

/// `Apply(Func::Pow, ..)` を潰し、中の全コードに `Modifier::Power` を足す。
///
/// `Apply` は親のリストの中で複数の要素に展開されるので `Vec` を返す。
fn desugar_pow(ast: Ast) -> Vec<Ast> {
    match ast {
        Ast::Score(items) => vec![Ast::Score(
            items
                .into_iter()
                .flat_map(|item| desugar_pow(*item))
                .map(Box::new)
                .collect(),
        )],
        Ast::Apply(Func::Pow, items, _) => items
            .into_iter()
            .flat_map(|item| desugar_pow(*item))
            .map(mark_power)
            .collect(),
        Ast::Apply(func, items, br) => vec![Ast::Apply(
            func,
            items
                .into_iter()
                .flat_map(|item| desugar_pow(*item))
                .map(Box::new)
                .collect(),
            br,
        )],
        other => vec![other],
    }
}

/// AST に含まれる全コードに `Modifier::Power` を足す
fn mark_power(ast: Ast) -> Ast {
    match ast {
        Ast::Measure(nodes, br) => {
            Ast::Measure(nodes.into_iter().map(mark_power_node).collect(), br)
        }
        Ast::Apply(func, items, br) => Ast::Apply(
            func,
            items
                .into_iter()
                .map(|item| Box::new(mark_power(*item)))
                .collect(),
            br,
        ),
        other => other,
    }
}

fn mark_power_node(node: Node) -> Node {
    match node {
        Node::Chord(c) => Node::Chord(mark_power_chord(c)),
        Node::Arpeggio(c) => Node::Arpeggio(mark_power_chord(c)),
        Node::Brush(c) => Node::Brush(mark_power_chord(c)),
        Node::Transform(t, c) => Node::Transform(t, mark_power_chord(c)),
        other => other,
    }
}

fn mark_power_chord(mut chord: ChordNode) -> ChordNode {
    chord.modifiers.insert(Modifier::Power);
    chord
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

    #[test]
    fn test_pow_becomes_root_and_fifth() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;

        fn semitones(code: &str) -> Vec<u8> {
            let ast = RechordImporter.import(code).unwrap().resolve();
            let Ast::Score(items) = &ast else { panic!("not a score") };
            let Ast::Measure(nodes, _) = items[0].as_ref() else {
                panic!("pow should be desugared into a measure: {:?}", items[0])
            };
            let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
            c.to_chord().unwrap().semitones.into_iter().collect()
        }

        // ルートと5thだけになる
        assert_eq!(semitones("pow(C)\n"), vec![0, 7]);
        // 元の修飾を上書きする
        assert_eq!(semitones("pow(Cm7)\n"), vec![0, 7]);
        assert_eq!(semitones("pow(CM7)\n"), vec![0, 7]);
    }

    #[test]
    fn test_pow_keeps_slash_bass() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;

        let ast = RechordImporter.import("pow(C/E)\n").unwrap().resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Measure(nodes, _) = items[0].as_ref() else { panic!("not a measure") };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(c.on, Some(Key::Absolute(Pitch::E)));
    }

    #[test]
    fn test_pow_flattens_all_measures() {
        use crate::import::{Importer, RechordImporter};

        let ast = RechordImporter.import("pow(C | E D)\n").unwrap().resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        // Apply は消えて2つの Measure になる
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| matches!(i.as_ref(), Ast::Measure(_, _))));
    }

    #[test]
    fn test_pat_survives_resolve() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Func;

        let ast = RechordImporter.import("pat(\"oxox\", C)\n").unwrap().resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        // リズムは MIDI の関心なので resolve では展開しない
        assert!(matches!(items[0].as_ref(), Ast::Apply(Func::Pat(_), _, _)));
    }

    #[test]
    fn test_pow_inside_pat_is_desugared() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Func, Node};

        let ast = RechordImporter
            .import("pat(\"oxox\", pow(C))\n")
            .unwrap()
            .resolve();
        let Ast::Score(items) = &ast else { panic!("not a score") };
        let Ast::Apply(Func::Pat(_), inner, _) = items[0].as_ref() else { panic!("not a pat") };
        let Ast::Measure(nodes, _) = inner[0].as_ref() else {
            panic!("pow should be desugared inside pat: {:?}", inner[0])
        };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(
            c.to_chord().unwrap().semitones.into_iter().collect::<Vec<_>>(),
            vec![0, 7]
        );
    }
}
