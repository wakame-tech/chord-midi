use crate::{
    model::ast::{Expr, ChordNode, Directive, Func, Node},
    model::modifier::Modifier,
    model::pitch::Pitch,
    model::scale::Scale,
};

fn chordnode_into_degree(chord: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
    ChordNode {
        key: chord.key.into_degree(tonic, scale),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_degree(tonic, scale)),
        root_string: chord.root_string,
    }
}

fn chordnode_into_pitch(chord: ChordNode, tonic: Pitch, scale: Scale) -> ChordNode {
    ChordNode {
        key: chord.key.into_pitch(tonic, scale),
        modifiers: chord.modifiers,
        on: chord.on.map(|on| on.into_pitch(tonic, scale)),
        root_string: chord.root_string,
    }
}

impl Expr {
    pub fn into_degree(self, tonic: Pitch, scale: Scale) -> Expr {
        match self {
            Expr::Seq(nodes) => Expr::Seq(
                nodes
                    .into_iter()
                    .map(|ast| ast.into_degree(tonic, scale))
                    .collect::<Vec<_>>(),
            ),
            Expr::Measure(nodes, br) => Expr::Measure(
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
            Expr::Apply(func, items, br) => Expr::Apply(
                func,
                items
                    .into_iter()
                    .map(|ast| ast.into_degree(tonic, scale))
                    .collect(),
                br,
            ),
            other => other,
        }
    }

    pub fn into_pitch(self, tonic: Pitch, scale: Scale) -> Expr {
        match self {
            Expr::Seq(nodes) => Expr::Seq(
                nodes
                    .into_iter()
                    .map(|ast| ast.into_pitch(tonic, scale))
                    .collect::<Vec<_>>(),
            ),
            Expr::Measure(nodes, br) => Expr::Measure(
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
            Expr::Apply(func, items, br) => Expr::Apply(
                func,
                items
                    .into_iter()
                    .map(|ast| ast.into_pitch(tonic, scale))
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
    pub fn resolve(self) -> Expr {
        let mut tonic = Pitch::C;
        let mut scale = Scale::Major;
        if let Expr::Seq(items) = &self {
            for it in items {
                match it {
                    Expr::Directive(Directive::Key(p)) => tonic = *p,
                    Expr::Directive(Directive::Scale(s)) => scale = *s,
                    _ => {}
                }
            }
        }
        let transposed = self.into_pitch(tonic, scale);
        // pow は音程の話なのでここで潰す。pat はリズムの話なので残す
        let desugared = desugar_shapes(transposed)
            .into_iter()
            .next()
            .expect("desugar_shapes always yields at least one node for a Seq");
        match desugared {
            Expr::Seq(items) => Expr::Seq(
                items
                    .into_iter()
                    .filter(|it| !matches!(it, Expr::Directive(_) | Expr::Comment(_)))
                    .collect(),
            ),
            other => other,
        }
    }
}

/// `Apply(Func::Pow, ..)` と `Apply(Func::Oct, ..)` を潰し、
/// 中の全コードに対応する修飾を足す。
///
/// `Apply` は親のリストの中で複数の要素に展開されるので `Vec` を返す。
fn desugar_shapes(ast: Expr) -> Vec<Expr> {
    match ast {
        Expr::Seq(items) => vec![Expr::Seq(
            items
                .into_iter()
                .flat_map(desugar_shapes)
                .collect(),
        )],
        Expr::Apply(Func::Pow, items, _) => items
            .into_iter()
            .flat_map(desugar_shapes)
            .map(|item| mark(item, Modifier::Power))
            .collect(),
        Expr::Apply(Func::Oct, items, _) => items
            .into_iter()
            .flat_map(desugar_shapes)
            .map(|item| mark(item, Modifier::Octave))
            .collect(),
        Expr::Apply(func, items, br) => vec![Expr::Apply(
            func,
            items.into_iter().flat_map(desugar_shapes).collect(),
            br,
        )],
        other => vec![other],
    }
}

/// AST に含まれる全コードに修飾を足す
fn mark(ast: Expr, modifier: Modifier) -> Expr {
    match ast {
        Expr::Measure(nodes, br) => Expr::Measure(
            nodes
                .into_iter()
                .map(|node| mark_node(node, modifier.clone()))
                .collect(),
            br,
        ),
        Expr::Apply(func, items, br) => Expr::Apply(
            func,
            items
                .into_iter()
                .map(|item| mark(item, modifier.clone()))
                .collect(),
            br,
        ),
        other => other,
    }
}

fn mark_node(node: Node, modifier: Modifier) -> Node {
    match node {
        Node::Chord(c) => Node::Chord(mark_chord(c, modifier)),
        Node::Arpeggio(c) => Node::Arpeggio(mark_chord(c, modifier)),
        Node::Brush(c) => Node::Brush(mark_chord(c, modifier)),
        Node::Transform(t, c) => Node::Transform(t, mark_chord(c, modifier)),
        other => other,
    }
}

fn mark_chord(mut chord: ChordNode, modifier: Modifier) -> ChordNode {
    chord.modifiers.insert(modifier);
    chord
}

#[cfg(test)]
mod tests {
    use crate::model::{
        ast::{Expr, ChordNode, Node},
        pitch::Pitch,
    };

    #[test]
    fn test_transpose() {
        use crate::model::pitch::Accidental;
        use crate::model::scale::{Degree, Scale};
        let i = Node::Chord(ChordNode::relative(Degree(1, Accidental::Natural)));
        let c = Node::Chord(ChordNode::absolute(Pitch::C));
        assert_eq!(
            Expr::Measure(vec![i], false).into_pitch(Pitch::C, Scale::Major),
            Expr::Measure(vec![c], false)
        );
        let iv = Node::Chord(ChordNode::relative(Degree(4, Accidental::Natural)));
        let f = Node::Chord(ChordNode::absolute(Pitch::F));
        assert_eq!(
            Expr::Measure(vec![iv], false).into_pitch(Pitch::C, Scale::Major),
            Expr::Measure(vec![f], false)
        );
    }

    #[test]
    fn test_resolve_transposes_degrees() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Node};
        let ast = RechordImporter.import("key: C\nV7 |\n").unwrap();
        let resolved = ast.resolve();
        let Expr::Seq(items) = &resolved else { panic!() };
        let mut found = false;
        for it in items {
            // directives stripped: only Measure should remain
            assert!(!matches!(it, Expr::Directive(_)));
            if let Expr::Measure(nodes, _) = it {
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
            Expr::Measure(vec![g], false).into_degree(Pitch::C, Scale::Major),
            Expr::Measure(vec![v], false)
        );
    }

    #[test]
    fn test_pow_becomes_root_and_fifth() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;

        fn semitones(code: &str) -> Vec<u8> {
            let ast = RechordImporter.import(code).unwrap().resolve();
            let Expr::Seq(items) = &ast else { panic!("not a score") };
            let Expr::Measure(nodes, _) = &items[0] else {
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
        let Expr::Seq(items) = &ast else { panic!("not a score") };
        let Expr::Measure(nodes, _) = &items[0] else { panic!("not a measure") };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(c.on, Some(Key::Absolute(Pitch::E)));
    }

    #[test]
    fn test_pow_flattens_all_measures() {
        use crate::import::{Importer, RechordImporter};

        let ast = RechordImporter.import("pow(C | E D)\n").unwrap().resolve();
        let Expr::Seq(items) = &ast else { panic!("not a score") };
        // Apply は消えて2つの Measure になる
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| matches!(i, Expr::Measure(_, _))));
    }

    #[test]
    fn test_pat_survives_resolve() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Func;

        let ast = RechordImporter.import("pat(\"oxox\", C)\n").unwrap().resolve();
        let Expr::Seq(items) = &ast else { panic!("not a score") };
        // リズムは MIDI の関心なので resolve では展開しない
        assert!(matches!(&items[0], Expr::Apply(Func::Pat(_), _, _)));
    }

    #[test]
    fn test_pow_inside_pat_is_desugared() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Func, Node};

        let ast = RechordImporter
            .import("pat(\"oxox\", pow(C))\n")
            .unwrap()
            .resolve();
        let Expr::Seq(items) = &ast else { panic!("not a score") };
        let Expr::Apply(Func::Pat(_), inner, _) = &items[0] else { panic!("not a pat") };
        let Expr::Measure(nodes, _) = &inner[0] else {
            panic!("pow should be desugared inside pat: {:?}", inner[0])
        };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(
            c.to_chord().unwrap().semitones.into_iter().collect::<Vec<_>>(),
            vec![0, 7]
        );
    }

