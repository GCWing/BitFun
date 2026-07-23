# 璐＄尞鎸囧崡

[English](./CONTRIBUTING.md)

鎰熻阿浣犲 taiji-quant 鐨勫叴瓒ｏ紒taiji-quant 鏄竴涓敱 Rust 涓?TypeScript 椹卞姩鐨勫绔?AI 缂栫▼鐜锛屾闈㈢/CLI/Server 鍏变韩鏍稿績閫昏緫銆傛湰鎸囧崡璇存槑濡備綍楂樻晥鍙備笌璐＄尞銆?

## 琛屼负鍑嗗垯

璇蜂繚鎸佸皧閲嶃€佸弸鍠勪笌寤鸿鎬ф矡閫氥€傛垜浠杩庝笉鍚岃儗鏅笌缁忛獙鐨勮础鐚€呫€?

## 蹇€熷紑濮?

### 鐜鍑嗗

- Node.js 22.12+锛堝缓璁?LTS 鐗堟湰锛?
- pnpm 10.15.0锛堝缓璁€氳繃 Corepack 浣跨敤锛?
- Rust toolchain锛堥€氳繃 rustup 瀹夎锛?
- 妗岄潰绔紑鍙戦渶鍑嗗 Tauri 渚濊禆

taiji-quant 灏嗘湰鍦?JavaScript 鏋勫缓鍜?CI 缁熶竴鍒?Node.js 22.12+銆備粨搴撻噷鐨?
GitHub Actions 鍗囩骇浣跨敤鐨勬槸鍏煎 Node.js 24 鐨?action runtime锛屼絾椤圭洰鑴氭湰
榛樿浠嶄互 Node.js 22.12+ 涓哄熀绾匡紝闄ら潪灞€閮ㄦ寚鍗楀彟鏈夎鏄庛€備粠鏃?Node.js 鐗堟湰鍒囨崲
鍚庯紝璇烽噸鏂拌繍琛?`pnpm install`銆?

#### Windows锛歄penSSL 閰嶇疆

澶у鏁?Windows 璐＄尞鑰呬笉闇€瑕佹墜鍔ㄩ厤缃?OpenSSL銆備娇鐢?`pnpm run desktop:dev`
鎴栧父瑙?`desktop:build*` 鑴氭湰鍗冲彲锛涜剼鏈細鍦ㄩ渶瑕佹椂鑷姩寮曞棰勭紪璇戠殑 OpenSSL 鍖呫€?

鍙湁鍦ㄨ嚜鍔ㄥ紩瀵煎け璐ャ€佸噯澶?CI 鐜锛屾垨浣犳槑纭娇鐢?`pnpm run desktop:dev:raw`
鏃舵墠闇€瑕佹墜鍔ㄥ鐞嗐€傛鏃惰繍琛?`scripts/ci/setup-openssl-windows.ps1`锛屾垨灏?
`OPENSSL_DIR` 鎸囧悜棰勭紪璇戠殑 x64 OpenSSL 鐩綍锛屽苟璁剧疆 `OPENSSL_STATIC=1`銆?

### 瀹夎渚濊禆

```bash
pnpm install
```

### 甯哥敤鍛戒护

```bash
# Desktop锛堟棩甯稿紑鍙戞帹鑽愶級
pnpm run desktop:dev                # 瀹屾暣鐑洿鏂帮細Vite HMR + Rust 鑷姩閲嶇紪璇戝苟閲嶅惎

# Desktop锛堣交閲忛瑙堬紝鏃?Rust 鑷姩閲嶇紪璇戯級
pnpm run desktop:preview:debug      # 澶嶇敤棰勬瀯寤轰簩杩涘埗 + Vite HMR锛汻ust 鏀瑰姩闇€鎵嬪姩閲嶅惎

# Desktop锛堢敓浜ф瀯寤猴級
pnpm run desktop:build

# E2E
pnpm run e2e:test
```

