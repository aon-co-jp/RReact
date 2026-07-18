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
- **未対応(次段階)**: コンポーネントモデル(関数コンポーネント・
  hooks相当の状態管理)、実DOM(`rhtml5`)へのパッチ適用、
  React Native/React Mobile相当の非HTMLターゲットへのレンダリング、
  Fiberのような中断可能なレンダリング・優先度スケジューリング。
- **検証**: `cargo test`で10件全green(VNodeビルダー1件+差分計算9件、
  同一木のNoOp・テキスト変更・タグ変更によるReplace・属性追加/削除・
  子要素の挿入/削除・属性と子要素の同時変化・keyによる並び替え追跡を
  含む)。警告0件。

## 次にすべきこと

1. `rhtml5::Element`/`Document`への`Patch`適用(実DOM反映、または
   SSR用に「差分無しの初回描画」だけでも`VNode`→`rhtml5::Node`への
   変換関数)
2. コンポーネントモデル第一段(状態を持たない純粋関数コンポーネントの
   みからスタート、hooks相当の状態管理は次々段階)
3. 子要素差分の最小移動数計算への改善(現状は正しいが最適とは限らない)

## 関連プロジェクト

- [rhtml5](https://github.com/aon-co-jp/rhtml5) / [rcss3](https://github.com/aon-co-jp/rcss3) — 別の並行構想(SSR用DOM/CSS)、将来的に連携する可能性がある
- [open-raid-z](https://github.com/aon-co-jp/open-raid-z) — 開発ルールの正本
