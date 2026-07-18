//! RHTML(`rhtml5`)↔RCSS(`rcss3`)↔RReact(本クレート)の相互接続。
//! `dom_bridge`フィーチャでのみ有効になる(既定では無効、依存を
//! 必須にしないための設計——RReact単独でも従来通り使える)。
//!
//! 最小限のEnd-to-Endパイプラインを提供する:
//! 1. `rhtml5::parse_document`でHTML文字列をDOM木(`rhtml5::Document`)
//!    にパースする(呼び出し側の責務、本モジュールはDocumentを受け取る)。
//! 2. `ElementRef`が`rcss3::ElementLike`を実装するアダプタとして
//!    `rhtml5::Element`を包む(orphan ruleにより`rhtml5::Element`へ
//!    直接`rcss3::ElementLike`を実装できないため、本クレート内に
//!    ローカルなラッパー型を置く——`rcss3`のCLAUDE.mdが示す「利用側の
//!    クレートでアダプタを実装する」方針に沿う)。
//! 3. `render_to_vnode`で、DOM木を辿りながら各要素に`rcss3::compute_style`
//!    でスタイルを解決し、`style`属性としてVNodeの属性へマージする。
//!
//! 完全なブラウザパイプラインではない(レイアウト計算・実際のDOM
//! パッチ適用は次段階の課題として明記、`RReact`側CLAUDE.mdの
//! 「次にすべきこと」参照)。

use std::collections::{BTreeMap, HashMap, HashSet};

use rcss3::{compute_style, style_to_string, ElementLike, Rule};
use rhtml5::{Attribute, Document, Element, Node};

use crate::diff::{AttrsPatch, ChildPatch, Patch};
use crate::vnode::{VElement, VNode};

/// `rhtml5::Element`への参照を`rcss3::ElementLike`として扱うための
/// ローカルなラッパー(orphan ruleのため必要な薄いアダプタ)。
#[derive(Clone, Copy)]
pub struct ElementRef<'a>(pub &'a Element);

impl<'a> ElementLike for ElementRef<'a> {
    fn tag_name(&self) -> &str {
        &self.0.tag_name
    }

    fn classes(&self) -> Vec<&str> {
        self.0.attr("class").map(|c| c.split_whitespace().collect()).unwrap_or_default()
    }

    fn id(&self) -> Option<&str> {
        self.0.attr("id")
    }
}

/// `rhtml5::Document`全体を、`stylesheet`で解決したインラインstyleを
/// 埋め込んだ`VNode`列(documentのトップレベル子要素に対応)へ変換する。
/// コメントノード(`rhtml5::Node::Comment`)はVNodeに対応する型が
/// 無いため読み飛ばす(最小パイプラインの割り切り、次段階の課題)。
pub fn render_to_vnode(document: &Document, stylesheet: &[Rule]) -> Vec<VNode> {
    render_nodes(&document.children, &[], stylesheet)
}

fn render_nodes<'a>(nodes: &'a [Node], ancestors: &[&ElementRef<'a>], stylesheet: &[Rule]) -> Vec<VNode> {
    // 隣接兄弟結合子(`+`)のマッチングに使う、ここまで見た要素ノードの列
    // (テキスト/コメントノードは兄弟結合子の判定対象外、実DOMの
    // セマンティクスと同じ)。
    let mut preceding_elements: Vec<ElementRef<'a>> = Vec::new();
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        let preceding_siblings: Vec<&ElementRef> = preceding_elements.iter().rev().collect();
        if let Some(vnode) = render_node(node, ancestors, &preceding_siblings, stylesheet) {
            out.push(vnode);
        }
        if let Node::Element(el) = node {
            preceding_elements.push(ElementRef(el));
        }
    }
    out
}

fn render_node(node: &Node, ancestors: &[&ElementRef], preceding_siblings: &[&ElementRef], stylesheet: &[Rule]) -> Option<VNode> {
    match node {
        Node::Text(text) => Some(VNode::Text(text.clone())),
        Node::Comment(_) => None,
        Node::Element(el) => {
            let el_ref = ElementRef(el);
            let computed = compute_style(stylesheet, &el_ref, ancestors, preceding_siblings);

            let mut attrs: BTreeMap<String, String> =
                el.attrs.iter().map(|a| (a.name.clone(), a.value.clone())).collect();
            if !computed.is_empty() {
                attrs.insert("style".to_string(), style_to_string(&computed));
            }

            let mut child_ancestors: Vec<&ElementRef> = Vec::with_capacity(ancestors.len() + 1);
            child_ancestors.push(&el_ref);
            child_ancestors.extend_from_slice(ancestors);
            let children = render_nodes(&el.children, &child_ancestors, stylesheet);

            Some(VNode::Element(VElement { tag: el.tag_name.clone(), attrs, key: None, children }))
        }
    }
}