> **`desktop:dev` 涓?`desktop:preview:debug` 鐨勫尯鍒?*锛歚desktop:dev` 杩愯 `tauri dev`锛屾彁渚?*瀹屾暣鐑洿鏂?* 鈥?鍓嶇鏀瑰姩閫氳繃 Vite HMR 鍗虫椂鐢熸晥锛孯ust/鍚庣鏀瑰姩浼氳Е鍙戝閲忛噸缂栬瘧骞惰嚜鍔ㄩ噸鍚簲鐢紝鏄棩甯稿紑鍙戠殑棣栭€夋柟寮忋€俙desktop:preview:debug` 鍚姩棰勬瀯寤虹殑 debug 浜岃繘鍒跺拰 Vite dev server锛涘墠绔紪杈戜粛鍙?HMR锛屼絾 **Rust 渚ф敼鍔ㄤ笉浼氳嚜鍔ㄩ噸缂栬瘧** 鈥?闇€瑕佹墜鍔ㄥ仠姝㈠苟閲嶆柊杩愯鍛戒护锛堟垨浣跨敤 `--force-rebuild`锛夈€傞€傚悎浠呴渶杩唬鍓嶇浠ｇ爜銆佹垨甯屾湜璺宠繃 `tauri dev` 鍒濆鍖栦互鏇村揩鍐峰惎鍔ㄧ殑鍦烘櫙銆?

> 瀹屾暣鑴氭湰鍒楄〃瑙?[`package.json`](package.json)銆俛gent 涓撶敤鍛戒护銆侀獙璇佷笌鏋舵瀯瑙勫垯瑙?[`AGENTS.md`](AGENTS.md)銆?

### 妗岄潰绔皟璇曞伐鍏?