    #[test]
    fn test_oct_becomes_root_only() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;

        fn semitones(code: &str) -> Vec<u8> {
            let ast = RechordImporter.import(code).unwrap().resolve();
            let Expr::Seq(items) = &ast else { panic!("not a score") };
            let Expr::Measure(nodes, _) = &items[0] else {
                panic!("oct should be desugared into a measure: {:?}", items[0])
            };
            let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
            c.to_chord().unwrap().semitones.into_iter().collect()
        }

        // ルート1音だけになる
        assert_eq!(semitones("oct(A)\n"), vec![0]);
        // 元の修飾を上書きする
        assert_eq!(semitones("oct(Am7)\n"), vec![0]);
        assert_eq!(semitones("oct(AM7)\n"), vec![0]);
    }

    #[test]
    fn test_oct_keeps_slash_bass() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;

        let ast = RechordImporter.import("oct(A/E)\n").unwrap().resolve();
        let Expr::Seq(items) = &ast else { panic!("not a score") };
        let Expr::Measure(nodes, _) = &items[0] else { panic!("not a measure") };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(c.on, Some(Key::Absolute(Pitch::E)));
    }

    #[test]
    fn test_oct_inside_pat_is_desugared() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Func, Node};

        let ast = RechordImporter
            .import("pat(\"oxox\", oct(A))\n")
            .unwrap()
            .resolve();
        let Expr::Seq(items) = &ast else { panic!("not a score") };
        let Expr::Apply(Func::Pat(_), inner, _) = &items[0] else { panic!("not a pat") };
        let Expr::Measure(nodes, _) = &inner[0] else {
            panic!("oct should be desugared inside pat: {:?}", inner[0])
        };
        let Node::Chord(c) = &nodes[0] else { panic!("not a chord") };
        assert_eq!(
            c.to_chord().unwrap().semitones.into_iter().collect::<Vec<_>>(),
            vec![0]
        );
    }
}
