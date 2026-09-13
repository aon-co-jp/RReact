# 開発方針＆開発環境ルール(RS-React)

作業ドライブは`F:\runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。

## リポジトリ改称(2026-09-13)

`RReact`→`RS-React`へGitHub上でrename済み(crate名も`rreact`→
`rs-react`へ改称)。`aruaru.pro`(コンコナラ型スキルマーケットプレイス+
求人サイト統合の新規プロジェクト)向けのフロント基盤整備に合わせた、
`RFrontEnd`傘下プロジェクトのネーミング統一の一環(ユーザー指示、
2026-09-13:「RReact自体をRS-Reactへ改称」。ネーミングルールはまだ
全体で統一されておらず、その都度指示する方針)。以下の記述内の
`RReact`表記は改称前の履歴として残す。

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
  `cargo test --features dom_bridge`で16件全green(上記10件+
  `dom_bridge`6件: 単純要素へのスタイルマージ・子孫結合子解決・
  非マッチ時にstyle属性を付けないこと・RHTML→RCSS→RReactの
  End-to-Endパイプラインで作った2つの木をdiffに渡せることの確認・
  **2026-07-18追加**: 子結合子(`>`)が直接の親のみに一致し祖父母には
  一致しないことの実地確認、隣接兄弟結合子(`+`)が実際にパースした
  `<ul><li>`列の中で直前の兄弟の有無を正しく判定することの確認)。
  警告0件。
- **2026-07-18: RCSS側のSelector型変更に追従**: RCSSが`Selector`型を
  `Vec<CompoundSelector>`から`Vec<SelectorSegment>`へ変更(子/隣接兄弟
  結合子対応)したのに伴い、`render_nodes`/`render_node`が
  `preceding_siblings`(直前の兄弟から順に並べた配列、テキスト/
  コメントノードは兄弟結合子の判定対象外)を追跡・伝播するよう変更。
  `ElementRef`に`Clone, Copy`を追加(兄弟列に値として保持するため)。

## 次にすべきこと

1. 子要素差分の最小移動数計算への改善(現状は正しいが最適とは限らない)
2. コンポーネントモデル第二段(`use_effect`相当・複数コンポーネントを
   束ねたツリー全体の再レンダーループ——現状の`hooks`モジュールは
   フック状態の管理のみで、ツリー走査してdirtyなインスタンスだけ
   再render→diff→`apply_patch`する「アプリループ」自体はまだ無い)

## 関連プロジェクト

- [rhtml5](https://github.com/aon-co-jp/rhtml5) / [rcss3](https://github.com/aon-co-jp/rcss3) — `dom_bridge`フィーチャで相互接続済み(2026-07-18、詳細は上記「現状」参照)
- [open-raid-z](https://github.com/aon-co-jp/open-raid-z) — 開発ルールの正本

## HANDOFF

- **2026-09-13 コンポーネントモデル第一段(関数コンポーネント+
  `use_state`)実装**: `src/hooks.rs`新設。呼び出し側が割り当てる
  `ComponentId`(ツリー上の位置や`key`から決定する想定)にフック状態を
  紐づけ、`run_component(id, render_fn)`でrender関数呼び出し前後の
  フックカーソルのリセットを行う設計(Reactの「フックは呼び出し順序が
  一定」というルールをそのまま踏襲)。`use_state`は初回のみ`init`を
  評価し、以降は保存済みの値を返す。`StateSetter::set`/`update`で
  即時に状態を書き換え、対応する`ComponentId`を`take_dirty`で
  観測可能にする(バッチングはしない素朴な同期反映)。テスト6件追加、
  `cargo test`(既定)16件・`cargo test --features dom_bridge`28件
  すべてgreen・警告0件。**未着手**: 複数コンポーネントを束ねた
  ツリー全体の再レンダーループ(dirtyなインスタンスだけ再render→
  diff→`apply_patch`する「アプリループ」自体)、`use_effect`相当。
  aruaru.pro(新規マーケットプレイスプロジェクト)のフロント基盤として
  必要になり着手。

- **2026-09-13 食い違いを裏取り済み**: 前回HANDOFFに記載されていた
  「`Patch`の実DOM反映が無い」という記載と、親リポジトリ`RFrontEnd`側
  CLAUDE.mdの「`dom_bridge::apply_patch`実装済み」という記載の食い違いを
  `cargo test --features dom_bridge`で実地確認。**実装済みが正しい**
  (`apply_patch`関数が存在し、22テスト全green——挿入/削除/置換/属性
  更新/テキスト更新/keyed子要素の並べ替えを含む)。本ファイルの
  「次にすべきこと」から該当項目を削除済み。次はコンポーネントモデル
  第一段(aruaru.pro新規マーケットプレイスプロジェクトのフロント基盤
  として必要になったため着手)。

- **2026-07-19 audiocafe-tokyo-rustユーザーからの完成度向上要望を記録**:
  `audiocafe-tokyo-rust`のユーザーから「未着手や未完成の技術があれば、
  それぞれのリポジトリもTESTしながら完成させていって、実用性と
  完成度を高めていって下さい」という要望があった。

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
