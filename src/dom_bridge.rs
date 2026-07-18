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

use std::collections::BTreeMap;

use rcss3::{compute_style, style_to_string, ElementLike, Rule};
use rhtml5::{Document, Element, Node};

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
}