妗岄潰绔?dev 鏋勫缓浼氬惎鐢?`devtools` Cargo feature銆俙F12` 鎵撳紑鍘熺敓 webview
DevTools锛沗Cmd/Ctrl + Shift + I` 鍒囨崲 taiji-quant 鍏冪礌妫€鏌ュ櫒锛宍Cmd/Ctrl + Shift + J`
涔熷彲浠ユ墦寮€鍘熺敓 DevTools銆傞潰鍚戞渶缁堢敤鎴风殑 `release` 鏋勫缓涓嶄細鍚敤杩欎簺宸ュ叿銆?

## 浠ｇ爜瑙勮寖涓庢灦鏋勭害鏉?

鏋舵瀯鏁忔劅瑙勫垯銆佹ā鍧楄竟鐣屽拰楠岃瘉鐭╅樀浠?[`AGENTS.md`](AGENTS.md) 涓哄噯銆傞潰鍚戣础鐚€呭彧闇€鎶婃彙锛?

- 鏃ュ織鍙娇鐢ㄨ嫳鏂囷紝骞朵繚鎸佸繀瑕併€佸彲璇汇€?
- 鐢ㄦ埛鍙鏂囨璧伴」鐩?i18n 娴佺▼锛涗笉瑕佹妸 Web UI locale catalog 鍏变韩缁欒緝灏忎骇鍝佸舰鎬併€?
- shared core 蹇呴』淇濇寔骞冲彴鏃犲叧锛汥esktop/Tauri 缁嗚妭灞炰簬 app adapter锛屽苟閫氳繃绫诲瀷鍖栬兘鍔涙帴鍙ｅ洖娴侊紱闇€瑕佷簨浠舵姇閫掓椂浣跨敤宸叉湁鐢熶骇 transport adapter銆?
- Tauri command 浣跨敤 `snake_case` 鍛戒护鍚嶅拰缁撴瀯鍖?`request` 鍙傛暟銆?
- core 鎷嗚В銆乫eature 杈圭晫銆佷緷璧栬竟鐣屽拰鏋勫缓鎻愰€熼噸鏋勫繀椤婚伒寰?
  `docs/architecture/product-architecture.md`銆?
- 鍔熻兘绾ц鍒欏簲鏀惧湪绂讳唬鐮佹渶杩戠殑妯″潡 `AGENTS.md` 涓€?

## 閲嶇偣鍏虫敞鐨勮础鐚柟鍚?

1. 璐＄尞濂界殑鎯虫硶/鍒涙剰锛堝姛鑳姐€佷氦浜掋€佽瑙夌瓑锛夛紝鎻愪氦 Issue
   > 娆㈣繋浜у搧缁忕悊銆乁I 璁捐甯堥€氳繃 PI 蹇€熸彁浜ゅ垱鎰忥紝鎴戜滑浼氬府鍔╁畬鍠勫紑鍙?
2. 浼樺寲 Agent 绯荤粺鍜屾晥鏋?
3. 瀵规彁鍗囩郴缁熺ǔ瀹氭€у拰瀹屽杽鍩虹鑳藉姏
4. 鎵╁睍鐢熸€侊紙Skills銆丮CP銆丩SP 鎻掍欢锛屾垨鑰呭鏌愪簺鍨傚煙寮€鍙戝満鏅殑鏇村ソ鏀寔锛?

## 璐＄尞娴佺▼涓?PR 绾﹀畾

### 闄ゅ姛鑳?淇澶栫殑璐＄尞鏂瑰悜

鎴戜滑娆㈣繋涓嶄粎闄愪簬鍔熻兘鎴栦慨澶嶇殑 PR銆傜ず渚嬪寘鎷細

| 璐＄尞鏂瑰悜 | 浣嶇疆/鏂囦欢 | 绀轰緥璇存槑 |
| --- | --- | --- |
| Prompts | `src/crates/assembly/core/src/agentic/agents/prompts/` | 鏂板鎴栦紭鍖栨彁绀鸿瘝锛屽苟鎸夐渶鏇存柊鐩稿叧閫昏緫 |
| Tools | `src/crates/assembly/core/src/agentic/tools/implementations/`銆乣src/crates/assembly/core/src/agentic/tools/registry.rs` | 鏂板宸ュ叿瀹炵幇锛屽苟鍦ㄥ伐鍏锋敞鍐岃〃涓敞鍐?|
| Subagents | `src/crates/assembly/core/src/agentic/agents/custom_subagents/`銆乣src/crates/assembly/core/src/agentic/agents/registry.rs` | 鏂板瀛愪唬鐞嗗疄鐜帮紝骞跺湪瀛愪唬鐞嗘敞鍐岃〃涓敞鍐?|
| 妯″紡璐＄尞 | `src/crates/assembly/core/src/agentic/agents/*_mode.rs`銆乣src/crates/assembly/core/src/agentic/agents/prompts/*_mode.md`銆乣src/web-ui/src/locales/*/settings/modes.json` | 鏂板/浼樺寲 Agent 妯″紡锛堜緥濡?Plan/Debug/Agentic 鎴栬嚜瀹氫箟妯″紡锛夌殑閫昏緫涓庢彁绀鸿瘝锛屽苟鍚屾鍓嶇妯″紡鏂囨 |
| Code Agent 涓?AIIde 鍦烘櫙鎸囧崡 | `website/src/docs/` | 琛ュ厖娴佺▼銆乸laybook 涓庣湡瀹炲満鏅鏄庯紙鎴栦粠 `README.md` 閾炬帴锛?|

### 寮€濮嬪墠

- 鍏堝紑 Issue 璇存槑闂鎴栨柟妗堬紝灏ゅ叾鏄緝澶ф敼鍔紝浠ラ伩鍏嶉噸澶嶄笌璁捐鍐茬獊
- 鏂板姛鑳芥垨 UI 鍙樻洿寤鸿鍏堣璁鸿璁℃柟鍚戯紝纭繚绗﹀悎浜у搧浣撻獙
- 灏?Issue 鍜?PR 妯℃澘浣滀负濉啓鎸囧紩锛涗繚鎸?PR 鑱氱劍锛屽繀瑕佹椂璇存槑璺宠繃浜嗗摢浜涢獙璇佷互鍙婂師鍥犮€?

### PR 鏍囬涓庢弿杩?

寤鸿浣跨敤 Conventional Commits 椋庢牸锛屼究浜庣淮鎶ょ増鏈褰曚笌鑷姩鍖栨祦绋嬶細

- `feat:` 鏂板姛鑳?
- `fix:` 淇闂
- `docs:` 鏂囨。鍙樻洿
- `chore:` 缁存姢/渚濊禆
- `refactor:` 閲嶆瀯涓斾笉鏀硅涓?
- `test:` 娴嬭瘯鐩稿叧

UI 鏀瑰姩璇烽檮鍓嶅悗瀵规瘮鎴浘鎴栫煭褰曞睆锛屾柟渚垮揩閫熻瘎瀹°€?

濡備负 AI 杈呭姪浜у嚭锛岃鍦?PR 涓敞鏄庡苟璇存槑娴嬭瘯绋嬪害锛堟湭娴?杞绘祴/宸叉祴锛夛紝渚夸簬璇勫椋庨櫓銆?

涓嶈鎻愪氦涓存椂 AI prompt銆佹湰鍦扮粷瀵硅矾寰勩€佺敓鎴愮殑鑽夌鏂囦欢銆侀厤瀵瑰瘑閽ャ€乼oken銆佽瘉涔︽垨鏃犲叧浜х墿銆侾R 搴旇仛鐒︿簬鏈浜у搧鎴栫淮鎶ゆ敼鍔ㄣ€?

### 鍒嗘敮绠＄悊

**`main` 鍒嗘敮涓洪粯璁ゅ崗浣滃垎鏀紝骞舵帴鍙楃壒鎬?PR銆?* 鏈粨搴撴杩庝骇鍝佺粡鐞嗐€佸紑鍙戣€呬娇鐢?AI 鐢熸垚浠ｇ爜杩涜蹇€熼獙璇佹垨鎻愪氦鎯虫硶锛屽洜姝?**鎵€鏈?PR 璇风洿鎺ユ彁浜ゅ埌 `main` 鍒嗘敮**銆?

### 鍙樻洿鑼冨洿

淇濇寔 PR 灏忚€岃仛鐒︼紝閬垮厤娣锋潅鏃犲叧鏀瑰姩銆?

## 娴嬭瘯涓庨獙璇?

鎸夋敼鍔ㄦ枃浠跺拰琛屼负閫夋嫨鏈€灏忔鏌ャ€傚畬鏁存瀯寤哄拰澶ц寖鍥存祴璇曠敱 CI 淇濇姢锛涘彧鏈夋敼鍔ㄥ奖鍝嶆瀯寤恒€佹墦鍖呫€佸彂甯冭涓猴紝
鎴?CI 鏃犳硶瑕嗙洊瀵瑰簲璺緞鏃讹紝鎵嶅湪鏈湴杩愯鏇撮噸鍛戒护銆?

甯歌鏈湴妫€鏌ワ細

| 鏀瑰姩绫诲瀷 | 甯哥敤楠岃瘉 |
| --- | --- |
| 浠撳簱鍏冧俊鎭垨 GitHub 閰嶇疆 | `pnpm run check:repo-hygiene && pnpm run check:github-config && git diff --check` |
| 鍓嶇杩愯鏃舵垨 UI | `pnpm run type-check:web`锛涜涓哄彉鍖栨椂鍐嶅姞鏈€杩戠殑 focused test |
| Mobile web | `pnpm --dir src/mobile-web run type-check` |
| Rust 鍏变韩 runtime 鎴?services | `cargo check --workspace`锛涜涓哄彉鍖栨椂鍐嶅姞 focused `cargo test` |
| Desktop/Tauri 闆嗘垚 | `cargo check -p taiji-quant-desktop` |
| i18n 璧勬簮鎴栧绾?| 浣跨敤 `AGENTS.md` 涓尮閰嶇殑 i18n 楠岃瘉琛?|

UI 鏀瑰姩鍦ㄦ湁甯姪鏃堕檮鎴浘鎴栫煭褰曞睆銆傛棤娉曡繍琛岀浉鍏虫鏌ユ椂锛屽湪 PR 涓鏄庡師鍥狅紝骞舵彁渚涢闄╂洿浣庣殑鎵嬪姩楠岃瘉璺緞銆?

## 瀹夊叏涓庡悎瑙?

- 涓嶈鎻愪氦瀵嗛挜銆乀oken銆佽瘉涔︽垨浠讳綍鏁忔劅淇℃伅
- 鏂板渚濊禆璇风‘璁よ鍙瘉鍏煎骞惰鏄庣敤閫?

## 鎰熻阿

姣忎竴浠借础鐚兘寰堥噸瑕侊紝娆㈣繋鎻愪氦 Issue銆丳R 鎴栧缓璁紒

## Taiji 妯″潡

鏈粨搴撳悓鏃舵墭绠?**澶瀬锛圱aiji锛?* 澶氭櫤鑳戒綋閲忓寲绯荤粺锛屼唬鐮佷綅浜?`src/crates/taiji/`銆?

### Crate 甯冨眬

| 绫诲埆 | 鏁伴噺 | Crate |
| --- | --- | --- |
| 娲昏穬 | 20 | `taiji-bar`銆乣taiji-cli`銆乣taiji-engine`銆乣taiji-engine-py`銆乣taiji-content`銆乣taiji-publisher`銆乣taiji-growth`銆乣taiji-alert`銆乣taiji-knowledge-graph`銆乣taiji-blog-gen`銆乣taiji-example`銆乣taiji-llm`銆乣taiji-backtest`銆乣taiji-executor`銆乣taiji-realtime`銆乣taiji-pattern`銆乣taiji-abnormal`銆乣taiji-sentiment`銆乣taiji-orderflow`銆乣taiji-strategen` |
| 闂簮锛堝凡娉ㄩ噴锛?| 4 | `taiji-dvmi`銆乣taiji-magnet`銆乣taiji-thrust`銆乣taiji-risk` |

### 闂簮绛栫暐

涓婅堪 4 涓棴婧?crate 鍦?`Cargo.toml` 鐨?workspace members 涓凡**娉ㄩ噴鎺?*銆?*涓嶅緱鍙栨秷娉ㄩ噴鎴栧皢闂簮浠ｇ爜鎻愪氦鍒板叕寮€浠撳簱銆?* CI 娴嬭瘯 job 鍚屾牱鎺掗櫎杩欎簺 crate銆?

### 寮€濮嬪墠

- 鍔″繀鍏堥槄璇?`docs/architecture/product-architecture.md`锛屼簡瑙ｆ灦鏋勬晱鎰熻鍒欎笌妯″潡杈圭晫銆?
- 鐔熸倝 taiji crate 涔嬮棿鐨勪緷璧栧叧绯伙細鍙傝 `.taiji-quant/team/type-contract-phase8-10.md` 涓殑璺?crate 渚濊禆鐭╅樀涓?merge 瑙勫垯銆?

### 楠岃瘉

鎻愪氦 taiji 鏀瑰姩鍓嶏紝鎸?crate 绾ч獙璇侊細

```bash
cargo check -p <crate>
cargo test -p <crate>
```

workspace 绾?taiji 楠岃瘉锛?

```bash
cargo check --workspace
cargo test --workspace
```

PR 搴旇仛鐒﹀崟涓€鍔熻兘棰嗗煙锛屽鏈夎烦杩囩殑楠岃瘉璇锋敞鏄庡師鍥犮€?
