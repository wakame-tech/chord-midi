use crate::{
    model::ast::{ChordNode, Expr, Func, Node},
    model::modifier::Modifier,
    model::pitch::Pitch,
    model::scale::Scale,
};
use anyhow::Result;

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
            Expr::Apply(func, inner, br) => {
                Expr::Apply(func, Box::new(inner.into_degree(tonic, scale)), br)
            }
            // 定義の本体も同じスコープで変換する。参照は名前だけなので触らない
            Expr::Define(name, body) => {
                Expr::Define(name, Box::new(body.into_degree(tonic, scale)))
            }
            // リピートは小節の並べ方の話。中身のコードは同じスコープで変換する
            Expr::Repeat(inner, n, br) => {
                Expr::Repeat(Box::new(inner.into_degree(tonic, scale)), n, br)
            },
            // into_degree は into_pitch の逆変換なので、包みは消さずに残す
            Expr::Keyed(p, inner) => Expr::Keyed(p, Box::new(inner.into_degree(p, scale))),
            Expr::Scaled(s, inner) => Expr::Scaled(s, Box::new(inner.into_degree(tonic, s))),
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
            Expr::Apply(func, inner, br) => {
                Expr::Apply(func, Box::new(inner.into_pitch(tonic, scale)), br)
            }
            // 定義の本体も同じスコープで変換する。参照は名前だけなので触らない
            Expr::Define(name, body) => {
                Expr::Define(name, Box::new(body.into_pitch(tonic, scale)))
            }
            // リピートは小節の並べ方の話。中身のコードは同じスコープで変換する
            Expr::Repeat(inner, n, br) => {
                Expr::Repeat(Box::new(inner.into_pitch(tonic, scale)), n, br)
            },
            // key: は以降のスコープ。中身を新しい tonic で解決し、包みは消す
            Expr::Keyed(p, inner) => inner.into_pitch(p, scale),
            Expr::Scaled(s, inner) => inner.into_pitch(tonic, s),
            other => other,
        }
    }

    /// 定義を展開し、key/scale のスコープを解いて度数を実音にし、
    /// Comment を落とした再生可能な AST を返す。Transform は MIDI 解釈時に展開する。
    ///
    /// `key:` が無いときの tonic は C。度数表記の進行 (プリセット等) が
    /// 常に実音に解決されて鳴るようにするため。
    ///
    /// 展開に失敗する (未定義の参照、循環参照、小節数の上限超過など) と `Err`。
    pub fn resolve(self) -> Result<Expr> {
        // 参照が残ったまま先へ流れないよう、いちばん先に展開する
        let expanded = crate::model::expand::expand(self)?;
        let transposed = expanded.into_pitch(Pitch::C, Scale::Major);
        // pow は音程の話なのでここで潰す。pat はリズムの話なので残す
        let desugared = desugar_shapes(transposed)
            .into_iter()
            .next()
            .expect("desugar_shapes always yields at least one node for a Seq");
        Ok(strip_comments(desugared))
    }
}

/// Comment を落とす。`Keyed`/`Scaled` は into_pitch が解いて消えている。
///
/// 包みが消えた跡に残る入れ子の `Seq` は親へ畳む。resolve 後の形を
/// 「小節が平らに並んだ Seq」に保つため (`Apply` の中身の `Seq` は畳まない)
fn strip_comments(expr: Expr) -> Expr {
    match expr {
        Expr::Seq(items) => Expr::Seq(
            items
                .into_iter()
                .filter(|it| !matches!(it, Expr::Comment(_)))
                .flat_map(|it| match strip_comments(it) {
                    Expr::Seq(inner) => inner,
                    other => vec![other],
                })
                .collect(),
        ),
        Expr::Apply(func, inner, br) => Expr::Apply(func, Box::new(strip_comments(*inner)), br),
        other => other,
    }
}

/// `Seq` を親のリストへ差し込む。`Apply` の中身は常に `Seq` なので、
/// `pow(C | E D)` を潰したときに `Seq` が入れ子で残らないようにする
fn splice(expr: Expr) -> Vec<Expr> {
    match expr {
        Expr::Seq(items) => items,
        other => vec![other],
    }
}