// --- `Patch`の実DOM(`rhtml5::Node`)への適用(2026-07-19新規) ---
//
// この生態系には`web-sys`/`wasm-bindgen`もブラウザDOMも存在しない
// (`Cargo.toml`確認済み、依存はゼロ)。本クレートおよびRHTML/RCSSは
// 「一からのRust実装」であり、`RReact`にとっての「実DOM」とは
// `rhtml5::Node`/`Element`木のことを指す(`RFrontEnd`側CLAUDE.mdの
// 「お引越し可能な設計判断」節に「`Patch`ベースの差分適用……
// `RHTML::Node`への実適用に変換するアダプタ」と明記されている通り)。
// よって本節は`diff::Patch`を`rhtml5::Node`木へ反映する処理を提供する
// (ブラウザや`wasm-bindgen-test`は一切不要、`cargo test`の通常の
// 単体テストで実DOM相当の木を直接検証できる)。
//
// # 対応関係(ノードの特定方法)
// `render_to_vnode`は`rhtml5::Node::Comment`を読み飛ばすため、
// コメントノードを含まない文書であれば、ある`Node`木から
// `render_to_vnode`相当の変換で得た`VNode`木は**兄弟インデックスが
// 1対1で対応する**(挿入・削除・置換もすべて`diff`が返す
// インデックスに従って同じ`rhtml5::Node`の子リストへそのまま反映
// できる)。コメントを含む文書ではこの対応が崩れる、という制約は
// `render_to_vnode`のコメント読み飛ばし方針に由来する既存の限界の
// 延長として明記しておく(次段階の課題)。
//
// # 子要素差分の適用アルゴリズム
// `diff::diff_children`は「変化が無く、かつold_index==new_indexの
// 位置」については`ChildPatch`を一切出力しない、という重要な性質を
// 持つ(`diff.rs`のコメント・実装を参照)。つまり`ChildPatch`列に
// 現れない新しい位置`new_index`は、必ず「同じ`new_index`位置に
// あった古い子がそのまま変化していない」ことを意味する。この不変条件
// を使うことで、位置ごとの由来(挿入/更新/そのまま)を1回のパスで
// 決定でき、要素の移動を伴う複雑な配列操作を避けられる。
pub fn apply_patch(node: &mut Node, patch: &Patch) {
    match patch {
        Patch::NoOp => {}
        Patch::Replace(new_vnode) => *node = vnode_to_node(new_vnode),
        Patch::UpdateText(text) => {
            if let Node::Text(current) = node {
                *current = text.clone();
            }
            // `node`がTextでない場合は、`patch`が対応していない木から
            // 計算されたことを意味する(呼び出し側の前提違反)。
            // パニックさせず黙って無視する(第一段の割り切り)。
        }
        Patch::UpdateElement { attrs, children } => {
            if let Node::Element(el) = node {
                if let Some(attrs_patch) = attrs {
                    apply_attrs_patch(&mut el.attrs, attrs_patch);
                }
                if let Some(children_patches) = children {
                    apply_children_patches(&mut el.children, children_patches);
                }
            }
        }
    }
}

/// `VNode`を新規の`rhtml5::Node`へ変換する(`Patch::Replace`・
/// `ChildPatch::Insert`で新しく実DOM側に現れるノードの生成に使う、
/// `render_node`の逆方向の変換に相当)。
fn vnode_to_node(vnode: &VNode) -> Node {
    match vnode {
        VNode::Text(text) => Node::Text(text.clone()),
        VNode::Element(el) => Node::Element(Element {
            tag_name: el.tag.clone(),
            attrs: el.attrs.iter().map(|(name, value)| Attribute { name: name.clone(), value: value.clone() }).collect(),
            children: el.children.iter().map(vnode_to_node).collect(),
        }),
    }
}

