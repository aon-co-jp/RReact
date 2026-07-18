# 開発方針＆開発環境ルール(RReact)

作業ドライブは`F:\open-runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。

## このプロジェクトの構想(2026-07-18新設)

React(React DOM)・React Native・React Mobileのコンポーネントモデルを、
既存のReact/React DOM/React Nativeのコードを一切流用せず一から
Rust + Poemで再現するプロジェクト。`RHTML5`/`RCSS3`/`RTypeScript`/
`RBootStrap`構想とは別の並行構想(ユーザー指示、2026-07-17: 「別
プロジェクトで並行開発」)。

## 現状(第一段、2026-07-18)

- `src/vnode.rs`: `VNode`(`Element`/`Text`)、`VElement`
  (`tag`/`attrs`/`key`/`children`)、`VElementBuilder`
  (`React.createElement`相当のビルダーAPI)。
- `src/diff.rs`: 仮想DOM差分計算(reconciliation)。
  - `Patch`列挙型: `Replace`・`UpdateText`・`UpdateElement{attrs,
    children}`(属性差分・子差分のどちらか一方または両方をまとめて
    運ぶ、複合ケースを持つ専用バリアント)・`NoOp`。
  - 子要素差分は`key`があればkeyで同一性を追跡し、無ければ位置
    (index)で対応させる2段構え。Reactの実装が使う最長増加部分列
    (LIS)ベースの最小移動数計算までは再現しない(正しさは保つが
    移動が最適とは限らない、という第一段の割り切り)。
- **`src/dom_bridge.rs`(2026-07-18新規、`dom_bridge`フィーチャで
  のみ有効)**: RHTML(`rhtml5`)↔RCSS(`rcss3`)↔RReact(本クレート)を
  つなぐ最小のEnd-to-Endパイプライン。`ElementRef`が`rhtml5::Element`を
  包んで`rcss3::ElementLike`を実装するアダプタ(orphan ruleのため
  `rhtml5::Element`に直接実装できず、利用側の本クレートでラッパー型を
  用意した)。`render_to_vnode(document, stylesheet)`が、RHTMLでパース
  したDOM木を辿りながら各要素にRCSSの`compute_style`(祖先チェーンを
  渡すことで子孫結合子にも対応)でスタイルを解決し、`style`属性として
  `VElement::attrs`へマージしつつ`VNode`木を組み立てる。Cargo.tomlの
  `rhtml5`/`rcss3`依存はどちらも`optional = true`(既定では無効、
  本クレート単独でも従来通り使える設計)。
- **未対応(次段階)**: コンポーネントモデル(関数コンポーネント・
  hooks相当の状態管理)、`Patch`の実DOM(`rhtml5`)への適用(現状は
  「差分無しの初回描画」に相当する`render_to_vnode`のみ、`diff`の
  結果を`rhtml5::Node`へ反映する処理はまだ無い)、React Native/React
  Mobile相当の非HTMLターゲットへのレンダリング、Fiberのような
  中断可能なレンダリング・優先度スケジューリング。
- **検証**: `cargo test`で10件全green(VNodeビルダー1件+差分計算9件)。
  `cargo test --features dom_bridge`で14件全green(上記10件+
  `dom_bridge`4件: 単純要素へのスタイルマージ・子孫結合子解決・
  非マッチ時にstyle属性を付けないこと・RHTML→RCSS→RReactの
  End-to-Endパイプラインで作った2つの木をdiffに渡せることの確認)。
  警告0件。

## 次にすべきこと

1. `Patch`の実DOM(`rhtml5::Node`)への適用(`dom_bridge`は「初回描画」
   までなので、2回目以降の差分反映がまだ無い)
2. コンポーネントモデル第一段(状態を持たない純粋関数コンポーネントの
   みからスタート、hooks相当の状態管理は次々段階)
3. 子要素差分の最小移動数計算への改善(現状は正しいが最適とは限らない)

## 関連プロジェクト

- [rhtml5](https://github.com/aon-co-jp/rhtml5) / [rcss3](https://github.com/aon-co-jp/rcss3) — `dom_bridge`フィーチャで相互接続済み(2026-07-18、詳細は上記「現状」参照)
- [open-raid-z](https://github.com/aon-co-jp/open-raid-z) — 開発ルールの正本

## HANDOFF

- **2026-07-18 RHTML↔RCSS↔RReact相互統合(`dom_bridge`フィーチャ)**:
  3つとも独立実装のまま繋がっていなかった状態から、最小のEnd-to-End
  パイプラインを実装。`rhtml5`/`rcss3`をoptional path依存として追加
  (`dom_bridge`フィーチャで有効化、既定では従来通り無依存)。
  `ElementRef`アダプタ(`rcss3::ElementLike`実装)と`render_to_vnode`
  (DOM木→スタイル解決→VNode木)を新規実装、RCSS側で同日追加した
  子孫結合子(`div p`)対応も実地で確認した。テストは10件→
  (フィーチャ有効時)14件、全green・警告0件。
  次にすべきこと: `Patch`の実DOM反映(2回目以降の差分適用)、
  コンポーネントモデル第一段。
