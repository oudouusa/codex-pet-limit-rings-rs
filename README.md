# codex-pet-limit-rings-rs

Codex Pet Limit Rings を **Windows向けにRustで作り直した** 軽量な常駐アプリです。Codex pet のまわりに利用上限の残量リングを重ねて表示します。

このリポジトリは、MITライセンスの [`petergpt/codex-pet-limit-rings`](https://github.com/petergpt/codex-pet-limit-rings) を参考にしています。参考元の「Codex本体を改造せず、外部コンパニオンアプリとしてpetに寄り添う」という設計思想を引き継ぎつつ、Windows専用のRust/Win32実装として再構築しています。

![Codex Pet Limit Rings around a Codex pet](docs/assets/codex-pet-limit-rings-screenshot.png)

## 特徴

- **Windows専用のネイティブRust実装**
  ElectronやWebViewを使わず、Win32 layered windowで透明オーバーレイを描画します。
- **軽量な常駐プロセス**
  アイドル時は描画更新を抑え、定期的にworking setをトリムします。ローカル検証ではアイドル時のworking setは約2-3MBでした。
- **Codex本体を改造しない**
  Codexのアプリファイル、pet画像、asar、署名、設定をパッチしません。インストールもアンインストールも独立しています。
- **どのCodex petでも使える**
  petの種類や画像を見ず、Codexが表示しているpetウィンドウの位置を追跡します。
- **Windows通知領域から操作できる**
  表示/非表示、更新、位置の微調整、終了を通知領域アイコンから操作できます。
- **ライブ値とキャッシュ値に対応**
  ChatGPTの利用状況エンドポイントからライブ値を読み、失敗時はローカルのCodexログに残った `codex.rate_limits` をフォールバックとして使います。

## 表示内容

- 外側リング: 短い時間窓の残量
- 内側リング: 週次上限の残量
- 残量が少なくなると、リング色がグリーン/ブルーからアンバー、レッドへ変化
- petまたはリングにホバーすると、リング端に残量パーセントを表示
- Codex petを閉じるとリングも非表示
- petを移動すると、リングもpetに追従

## 必要なもの

- Windows 10 または Windows 11
- Codex desktop app
- Codex pet が有効になっていること
- PowerShell 5.1 以上
- Rust/Cargo stable toolchain
- Windows Rustビルド環境
  通常は `x86_64-pc-windows-msvc` と Microsoft C++ Build Tools / Visual Studio Build Tools
- `%USERPROFILE%\.codex` 配下のローカルCodex状態ファイル

不要なもの:

- OpenAI API key
- 管理者権限
- Codex本体へのパッチ

## 使い方

このリポジトリは、現時点ではソースからのビルドを前提にしています。

開発実行:

```powershell
.\tools\run-limit-rings.ps1
```

WindowsのStartupへインストール:

```powershell
.\tools\install-limit-rings.ps1
```

インストール確認:

```powershell
.\tools\verify-limit-rings.ps1
```

アンインストール:

```powershell
.\tools\uninstall-limit-rings.ps1
```

リング位置が少しずれる場合は、petの中心にマウスカーソルを置いて `Ctrl+Alt+R` を押してください。通知領域メニューの `Left` / `Right` / `Up` / `Down` でも微調整できます。

## Codexにインストールを頼む

CodexにこのリポジトリURLを渡す場合は、次のように頼めます。

```text
Install Codex Pet Limit Rings from https://github.com/oudouusa/codex-pet-limit-rings-rs for this Windows computer, start it, and verify it is running.
```

このリポジトリにはCodex向けの作業メモとSkillを同梱しています。

- `AGENTS.md`
- `skills/codex-pet-limit-rings/SKILL.md`
- `docs/limit-rings.md`
- `docs/windows-limit-rings.md`

SkillをローカルCodexへインストールする場合:

```powershell
.\tools\install-codex-skill.ps1
```

## データとプライバシー

このアプリが読むもの:

- `%USERPROFILE%\.codex\.codex-global-state.json`
  Codex petの表示状態と位置
- `%USERPROFILE%\.codex\auth.json`
  ChatGPT利用状況を読むためのローカルアクセストークン
- `%USERPROFILE%\.codex\logs_2.sqlite` または `logs_1.sqlite`
  ライブ取得に失敗した場合のローカルキャッシュ
- `https://chatgpt.com/backend-api/wham/usage`
  ライブ利用状況

このアプリがしないこと:

- OpenAI API keyの要求
- pet画像、スクリーンショット、プロンプト、リポジトリ内容の送信
- Codexアプリ本体の改変

## 開発

フォーマット:

```powershell
cargo fmt --check
```

チェック:

```powershell
cargo check
cargo clippy -- -D warnings
```

リリースビルド:

```powershell
cargo build --release
```

プレビューPNG生成:

```powershell
cargo run -- --preview .\tmp\limit-rings-windows-preview.png --size 220
```

Windows上でのrelease実行ファイル:

```text
target\release\codex-pet-limit-rings.exe
```

WSLなどからWindows GNU targetで確認する場合:

```bash
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_AR=x86_64-w64-mingw32-ar \
cargo build --target x86_64-pc-windows-gnu --release
```

## リポジトリ構成

```text
src/
  main.rs                  エントリポイント
  windows_app.rs           Win32 overlay実装

tools/
  install-limit-rings.ps1  ビルド、インストール、Startup登録
  run-limit-rings.ps1      開発実行
  verify-limit-rings.ps1   インストール確認
  uninstall-limit-rings.ps1
  install-codex-skill.ps1

docs/
  limit-rings.md           データと描画モデル
  windows-limit-rings.md   Windows実装メモ

skills/codex-pet-limit-rings/
  SKILL.md                 Codex向けインストール/検証ワークフロー
```

このリポジトリはWindows/Rust版として整理しているため、参考元に含まれるmacOS Swiftアプリ、shell installer、plist、weather-pet実験コードは含めていません。

## 参考元

- [`petergpt/codex-pet-limit-rings`](https://github.com/petergpt/codex-pet-limit-rings)

設計、ドキュメント構成、コンパニオンアプリとしての境界づけは参考元から着想を得ています。詳細は `NOTICE.md` を参照してください。

## License

MIT. See `LICENSE`.