/// `Apply(Func::Pow, ..)` と `Apply(Func::Oct, ..)` を潰し、
/// 中の全コードに対応する修飾を足す。
///
/// `Apply` は親のリストの中で複数の要素に展開されるので `Vec` を返す。
fn desugar_shapes(ast: Expr) -> Vec<Expr> {
    match ast {
        Expr::Seq(items) => vec![Expr::Seq(
            items.into_iter().flat_map(desugar_shapes).collect(),
        )],
        Expr::Apply(Func::Pow, inner, _) => desugar_shapes(*inner)
            .into_iter()
            .flat_map(splice)
            .map(|item| mark(item, Modifier::Power))
            .collect(),
        Expr::Apply(Func::Oct, inner, _) => desugar_shapes(*inner)
            .into_iter()
            .flat_map(splice)
            .map(|item| mark(item, Modifier::Octave))
            .collect(),
        Expr::Apply(func, inner, br) => {
            let inner = Expr::Seq(
                desugar_shapes(*inner)
                    .into_iter()
                    .flat_map(splice)
                    .collect(),
            );
            vec![Expr::Apply(func, Box::new(inner), br)]
        }
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
        // `Apply` の中身は常に `Seq` なので、その中の各要素へ潜って印を付け直す
        Expr::Seq(items) => Expr::Seq(
            items
                .into_iter()
                .map(|item| mark(item, modifier.clone()))
                .collect(),
        ),
        Expr::Apply(func, inner, br) => Expr::Apply(func, Box::new(mark(*inner, modifier)), br),
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
        ast::{ChordNode, Expr, Node},
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
    fn test_resolve_expands_refs() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;
        let ast = RechordImporter
            .import("$riff = C | E |\n$riff | D |\n")
            .unwrap()
            .resolve()
            .unwrap();
        fn count_measures(e: &Expr) -> usize {
            match e {
                Expr::Measure(_, _) => 1,
                Expr::Seq(items) => items.iter().map(count_measures).sum(),
                Expr::Apply(_, inner, _) => count_measures(inner),
                _ => 0,
            }
        }
        assert_eq!(count_measures(&ast), 3);
    }

    #[test]
    fn test_resolve_reports_undefined_ref() {
        use crate::import::{Importer, RechordImporter};
        let e = RechordImporter.import("$nope | C |\n").unwrap().resolve();
        assert!(e.is_err());
    }

    #[test]
    fn test_resolve_transposes_degrees() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Node};
        let ast = RechordImporter.import("key: C\nV7 |\n").unwrap();
        let resolved = ast.resolve().unwrap();
        let Expr::Seq(items) = &resolved else {
            panic!()
        };
        let mut found = false;
        for it in items {
            // directives stripped: only Measure should remain
            assert!(!matches!(it, Expr::Keyed(_, _) | Expr::Scaled(_, _)));
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
    fn test_key_scope_applies_to_following_measures() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Expr, Node};
        use crate::model::key::Key;
        use crate::model::pitch::Pitch;

        fn first_pitches(expr: &Expr, out: &mut Vec<Pitch>) {
            match expr {
                Expr::Seq(items) => items.iter().for_each(|i| first_pitches(i, out)),
                Expr::Apply(_, inner, _) => first_pitches(inner, out),
                Expr::Keyed(_, inner) | Expr::Scaled(_, inner) => first_pitches(inner, out),
                Expr::Measure(nodes, _) => {
                    for n in nodes {
                        if let Node::Chord(c) = n {
                            if let Key::Absolute(p) = c.key {
                                out.push(p);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // 途中の key: がそこから後ろにだけ効く
        let ast = RechordImporter
            .import("key: C\nI |\nkey: D\nI |\n")
            .unwrap()
            .resolve()
            .unwrap();
        let mut got = Vec::new();
        first_pitches(&ast, &mut got);
        assert_eq!(got, vec![Pitch::C, Pitch::D]);
    }

    #[test]
    fn test_resolve_removes_keyed_wrappers() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Expr;

        fn has_wrapper(expr: &Expr) -> bool {
            match expr {
                Expr::Keyed(_, _) | Expr::Scaled(_, _) => true,
                Expr::Seq(items) => items.iter().any(has_wrapper),
                Expr::Apply(_, inner, _) => has_wrapper(inner),
                _ => false,
            }
        }

        let ast = RechordImporter
            .import("key: C\nI |\n")
            .unwrap()
            .resolve()
            .unwrap();
        assert!(!has_wrapper(&ast), "resolve 後に Keyed/Scaled が残っている");
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
            let ast = RechordImporter.import(code).unwrap().resolve().unwrap();
            let Expr::Seq(items) = &ast else {
                panic!("not a score")
            };
            let Expr::Measure(nodes, _) = &items[0] else {
                panic!("pow should be desugared into a measure: {:?}", items[0])
            };
            let Node::Chord(c) = &nodes[0] else {
                panic!("not a chord")
            };
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

        let ast = RechordImporter
            .import("pow(C/E)\n")
            .unwrap()
            .resolve()
            .unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        let Expr::Measure(nodes, _) = &items[0] else {
            panic!("not a measure")
        };
        let Node::Chord(c) = &nodes[0] else {
            panic!("not a chord")
        };
        assert_eq!(c.on, Some(Key::Absolute(Pitch::E)));
    }

    #[test]
    fn test_pow_flattens_all_measures() {
        use crate::import::{Importer, RechordImporter};

        let ast = RechordImporter
            .import("pow(C | E D)\n")
            .unwrap()
            .resolve()
            .unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        // Apply は消えて2つの Measure になる
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| matches!(i, Expr::Measure(_, _))));
    }

    #[test]
    fn test_pat_survives_resolve() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Func;

        let ast = RechordImporter
            .import("pat(\"oxox\", C)\n")
            .unwrap()
            .resolve()
            .unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
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
            .resolve()
            .unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        let Expr::Apply(Func::Pat(_), inner, _) = &items[0] else {
            panic!("not a pat")
        };
        let Expr::Seq(inner) = inner.as_ref() else {
            panic!("apply の中身は Seq")
        };
        let Expr::Measure(nodes, _) = &inner[0] else {
            panic!("pow should be desugared inside pat: {:?}", inner[0])
        };
        let Node::Chord(c) = &nodes[0] else {
            panic!("not a chord")
        };
        assert_eq!(
            c.to_chord()
                .unwrap()
                .semitones
                .into_iter()
                .collect::<Vec<_>>(),
            vec![0, 7]
        );
    }

    #[test]
    fn test_pat_inside_pow_is_marked() {
        // pow が外側にあるとき、内側の pat が包む Seq の向こうまで
        // Power 修飾が届くことを確認する (mark の Seq 対応の回帰テスト)
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Func, Node};

        let ast = RechordImporter
            .import("pow(pat(\"oxox\", C))\n")
            .unwrap()
            .resolve()
            .unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        let Expr::Apply(Func::Pat(_), inner, _) = &items[0] else {
            panic!("not a pat")
        };
        let Expr::Seq(inner) = inner.as_ref() else {
            panic!("apply の中身は Seq")
        };
        let Expr::Measure(nodes, _) = &inner[0] else {
            panic!("not a measure: {:?}", inner[0])
        };
        let Node::Chord(c) = &nodes[0] else {
            panic!("not a chord")
        };
        assert_eq!(
            c.to_chord()
                .unwrap()
                .semitones
                .into_iter()
                .collect::<Vec<_>>(),
            vec![0, 7]
        );
    }

    #[test]
    fn test_oct_becomes_root_only() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::Node;

        fn semitones(code: &str) -> Vec<u8> {
            let ast = RechordImporter.import(code).unwrap().resolve().unwrap();
            let Expr::Seq(items) = &ast else {
                panic!("not a score")
            };
            let Expr::Measure(nodes, _) = &items[0] else {
                panic!("oct should be desugared into a measure: {:?}", items[0])
            };
            let Node::Chord(c) = &nodes[0] else {
                panic!("not a chord")
            };
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

        let ast = RechordImporter
            .import("oct(A/E)\n")
            .unwrap()
            .resolve()
            .unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        let Expr::Measure(nodes, _) = &items[0] else {
            panic!("not a measure")
        };
        let Node::Chord(c) = &nodes[0] else {
            panic!("not a chord")
        };
        assert_eq!(c.on, Some(Key::Absolute(Pitch::E)));
    }

    #[test]
    fn test_oct_inside_pat_is_desugared() {
        use crate::import::{Importer, RechordImporter};
        use crate::model::ast::{Func, Node};

        let ast = RechordImporter
            .import("pat(\"oxox\", oct(A))\n")
            .unwrap()
            .resolve()
            .unwrap();
        let Expr::Seq(items) = &ast else {
            panic!("not a score")
        };
        let Expr::Apply(Func::Pat(_), inner, _) = &items[0] else {
            panic!("not a pat")
        };
        let Expr::Seq(inner) = inner.as_ref() else {
            panic!("apply の中身は Seq")
        };
        let Expr::Measure(nodes, _) = &inner[0] else {
            panic!("oct should be desugared inside pat: {:?}", inner[0])
        };
        let Node::Chord(c) = &nodes[0] else {
            panic!("not a chord")
        };
        assert_eq!(
            c.to_chord()
                .unwrap()
                .semitones
                .into_iter()
                .collect::<Vec<_>>(),
            vec![0]
        );
    }
}