fn apply_attrs_patch(attrs: &mut Vec<Attribute>, patch: &AttrsPatch) {
    for (name, value) in &patch.set {
        if let Some(existing) = attrs.iter_mut().find(|a| &a.name == name) {
            existing.value = value.clone();
        } else {
            attrs.push(Attribute { name: name.clone(), value: value.clone() });
        }
    }
    if !patch.remove.is_empty() {
        attrs.retain(|a| !patch.remove.contains(&a.name));
    }
}

fn apply_children_patches(children: &mut Vec<Node>, patches: &[ChildPatch]) {
    // 古い子リストのスナップショット(`Update`の`old_index`・
    // 「パッチに現れない位置はold_index==new_indexでそのまま」という
    // 不変条件の両方を、このスナップショットに対して評価する)。
    let old_snapshot = children.clone();

    let mut inserts: HashMap<usize, &VNode> = HashMap::new();
    let mut updates: HashMap<usize, (usize, &Patch)> = HashMap::new();
    let mut removed_old_indices: HashSet<usize> = HashSet::new();

    for patch in patches {
        match patch {
            ChildPatch::Insert { index, node } => {
                inserts.insert(*index, node);
            }
            ChildPatch::Remove { index } => {
                removed_old_indices.insert(*index);
            }
            ChildPatch::Update { old_index, new_index, patch } => {
                updates.insert(*new_index, (*old_index, patch.as_ref()));
            }
        }
    }

    let final_len = old_snapshot.len() + inserts.len() - removed_old_indices.len();
    let mut result = Vec::with_capacity(final_len);

    for new_index in 0..final_len {
        if let Some(vnode) = inserts.get(&new_index) {
            result.push(vnode_to_node(vnode));
        } else if let Some((old_index, inner_patch)) = updates.get(&new_index) {
            let mut moved = old_snapshot[*old_index].clone();
            apply_patch(&mut moved, inner_patch);
            result.push(moved);
        } else {
            // パッチが無い位置は、上記の不変条件によりold_index==new_index
            // かつ変化していない子(そのまま複製すればよい)。
            result.push(old_snapshot[new_index].clone());
        }
    }

    *children = result;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{diff, ChildPatch, Patch};
    use rcss3::parse_stylesheet;
    use rhtml5::parse_document;

    #[test]
    fn simple_element_gets_computed_style_merged_into_attrs() {
        let doc = parse_document(r#"<p class="foo">hi</p>"#);
        let stylesheet = parse_stylesheet(".foo { color: red; }");
        let nodes = render_to_vnode(&doc, &stylesheet);

        assert_eq!(nodes.len(), 1);
        let VNode::Element(p) = &nodes[0] else { panic!("expected element") };
        assert_eq!(p.tag, "p");
        assert_eq!(p.attrs.get("class"), Some(&"foo".to_string()));
        assert_eq!(p.attrs.get("style"), Some(&"color: red;".to_string()));
        assert_eq!(p.children, vec![VNode::text("hi")]);
    }

    #[test]
    fn descendant_combinator_resolves_through_the_parsed_ancestor_chain() {
        let doc = parse_document(r#"<div><p>hi</p></div>"#);
        let stylesheet = parse_stylesheet("div p { color: green; }");
        let nodes = render_to_vnode(&doc, &stylesheet);

        let VNode::Element(div) = &nodes[0] else { panic!("expected element") };
        let VNode::Element(p) = &div.children[0] else { panic!("expected element") };
        assert_eq!(p.attrs.get("style"), Some(&"color: green;".to_string()));
    }

    #[test]
    fn non_matching_descendant_selector_does_not_add_a_style_attr() {
        let doc = parse_document(r#"<section><p>hi</p></section>"#);
        let stylesheet = parse_stylesheet("div p { color: green; }");
        let nodes = render_to_vnode(&doc, &stylesheet);

        let VNode::Element(section) = &nodes[0] else { panic!("expected element") };
        let VNode::Element(p) = &section.children[0] else { panic!("expected element") };
        assert_eq!(p.attrs.get("style"), None);
    }

    #[test]
    fn child_combinator_resolves_through_the_parsed_immediate_parent() {
        let doc = parse_document(r#"<div><section><p>hi</p></section></div>"#);
        // "div > p" のdivは直接の親(section)ではなく祖父母なので不一致、
        // "section > p" は直接の親なので一致するはず。
        let stylesheet = parse_stylesheet("div > p { color: red; } section > p { color: purple; }");
        let nodes = render_to_vnode(&doc, &stylesheet);

        let VNode::Element(div) = &nodes[0] else { panic!("expected element") };
        let VNode::Element(section) = &div.children[0] else { panic!("expected element") };
        let VNode::Element(p) = &section.children[0] else { panic!("expected element") };
        assert_eq!(p.attrs.get("style"), Some(&"color: purple;".to_string()));
    }

    #[test]
    fn adjacent_sibling_combinator_resolves_through_the_parsed_sibling_list() {
        let doc = parse_document(r#"<ul><li>a</li><li>b</li><li>c</li></ul>"#);
        let stylesheet = parse_stylesheet("li + li { color: orange; }");
        let nodes = render_to_vnode(&doc, &stylesheet);

        let VNode::Element(ul) = &nodes[0] else { panic!("expected element") };
        let VNode::Element(first) = &ul.children[0] else { panic!("expected element") };
        let VNode::Element(second) = &ul.children[1] else { panic!("expected element") };
        let VNode::Element(third) = &ul.children[2] else { panic!("expected element") };

        // 最初のliには直前の兄弟が無いので不一致。
        assert_eq!(first.attrs.get("style"), None);
        // 2番目・3番目はどちらも直前にliを持つので一致。
        assert_eq!(second.attrs.get("style"), Some(&"color: orange;".to_string()));
        assert_eq!(third.attrs.get("style"), Some(&"color: orange;".to_string()));
    }

    #[test]
    fn end_to_end_pipeline_feeds_into_vdom_diff() {
        // RHTMLでパース→RCSSでスタイル解決→RReactのVNode化、という
        // 最小のEnd-to-Endパイプラインで作った2つの木を、RReactの
        // diffにそのまま渡せることを確認する(相互統合の要)。
        let stylesheet = parse_stylesheet("p { color: red; }");

        let old_doc = parse_document(r#"<div><p>hi</p></div>"#);
        let old_nodes = render_to_vnode(&old_doc, &stylesheet);

        let new_doc = parse_document(r#"<div><p>bye</p></div>"#);
        let new_nodes = render_to_vnode(&new_doc, &stylesheet);

        // トップレベル(div)の差分は、子要素(p)のin-place更新1件。
        let patch = diff(&old_nodes[0], &new_nodes[0]);
        let Patch::UpdateElement { attrs, children: Some(children) } = patch else {
            panic!("expected an element update patch with child changes")
        };
        assert!(attrs.is_none(), "style/class attrs are identical, only the text child changed");
        assert_eq!(children.len(), 1);
        let ChildPatch::Update { patch: p_patch, .. } = &children[0] else { panic!("expected an update patch") };

        // pの差分は、テキスト子("hi"→"bye")のin-place更新1件。
        let Patch::UpdateElement { attrs: p_attrs, children: Some(p_children) } = p_patch.as_ref() else {
            panic!("expected p's own element update patch")
        };
        assert!(p_attrs.is_none());
        assert_eq!(p_children.len(), 1);
        let ChildPatch::Update { patch: text_patch, .. } = &p_children[0] else { panic!("expected an update patch") };
        assert_eq!(**text_patch, Patch::UpdateText("bye".to_string()));
    }

    // --- `apply_patch`(`Patch`の実DOM(`rhtml5::Node`)への適用)のテスト ---
    // ここでの「実DOM」は`web-sys`のブラウザDOMではなく、本生態系に
    // おける実DOM相当の`rhtml5::Node`木そのもの(依存ゼロで存在する
    // ため、通常の`cargo test`で直接検証できる——`wasm-bindgen-test`や
    // ヘッドレスブラウザは不要)。

    /// RHTMLでパースした文書の最初のトップレベル子(`Node`)を、対応する
    /// `VNode`(`render_to_vnode`の出力)と一緒に返す小さなヘルパー
    /// (「実DOM」と「そこから作った仮想DOM」の両方を同じHTMLから
    /// 用意し、以後は仮想DOM側だけを新しい木にdiffして実DOM側へ
    /// `apply_patch`する、という実際の使い方をそのまま再現する)。
    fn real_and_virtual(html: &str) -> (Node, VNode) {
        let doc = parse_document(html);
        let vnodes = render_to_vnode(&doc, &[]);
        (doc.children.into_iter().next().unwrap(), vnodes.into_iter().next().unwrap())
    }

    #[test]
    fn apply_patch_updates_text_in_place() {
        let (mut real, old_vnode) = real_and_virtual("<p>hi</p>");
        let new_vnode = VNode::element("p").child(VNode::text("bye")).build();

        // pそのものの差分は子(テキスト)のUpdate、その中身がUpdateText。
        let Patch::UpdateElement { children: Some(children), .. } = diff(&old_vnode, &new_vnode) else {
            panic!("expected element update")
        };
        let ChildPatch::Update { patch: text_patch, .. } = &children[0] else { panic!("expected update") };

        let Node::Element(el) = &mut real else { panic!("expected element") };
        apply_patch(&mut el.children[0], text_patch);

        assert_eq!(el.children[0], Node::Text("bye".to_string()));
    }

    #[test]
    fn apply_patch_sets_and_removes_attributes() {
        let (mut real, old_vnode) = real_and_virtual(r#"<div class="a" disabled></div>"#);
        let new_vnode = VNode::element("div").attr("class", "b").build();

        let patch = diff(&old_vnode, &new_vnode);
        apply_patch(&mut real, &patch);

        let Node::Element(el) = &real else { panic!("expected element") };
        assert_eq!(el.attr("class"), Some("b"));
        assert_eq!(el.attr("disabled"), None);
    }

    #[test]
    fn apply_patch_inserts_and_removes_children() {
        let (mut real, old_vnode) = real_and_virtual("<ul><li>a</li><li>b</li></ul>");
        let new_vnode = VNode::element("ul")
            .child(VNode::element("li").child(VNode::text("a")).build())
            .child(VNode::element("li").child(VNode::text("c")).build())
            .build();

        let patch = diff(&old_vnode, &new_vnode);
        apply_patch(&mut real, &patch);

        let Node::Element(ul) = &real else { panic!("expected element") };
        assert_eq!(ul.children.len(), 2);
        let Node::Element(second) = &ul.children[1] else { panic!("expected element") };
        assert_eq!(second.children, vec![Node::Text("c".to_string())]);
    }

    #[test]
    fn apply_patch_reorders_keyed_children_and_preserves_untouched_ones() {
        let old_vnode = VNode::element("ul")
            .child(VNode::element("li").key("a").child(VNode::text("A")).build())
            .child(VNode::element("li").key("b").child(VNode::text("B")).build())
            .child(VNode::element("li").key("c").child(VNode::text("C")).build())
            .build();
        let mut real = vnode_to_node(&old_vnode);

        // b, a, c(cはそのまま=old_index==new_index==2、パッチには現れない)。
        let new_vnode = VNode::element("ul")
            .child(VNode::element("li").key("b").child(VNode::text("B")).build())
            .child(VNode::element("li").key("a").child(VNode::text("A")).build())
            .child(VNode::element("li").key("c").child(VNode::text("C")).build())
            .build();

        let patch = diff(&old_vnode, &new_vnode);
        apply_patch(&mut real, &patch);

        let Node::Element(ul) = &real else { panic!("expected element") };
        assert_eq!(ul.children.len(), 3);
        let texts: Vec<&str> = ul
            .children
            .iter()
            .map(|n| {
                let Node::Element(li) = n else { panic!("expected li") };
                let Node::Text(t) = &li.children[0] else { panic!("expected text") };
                t.as_str()
            })
            .collect();
        assert_eq!(texts, vec!["B", "A", "C"]);
    }

    #[test]
    fn apply_patch_replaces_node_when_tag_changes() {
        let old_vnode = VNode::element("div").build();
        let mut real = vnode_to_node(&old_vnode);
        let new_vnode = VNode::element("span").attr("class", "x").build();

        let patch = diff(&old_vnode, &new_vnode);
        apply_patch(&mut real, &patch);

        let Node::Element(el) = &real else { panic!("expected element") };
        assert_eq!(el.tag_name, "span");
        assert_eq!(el.attr("class"), Some("x"));
    }

    #[test]
    fn apply_patch_no_op_leaves_real_dom_untouched() {
        let vnode = VNode::element("div").attr("class", "a").child(VNode::text("hi")).build();
        let mut real = vnode_to_node(&vnode);
        let untouched = real.clone();

        let patch = diff(&vnode, &vnode.clone());
        assert_eq!(patch, Patch::NoOp);
        apply_patch(&mut real, &patch);

        assert_eq!(real, untouched);
    }
}
