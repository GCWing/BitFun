// Daily Divination — built-in MiniApp.
// Programmer-themed tarot: 24 cards, 4 fortune dimensions, daily-locked via app.storage.
//
// i18n strategy
// -------------
// Every locale-dependent dataset (cards / suits / colors / hours / mantras /
// insights / UI labels) is split into ZH and EN tables of equal length so the
// daily seed always picks the same *index* — switching languages re-renders
// the same fortune in the chosen language without invalidating yesterday's
// stored "drawn" state. Visual fields (symbol/tone) are shared.

// ── Cards: shared visuals + per-locale strings ───────────────────────────
// Hue-balanced palette across 24 cards. Each `tone` is [primary, deep-bg] —
// primary drives accents (fortune bars, scene tint), deep-bg is the card
// background gradient endpoint. Hues are spread roughly uniformly around the
// wheel (red → orange → gold → lime → teal → cyan → blue → indigo → violet
// → magenta → rose) while still nodding to each card's symbolism.
const CARD_VISUALS = [
  // 0  命运之轮 — amethyst (270°)
  { symbol: '✦', tone: ['#6d28d9', '#1a0936'] },
  // 1  星辰指引 — sapphire (220°)
  { symbol: '✶', tone: ['#1e3a8a', '#08112e'] },
  // 2  熔炉之心 — molten orange (18°)
  { symbol: '✺', tone: ['#c2410c', '#2a0a02'] },
  // 3  寂静之钟 — slate (215°, low-sat)
  { symbol: '☾', tone: ['#475569', '#0c121b'] },
  // 4  银河书简 — deep indigo (250°)
  { symbol: '☄', tone: ['#4338ca', '#0d0a2e'] },
  // 5  红宝匠人 — ruby (350°)
  { symbol: '◆', tone: ['#be123c', '#2c0612'] },
  // 6  青铜之蛇 — bronze (35°)
  { symbol: '∞', tone: ['#92400e', '#261105'] },
  // 7  光之回响 — cyan (188°)
  { symbol: '✧', tone: ['#0891b2', '#031f29'] },
  // 8  苔藓低语 — moss (90°)
  { symbol: '❀', tone: ['#65a30d', '#121e02'] },
  // 9  星海罗盘 — steel blue (210°)
  { symbol: '⊛', tone: ['#1d4ed8', '#06163a'] },
  // 10 黄昏炉火 — amber (28°)
  { symbol: '✦', tone: ['#b45309', '#2a1106'] },
  // 11 悬浮之环 — jade (170°)
  { symbol: '◌', tone: ['#0f766e', '#03221f'] },
  // 12 镜面湖 — aqua (198°)
  { symbol: '☼', tone: ['#0369a1', '#031c33'] },
  // 13 深林信使 — forest (135°)
  { symbol: '✉', tone: ['#15803d', '#051a0d'] },
  // 14 夜之提琴 — violet (285°)
  { symbol: '♪', tone: ['#7e22ce', '#1c0830'] },
  // 15 黎明铸铁 — crimson (358°)
  { symbol: '⚔', tone: ['#b91c1c', '#260606'] },
  // 16 极光之纱 — aurora teal-green (160°)
  { symbol: '✤', tone: ['#0d9488', '#02322f'] },
  // 17 羽落之笔 — graphite (220°, near-neutral)
  { symbol: '✎', tone: ['#52525b', '#0d0d10'] },
  // 18 潮汐之环 — ocean (235°)
  { symbol: '∽', tone: ['#1e40af', '#08123a'] },
  // 19 紫晶圣杯 — magenta (305°)
  { symbol: '♥', tone: ['#a21caf', '#2a072d'] },
  // 20 金色齿轮 — gold (45°)
  { symbol: '✦', tone: ['#a16207', '#2a1805'] },
  // 21 晨曦之翼 — rose (335°)
  { symbol: '✿', tone: ['#be185d', '#2c0a1c'] },
  // 22 寒星之刃 — frost steel-cyan (200°, low-sat)
  { symbol: '✝', tone: ['#0e7490', '#03161c'] },
  // 23 月光石阶 — midnight (245°)
  { symbol: '☽', tone: ['#312e81', '#0a0928'] },
];

const CARD_STRINGS = {
  'zh-CN': [
    { name: '命运之轮', tag: '机缘', keyword: '流转 · 节奏', quotes: [
      '每个 commit 都在改变命运的曲率，今天值得一次推送。',
      '齿轮自有其转法，你只需在对的时刻按下回车。',
      '今日属于"先动起来再说"，方向会自己浮现。',
      '昨天卡住的事，换个时间点再试，常常就通了。',
    ] },
    { name: '星辰指引', tag: '希望', keyword: '远方 · 灵感', quotes: [
      '当你卡住时，抬头看看 documentation 之外的世界。',
      '把眼光放远一档，眼前的死结就成了路标。',
      '今天值得收藏一篇与日常项目无关的好文。',
      '相信那个让你心动的"小副业"念头，它在为你导航。',
    ] },
    { name: '熔炉之心', tag: '锻造', keyword: '精炼 · 重构', quotes: [
      '今日适合一次果敢的重构，删除即创造。',
      '你心里那段"早晚要改"的代码，今天就是早。',
      '与其修补，不如把它推回炉火里重铸。',
      '减法比加法更需要勇气，今天你有这份勇气。',
    ] },
    { name: '寂静之钟', tag: '冥想', keyword: '深思 · 沉潜', quotes: [
      '让 IDE 暂停十分钟，答案常在白板上浮现。',
      '今天少打字，多想一想。手指会感谢大脑。',
      '把问题写下来读一遍，半数 bug 当场暴露。',
      '安静是最被低估的生产力工具。',
    ] },
    { name: '银河书简', tag: '智识', keyword: '阅读 · 累积', quotes: [
      '今天读完一个长 issue 的讨论，比写十行代码值钱。',
      '允许自己花一小时读源码，那是滚雪球的开始。',
      '收藏夹里那篇文，今天就读完它。',
      '一篇好的 RFC，胜过十次会议。',
    ] },
    { name: '红宝匠人', tag: '创造', keyword: '雕琢 · 细节', quotes: [
      '把一个边界条件想清楚，就是今天最好的输出。',
      '今日适合打磨那个"差不多了"的细节。',
      '错误信息也是产品的一部分，把它写得人话一点。',
      '一处微调，往往胜过一次重写。',
    ] },
    { name: '青铜之蛇', tag: '蜕变', keyword: '环路 · 蜕变', quotes: [
      '一个 retry-loop 修好了，整条链路都活了过来。',
      '让自己经历一次"原来如此"的瞬间。',
      '今天值得一次彻底的认知刷新。',
      '换个角度看那个老问题，它会变得很小。',
    ] },
    { name: '光之回响', tag: '协作', keyword: '回声 · 共振', quotes: [
      '一句"我来帮你看看"，就是今日最强的 buff。',
      '主动 ping 一下卡住的同事，你的 5 分钟可能省他半天。',
      '今天答一个别人问过你的问题，回声会传得很远。',
      '感谢一位帮过你的同事，越具体越好。',
    ] },
    { name: '苔藓低语', tag: '休憩', keyword: '生长 · 留白', quotes: [
      '让进度条慢一点，让创造力快一点。',
      '今日宜偷一会儿懒，灵感不在键盘上。',
      '允许一天的"看似没产出"，土壤需要时间发酵。',
      '把椅子推开，去窗边站三分钟。',
    ] },
    { name: '星海罗盘', tag: '抉择', keyword: '方向 · 决断', quotes: [
      '别再纠结技术选型，先把第一行代码写出来。',
      '今日适合做出那个一直拖着的决定。',
      '选 A 还是选 B 都行，只要别再选"再等等"。',
      '把方案写在纸上，多数选择会自我揭晓。',
    ] },
    { name: '黄昏炉火', tag: '专注', keyword: '心流 · 燃烧', quotes: [
      '关闭 Slack，今天属于你和编辑器的二人世界。',
      '把今天最想做的事排到上午第一格。',
      '一段不被打断的 90 分钟，胜过一整天的碎片时间。',
      '让"勿扰模式"成为今天的礼物。',
    ] },
    { name: '悬浮之环', tag: '平衡', keyword: '取舍 · 张力', quotes: [
      '完美与上线之间，请选择上线。',
      '今天值得为某件事说一次"不"。',
      '少做一件事，远比多做一件事难。',
      '把范围缩小一半，效果常常翻倍。',
    ] },
    { name: '镜面湖', tag: '复盘', keyword: '映照 · 觉察', quotes: [
      '回看一周前自己写的代码，会比 review 更诚实。',
      '今天写一段三行的复盘，明天就用得到。',
      '问自己：这一周最让我自豪的一件事是什么？',
      '过去的你犯过的错，未必你今天还在犯。',
    ] },
    { name: '深林信使', tag: '消息', keyword: '传达 · 链接', quotes: [
      '一封写得清楚的邮件，胜过三场会议。',
      '今天适合主动同步一次进展，让信息走在前面。',
      '把那条想了三天的话发出去，最坏不过没回复。',
      '一句"对齐一下"，能省掉一周的猜测。',
    ] },
    { name: '夜之提琴', tag: '诗意', keyword: '韵律 · 优雅', quotes: [
      '为变量起一个动听的名字，命名是程序员的诗。',
      '今天写一段你愿意拿给朋友看的代码。',
      '让函数像句子那样易读，让模块像段落那样自洽。',
      '把空行用得像呼吸一样自然。',
    ] },
    { name: '黎明铸铁', tag: '勇气', keyword: '直面 · 挑战', quotes: [
      '今天直面那个一直被你跳过的 TODO。',
      '把最难的那件事放在第一个，剩下的会变容易。',
      '该说的话就说出来，迟到的反馈是没礼貌的反馈。',
      '把"等我学会再做"换成"边做边学"。',
    ] },
    { name: '极光之纱', tag: '灵感', keyword: '迸发 · 流动', quotes: [
      '保持沐浴或散步的状态，bug 多半在水流声里被冲掉。',
      '今日的好点子在键盘外，记得带个本子。',
      '允许自己暂时离开屏幕，灵感会从背后追上来。',
      '换一个写代码的地方，思路也会跟着挪窝。',
    ] },
    { name: '羽落之笔', tag: '记录', keyword: '书写 · 沉淀', quotes: [
      '今日适合写一篇文档，未来的你会感谢现在的自己。',
      '把口口相传的规则落到 README 里。',
      '为今天的小决定写一句"为什么"，半年后它救你。',
      '把脑子里的图画到 README 里，团队就有了共识。',
    ] },
    { name: '潮汐之环', tag: '节奏', keyword: '起伏 · 周期', quotes: [
      '高效与低谷皆是潮汐，重要的是别在退潮时责怪自己。',
      '今日宜跟着身体走，效率自有其潮位。',
      '不必每天都全力奔跑，会跑的人也会走。',
      '低能量时段，做低能量任务，那叫聪明。',
    ] },
    { name: '紫晶圣杯', tag: '丰饶', keyword: '滋养 · 馈赠', quotes: [
      '别忘了喝水。也别忘了夸自己一句。',
      '今日给自己留一份小奖励，哪怕是一杯好咖啡。',
      '吃顿好的，再回去 debug。',
      '今天对自己温柔一些，世界对你也会。',
    ] },
    { name: '金色齿轮', tag: '系统', keyword: '机制 · 架构', quotes: [
      '一个清晰的模块边界，胜过十个聪明的 hack。',
      '今日宜画一张架构图，在脑子之外把它显形。',
      '与其打补丁，不如先想清楚是谁在和谁说话。',
      '为机制投资一点时间，未来连本带利还你。',
    ] },
    { name: '晨曦之翼', tag: '启程', keyword: '出发 · 第一步', quotes: [
      '把"等我准备好"换成"先 push 一个 draft PR"。',
      '今日适合开一个新仓库，哪怕只写一个 README。',
      '0 → 1 永远是最难也最值得的那一步。',
      '只要开始，就已经领先昨天的自己。',
    ] },
    { name: '寒星之刃', tag: '清算', keyword: '剔除 · 净化', quotes: [
      '今天适合删一些过时的依赖，少即是多。',
      '把那个一年没人用的功能下线吧。',
      '收件箱清零一次，整个人都轻盈了。',
      '过期的待办，不删就是在偷未来你的注意力。',
    ] },
    { name: '月光石阶', tag: '指引', keyword: '夜行 · 步步', quotes: [
      '不必看清整个阶梯，先迈出眼前的这一步。',
      '今日只问"下一小步是什么"，别的交给明天。',
      '黑暗里走得稳的人，都不靠看清远方。',
      '把大目标拆到 30 分钟以内，再开始动手。',
    ] },
  ],
  'zh-TW': [
    { name: '命運之輪', tag: '機緣', keyword: '流轉 · 節奏', quotes: [
      '每個 commit 都在改變命運的曲率，今天值得一次推送。',
      '齒輪自有其轉法，你只需在對的時刻按下回車。',
      '今日屬於"先動起來再說"，方向會自己浮現。',
      '昨天卡住的事，換個時間點再試，常常就通了。',
    ] },
    { name: '星辰指引', tag: '希望', keyword: '遠方 · 靈感', quotes: [
      '當你卡住時，抬頭看看 documentation 之外的世界。',
      '把眼光放遠一檔，眼前的死結就成了路標。',
      '今天值得收藏一篇與日常項目無關的好文。',
      '相信那個讓你心動的"小副業"念頭，它在為你導航。',
    ] },
    { name: '熔爐之心', tag: '鍛造', keyword: '精煉 · 重構', quotes: [
      '今日適合一次果敢的重構，刪除即創造。',
      '你心裡那段"早晚要改"的代碼，今天就是早。',
      '與其修補，不如把它推回爐火裡重鑄。',
      '減法比加法更需要勇氣，今天你有這份勇氣。',
    ] },
    { name: '寂靜之鐘', tag: '冥想', keyword: '深思 · 沉潛', quotes: [
      '讓 IDE 暫停十分鐘，答案常在白板上浮現。',
      '今天少打字，多想一想。手指會感謝大腦。',
      '把問題寫下來讀一遍，半數 bug 當場暴露。',
      '安靜是最被低估的生產力工具。',
    ] },
    { name: '銀河書簡', tag: '智識', keyword: '閱讀 · 累積', quotes: [
      '今天讀完一個長 issue 的討論，比寫十行代碼值錢。',
      '允許自己花一小時讀源碼，那是滾雪球的開始。',
      '收藏夾裡那篇文，今天就讀完它。',
      '一篇好的 RFC，勝過十次會議。',
    ] },
    { name: '紅寶匠人', tag: '創造', keyword: '雕琢 · 細節', quotes: [
      '把一個邊界條件想清楚，就是今天最好的輸出。',
      '今日適合打磨那個"差不多了"的細節。',
      '錯誤信息也是產品的一部分，把它寫得人話一點。',
      '一處微調，往往勝過一次重寫。',
    ] },
    { name: '青銅之蛇', tag: '蛻變', keyword: '環路 · 蛻變', quotes: [
      '一個 retry-loop 修好了，整條鏈路都活了過來。',
      '讓自己經歷一次"原來如此"的瞬間。',
      '今天值得一次徹底的認知刷新。',
      '換個角度看那個老問題，它會變得很小。',
    ] },
    { name: '光之迴響', tag: '協作', keyword: '回聲 · 共振', quotes: [
      '一句"我來幫你看看"，就是今日最強的 buff。',
      '主動 ping 一下卡住的同事，你的 5 分鐘可能省他半天。',
      '今天答一個別人問過你的問題，回聲會傳得很遠。',
      '感謝一位幫過你的同事，越具體越好。',
    ] },
    { name: '苔蘚低語', tag: '休憩', keyword: '生長 · 留白', quotes: [
      '讓進度條慢一點，讓創造力快一點。',
      '今日宜偷一會兒懶，靈感不在鍵盤上。',
      '允許一天的"看似沒產出"，土壤需要時間發酵。',
      '把椅子推開，去窗邊站三分鐘。',
    ] },
    { name: '星海羅盤', tag: '抉擇', keyword: '方向 · 決斷', quotes: [
      '別再糾結技術選型，先把第一行代碼寫出來。',
      '今日適合做出那個一直拖著的決定。',
      '選 A 還是選 B 都行，只要別再選"再等等"。',
      '把方案寫在紙上，多數選擇會自我揭曉。',
    ] },
    { name: '黃昏爐火', tag: '專注', keyword: '心流 · 燃燒', quotes: [
      '關閉 Slack，今天屬於你和編輯器的二人世界。',
      '把今天最想做的事排到上午第一格。',
      '一段不被打斷的 90 分鐘，勝過一整天的碎片時間。',
      '讓"勿擾模式"成為今天的禮物。',
    ] },
    { name: '懸浮之環', tag: '平衡', keyword: '取捨 · 張力', quotes: [
      '完美與上線之間，請選擇上線。',
      '今天值得為某件事說一次"不"。',
      '少做一件事，遠比多做一件事難。',
      '把範圍縮小一半，效果常常翻倍。',
    ] },
    { name: '鏡面湖', tag: '覆盤', keyword: '映照 · 覺察', quotes: [
      '回看一週前自己寫的代碼，會比 review 更誠實。',
      '今天寫一段三行的覆盤，明天就用得到。',
      '問自己：這一週最讓我自豪的一件事是什麼？',
      '過去的你犯過的錯，未必你今天還在犯。',
    ] },
    { name: '深林信使', tag: '消息', keyword: '傳達 · 鏈接', quotes: [
      '一封寫得清楚的郵件，勝過三場會議。',
      '今天適合主動同步一次進展，讓信息走在前面。',
      '把那條想了三天的話發出去，最壞不過沒回復。',
      '一句"對齊一下"，能省掉一週的猜測。',
    ] },
    { name: '夜之提琴', tag: '詩意', keyword: '韻律 · 優雅', quotes: [
      '為變量起一個動聽的名字，命名是程序員的詩。',
      '今天寫一段你願意拿給朋友看的代碼。',
      '讓函數像句子那樣易讀，讓模塊像段落那樣自洽。',
      '把空行用得像呼吸一樣自然。',
    ] },
    { name: '黎明鑄鐵', tag: '勇氣', keyword: '直面 · 挑戰', quotes: [
      '今天直面那個一直被你跳過的 TODO。',
      '把最難的那件事放在第一個，剩下的會變容易。',
      '該說的話就說出來，遲到的反饋是沒禮貌的反饋。',
      '把"等我學會再做"換成"邊做邊學"。',
    ] },
    { name: '極光之紗', tag: '靈感', keyword: '迸發 · 流動', quotes: [
      '保持沐浴或散步的狀態，bug 多半在水流聲裡被沖掉。',
      '今日的好點子在鍵盤外，記得帶個本子。',
      '允許自己暫時離開屏幕，靈感會從背後追上來。',
      '換一個寫代碼的地方，思路也會跟著挪窩。',
    ] },
    { name: '羽落之筆', tag: '記錄', keyword: '書寫 · 沉澱', quotes: [
      '今日適合寫一篇文檔，未來的你會感謝現在的自己。',
      '把口口相傳的規則落到 README 裡。',
      '為今天的小決定寫一句"為什麼"，半年後它救你。',
      '把腦子裡的圖畫到 README 裡，團隊就有了共識。',
    ] },
    { name: '潮汐之環', tag: '節奏', keyword: '起伏 · 週期', quotes: [
      '高效與低谷皆是潮汐，重要的是別在退潮時責怪自己。',
      '今日宜跟著身體走，效率自有其潮位。',
      '不必每天都全力奔跑，會跑的人也會走。',
      '低能量時段，做低能量任務，那叫聰明。',
    ] },
    { name: '紫晶聖盃', tag: '豐饒', keyword: '滋養 · 饋贈', quotes: [
      '別忘了喝水。也別忘了誇自己一句。',
      '今日給自己留一份小獎勵，哪怕是一杯好咖啡。',
      '吃頓好的，再回去 debug。',
      '今天對自己溫柔一些，世界對你也會。',
    ] },
    { name: '金色齒輪', tag: '系統', keyword: '機制 · 架構', quotes: [
      '一個清晰的模塊邊界，勝過十個聰明的 hack。',
      '今日宜畫一張架構圖，在腦子之外把它顯形。',
      '與其打補丁，不如先想清楚是誰在和誰說話。',
      '為機制投資一點時間，未來連本帶利還你。',
    ] },
    { name: '晨曦之翼', tag: '啟程', keyword: '出發 · 第一步', quotes: [
      '把"等我準備好"換成"先 push 一個 draft PR"。',
      '今日適合開一個新倉庫，哪怕只寫一個 README。',
      '0 → 1 永遠是最難也最值得的那一步。',
      '只要開始，就已經領先昨天的自己。',
    ] },
    { name: '寒星之刃', tag: '清算', keyword: '剔除 · 淨化', quotes: [
      '今天適合刪一些過時的依賴，少即是多。',
      '把那個一年沒人用的功能下線吧。',
      '收件箱清零一次，整個人都輕盈了。',
      '過期的待辦，不刪就是在偷未來你的注意力。',
    ] },
    { name: '月光石階', tag: '指引', keyword: '夜行 · 步步', quotes: [
      '不必看清整個階梯，先邁出眼前的這一步。',
      '今日只問"下一小步是什麼"，別的交給明天。',
      '黑暗裡走得穩的人，都不靠看清遠方。',
      '把大目標拆到 30 分鐘以內，再開始動手。',
    ] },
  ],

  'en-US': [
    { name: 'Wheel of Fortune', tag: 'Chance', keyword: 'Flow · Rhythm', quotes: [
      'Every commit bends the curve of fate — today is worth a push.',
      'The gears spin themselves; you just press Enter at the right moment.',
      'Today belongs to "start moving"; direction will reveal itself.',
      'What blocked you yesterday often unblocks itself at a different hour.',
    ] },
    { name: 'Star Compass', tag: 'Hope', keyword: 'Distance · Inspiration', quotes: [
      'When stuck, look beyond the documentation.',
      'Zoom out one notch — the knot turns into a signpost.',
      'Save one good article unrelated to today\'s project.',
      'Trust that little side-project itch; it knows where to take you.',
    ] },
    { name: 'Heart of the Forge', tag: 'Forge', keyword: 'Refine · Refactor', quotes: [
      'Today rewards a brave refactor — deletion is creation.',
      'That code you swore you\'d fix "someday" — today is someday.',
      'Stop patching; cast it back into the fire and reforge it.',
      'Subtraction takes more courage than addition; today you have it.',
    ] },
    { name: 'Silent Bell', tag: 'Meditate', keyword: 'Reflect · Sink', quotes: [
      'Pause your IDE for ten minutes — answers surface on the whiteboard.',
      'Type less, think more. Your fingers will thank your brain.',
      'Write the problem down and read it once — half the bugs reveal themselves.',
      'Quiet is the most underrated productivity tool.',
    ] },
    { name: 'Galactic Codex', tag: 'Knowledge', keyword: 'Read · Compound', quotes: [
      'Reading one long issue thread today beats writing ten lines of code.',
      'Allow yourself an hour of source-reading — that\'s how the snowball starts.',
      'That tab in your "read later" — finish it today.',
      'A good RFC beats ten meetings.',
    ] },
    { name: 'Ruby Artisan', tag: 'Craft', keyword: 'Polish · Detail', quotes: [
      'Thinking one edge case through clearly is today\'s best output.',
      'Polish the detail you\'ve been calling "good enough".',
      'Error messages are part of the product — write them like a human.',
      'One small tweak often beats one full rewrite.',
    ] },
    { name: 'Bronze Serpent', tag: 'Shed', keyword: 'Loop · Renewal', quotes: [
      'Fix one retry-loop and the whole pipeline comes back to life.',
      'Let yourself have one "oh, that\'s why" moment today.',
      'Today deserves a real cognitive refresh.',
      'View that old problem from another angle — it shrinks.',
    ] },
    { name: 'Echo of Light', tag: 'Collab', keyword: 'Echo · Resonance', quotes: [
      '"Let me take a look" is today\'s strongest buff.',
      'Ping a stuck teammate — your 5 minutes may save their afternoon.',
      'Answer a question someone once asked you; the echo travels far.',
      'Thank someone who helped you — the more specific, the better.',
    ] },
    { name: 'Moss Whispers', tag: 'Rest', keyword: 'Grow · Whitespace', quotes: [
      'Slow the progress bar, speed the imagination.',
      'Today permits a little laziness — inspiration isn\'t on the keyboard.',
      'Allow a day that "looks unproductive" — soil needs time to ferment.',
      'Push the chair back; stand by the window for three minutes.',
    ] },
    { name: 'Astrolabe', tag: 'Decide', keyword: 'Direction · Resolve', quotes: [
      'Stop agonizing over stack choices — write line one first.',
      'Today is a good day to make that decision you\'ve been postponing.',
      'A or B is fine — just stop choosing "wait a bit longer".',
      'Write the options on paper; most choices unmask themselves.',
    ] },
    { name: 'Dusk Hearth', tag: 'Focus', keyword: 'Flow · Burn', quotes: [
      'Close Slack — today belongs to you and your editor.',
      'Put your most important task in the first slot of the morning.',
      'Ninety unbroken minutes beat a whole day of fragments.',
      'Let "Do Not Disturb" be today\'s gift to yourself.',
    ] },
    { name: 'Floating Ring', tag: 'Balance', keyword: 'Trade · Tension', quotes: [
      'Between perfect and shipped, choose shipped.',
      'Today is worth saying "no" to one thing.',
      'Doing one thing less is harder than doing one thing more.',
      'Halve the scope and the impact often doubles.',
    ] },
    { name: 'Mirror Lake', tag: 'Reflect', keyword: 'Reflect · Awareness', quotes: [
      'Re-reading code from a week ago is more honest than any review.',
      'Write a three-line retro today; tomorrow will use it.',
      'Ask yourself: what am I most proud of this week?',
      'Mistakes the past you made — today you may already be past them.',
    ] },
    { name: 'Forest Courier', tag: 'Message', keyword: 'Convey · Connect', quotes: [
      'One clearly written email beats three meetings.',
      'Sync progress proactively; let information run ahead.',
      'Send the message you\'ve been drafting for three days — silence is the worst case.',
      'A simple "let\'s align" saves a week of guessing.',
    ] },
    { name: 'Night Violin', tag: 'Poetic', keyword: 'Cadence · Grace', quotes: [
      'Give a variable a beautiful name — naming is the programmer\'s poetry.',
      'Today, write code you\'d show a friend.',
      'Make functions read like sentences and modules cohere like paragraphs.',
      'Use blank lines as naturally as breath.',
    ] },
    { name: 'Dawn Iron', tag: 'Courage', keyword: 'Face · Challenge', quotes: [
      'Face the TODO you\'ve been skipping.',
      'Put the hardest task first; the rest become easier.',
      'Say the thing — late feedback is rude feedback.',
      'Replace "after I learn it" with "learn while doing".',
    ] },
    { name: 'Aurora Veil', tag: 'Inspire', keyword: 'Burst · Flow', quotes: [
      'Take a shower or a walk — most bugs wash away in running water.',
      'Today\'s best ideas are off the keyboard; bring a notebook.',
      'Let yourself leave the screen; inspiration catches up from behind.',
      'Change where you code and your thinking changes too.',
    ] },
    { name: 'Feather Quill', tag: 'Record', keyword: 'Write · Settle', quotes: [
      'Today is for writing a doc — future-you will be grateful.',
      'Move tribal knowledge into the README.',
      'Add one "why" to today\'s small decision; six months later it saves you.',
      'Draw the picture in your head into the README; the team gets shared truth.',
    ] },
    { name: 'Tidal Ring', tag: 'Rhythm', keyword: 'Ebb · Cycle', quotes: [
      'Both peaks and troughs are tides — don\'t blame yourself at low tide.',
      'Today, follow your body; productivity has its own waterline.',
      'You don\'t need to sprint every day; the best runners also walk.',
      'Match low-energy hours with low-energy tasks — that\'s being smart.',
    ] },
    { name: 'Amethyst Chalice', tag: 'Bounty', keyword: 'Nourish · Gift', quotes: [
      'Don\'t forget to drink water. Or to praise yourself.',
      'Leave yourself a small reward today, even just a great coffee.',
      'Eat well, then go back to debugging.',
      'Be gentle with yourself today; the world will return the favor.',
    ] },
    { name: 'Golden Gear', tag: 'System', keyword: 'Mechanism · Architecture', quotes: [
      'A clear module boundary beats ten clever hacks.',
      'Today, draw an architecture diagram; make it real outside your head.',
      'Before patching, ask who is talking to whom.',
      'Invest in mechanism; the future repays with interest.',
    ] },
    { name: 'Dawn Wings', tag: 'Begin', keyword: 'Depart · First step', quotes: [
      'Replace "when I\'m ready" with "open a draft PR".',
      'Today is for starting a new repo — even just a README.',
      '0 → 1 is always the hardest and most worthwhile step.',
      'The moment you start, you\'re already ahead of yesterday.',
    ] },
    { name: 'Frost Star Blade', tag: 'Purge', keyword: 'Prune · Cleanse', quotes: [
      'Today is for deleting outdated dependencies — less is more.',
      'Sunset that feature no one has used in a year.',
      'Inbox-zero once and your whole self feels lighter.',
      'Stale TODOs steal future-you\'s attention; delete them.',
    ] },
    { name: 'Moonlit Steps', tag: 'Guide', keyword: 'Night walk · Step', quotes: [
      'You don\'t need to see the whole staircase — just take the next step.',
      'Today, only ask "what is the next small step"; leave the rest to tomorrow.',
      'Those who walk steadily in the dark don\'t depend on seeing far.',
      'Cut big goals into 30-minute slices, then begin.',
    ] },
  ],
};

const FORTUNE_KEY_IDS = ['overall', 'work', 'inspire', 'wealth'];

const SUITS_GOOD = {
  'zh-CN': [
    '重构一段陈年代码', '写一篇技术笔记', '认真做一次 Code Review', 'Pair programming 一小时',
    '提一个 draft PR', '关闭通知专注 90 分钟', '用便签理清需求', '部署一次到测试环境',
    '认真补单元测试', '把一个 TODO 注释清掉', '请同事喝一杯咖啡', '早一点下班，散步回家',
    '给变量起个好听的名字', '更新依赖小版本', '阅读一份开源项目 README',
    '把脑子里的草图画到白板上', '为某段代码加一段中文注释', '清空一次桌面文件夹',
    '回顾上周的待办，删掉两条', '把一个老 issue 关掉', '写一段集成测试',
    '把一个长函数拆成两个', '给项目加一行 logging', '主动同步一次进展',
    '请教一个不熟悉领域的同事', '为新人写一份"如何上手"', '把一个 TODO 转成 issue',
    '尝试一个新的快捷键', '把一段 if-else 改成查表', '把一个魔法数字提成常量',
    '用纸笔思考十分钟', '尝试一种新的休息节奏', '在 commit message 里写"为什么"',
    '回应一个搁置的 PR comment', '主动 1:1 一位同事', '为今天定一个最重要的目标',
    '关掉两个长期不看的群', '为周报准备一段亮点', '把混乱的 imports 排好',
    '为一个边界条件加一个测试', '抽一段时间彻底安静地思考', '感谢一个帮过你的人',
  ],
  'zh-TW': [
    '重構一段陳年代碼', '寫一篇技術筆記', '認真做一次 Code Review', 'Pair programming 一小時',
    '提一個 draft PR', '關閉通知專注 90 分鐘', '用便籤理清需求', '部署一次到測試環境',
    '認真補單元測試', '把一個 TODO 註釋清掉', '請同事喝一杯咖啡', '早一點下班，散步回家',
    '給變量起個好聽的名字', '更新依賴小版本', '閱讀一份開源項目 README',
    '把腦子裡的草圖畫到白板上', '為某段代碼加一段中文註釋', '清空一次桌面文件夾',
    '回顧上週的待辦，刪掉兩條', '把一個老 issue 關掉', '寫一段集成測試',
    '把一個長函數拆成兩個', '給項目加一行 logging', '主動同步一次進展',
    '請教一個不熟悉領域的同事', '為新人寫一份"如何上手"', '把一個 TODO 轉成 issue',
    '嘗試一個新的快捷鍵', '把一段 if-else 改成查表', '把一個魔法數字提成常量',
    '用紙筆思考十分鐘', '嘗試一種新的休息節奏', '在 commit message 裡寫"為什麼"',
    '回應一個擱置的 PR comment', '主動 1:1 一位同事', '為今天定一個最重要的目標',
    '關掉兩個長期不看的群', '為週報準備一段亮點', '把混亂的 imports 排好',
    '為一個邊界條件加一個測試', '抽一段時間徹底安靜地思考', '感謝一個幫過你的人',
  ],

  'en-US': [
    'Refactor an old piece of code', 'Write a tech note', 'Do a real code review', 'Pair-program for an hour',
    'Open a draft PR', 'Mute notifications for 90 minutes', 'Lay out the requirements on sticky notes', 'Deploy once to staging',
    'Backfill some unit tests', 'Resolve one TODO comment', 'Buy a teammate coffee', 'Leave a bit early and walk home',
    'Pick a beautiful variable name', 'Bump a minor dependency', 'Read an open-source README',
    'Move the sketch in your head onto a whiteboard', 'Add a doc comment to a tricky block', 'Clean your desktop folder',
    'Drop two items from last week\'s todos', 'Close an old issue', 'Write one integration test',
    'Split a long function into two', 'Add one logging line to the project', 'Sync your progress proactively',
    'Ask an expert in an unfamiliar area', 'Write a "getting started" for newcomers', 'Turn a TODO into an issue',
    'Try a new keyboard shortcut', 'Replace an if-else chain with a lookup', 'Hoist a magic number into a constant',
    'Think with paper and pen for ten minutes', 'Try a new rest rhythm', 'Write the "why" in your commit message',
    'Reply to a stalled PR comment', 'Schedule a 1:1 with a teammate', 'Pick the most important goal of the day',
    'Mute two long-ignored chat rooms', 'Prep one highlight for the weekly report', 'Tidy up messy imports',
    'Add a test for an edge case', 'Take a stretch of true quiet thought', 'Thank someone who helped you',
  ],
};

const SUITS_BAD = {
  'zh-CN': [
    '周五傍晚发布到生产', '直接改 main 分支', 'git push --force', '跳过测试就合并',
    'rm -rf 不看路径', '在没备份时改数据库', 'npm install -g 不看版本', '关掉 CI 通知',
    '在情绪激动时回复评论', '把 try { ... } catch {} 留在 PR 里', '熬夜调一个一行就能改的 bug',
    '在没看清需求时就动手', '为了赶进度跳过 code review', '同时开十个分支',
    '在 PR 里夹带不相关的改动', '在饿肚子时做架构决定', '凌晨发线上变更',
    '在 review 里只说"LGTM"不解释', '为一个细节争论超过 30 分钟', '把 hotfix 直接合到 main',
    '把"以后再说"写进注释', '把 print 调试当作日志', '在不熟悉的代码里盲目加 try-catch',
    '一边开会一边写关键代码', '同时承诺三件事都给同一天', '在没充分睡眠时上线',
    '反复刷新 CI 当作 debug', '在情绪低谷时做职业决定', '在没看 docs 时就重写它',
    '把 review 当作"挑毛病"',
  ],
  'zh-TW': [
    '週五傍晚發佈到生產', '直接改 main 分支', 'git push --force', '跳過測試就合併',
    'rm -rf 不看路徑', '在沒備份時改數據庫', 'npm install -g 不看版本', '關掉 CI 通知',
    '在情緒激動時回覆評論', '把 try { ... } catch {} 留在 PR 裡', '熬夜調一個一行就能改的 bug',
    '在沒看清需求時就動手', '為了趕進度跳過 code review', '同時開十個分支',
    '在 PR 裡夾帶不相關的改動', '在餓肚子時做架構決定', '凌晨發線上變更',
    '在 review 裡只說"LGTM"不解釋', '為一個細節爭論超過 30 分鐘', '把 hotfix 直接合到 main',
    '把"以後再說"寫進註釋', '把 print 調試當作日誌', '在不熟悉的代碼裡盲目加 try-catch',
    '一邊開會一邊寫關鍵代碼', '同時承諾三件事都給同一天', '在沒充分睡眠時上線',
    '反覆刷新 CI 當作 debug', '在情緒低谷時做職業決定', '在沒看 docs 時就重寫它',
    '把 review 當作"挑毛病"',
  ],

  'en-US': [
    'Ship to production on a Friday evening', 'Push straight to main', 'git push --force', 'Merge without running tests',
    'rm -rf without checking the path', 'Touch the database without a backup', 'npm install -g without checking the version', 'Mute CI notifications',
    'Reply to a heated comment while heated', 'Leave try { ... } catch {} in the PR', 'Stay up all night for a one-line bug',
    'Start coding before reading the spec', 'Skip code review to "ship faster"', 'Open ten branches at once',
    'Sneak unrelated changes into a PR', 'Make architecture decisions while hungry', 'Push a production change at midnight',
    'Just say "LGTM" without explaining', 'Argue 30+ minutes over one detail', 'Merge a hotfix straight into main',
    'Write "later" in a comment', 'Use print statements as your logs', 'Wrap unknown code in blind try-catch',
    'Write critical code during a meeting', 'Promise three things due the same day', 'Deploy on too little sleep',
    'Re-trigger CI as a debugging strategy', 'Make career decisions during a low mood', 'Rewrite something before reading its docs',
    'Treat code review as nitpicking',
  ],
};

const COLORS = {
  'zh-CN': [
    { name: '靛青', hex: '#4f46e5' }, { name: '玫珀', hex: '#f472b6' }, { name: '湖蓝', hex: '#06b6d4' },
    { name: '森绿', hex: '#10b981' }, { name: '橙金', hex: '#f59e0b' }, { name: '雾紫', hex: '#a78bfa' },
    { name: '砖红', hex: '#ef4444' }, { name: '雪白', hex: '#f5f5f7' }, { name: '炭黑', hex: '#1f2937' },
    { name: '茶褐', hex: '#92400e' }, { name: '青瓷', hex: '#5eead4' }, { name: '檀香', hex: '#c2956a' },
    { name: '黛蓝', hex: '#3730a3' }, { name: '银灰', hex: '#94a3b8' }, { name: '苔绿', hex: '#65a30d' },
    { name: '梅红', hex: '#be185d' },
  ],
  'zh-TW': [
    { name: '靛青', hex: '#4f46e5' }, { name: '玫珀', hex: '#f472b6' }, { name: '湖藍', hex: '#06b6d4' },
    { name: '森綠', hex: '#10b981' }, { name: '橙金', hex: '#f59e0b' }, { name: '霧紫', hex: '#a78bfa' },
    { name: '磚紅', hex: '#ef4444' }, { name: '雪白', hex: '#f5f5f7' }, { name: '炭黑', hex: '#1f2937' },
    { name: '茶褐', hex: '#92400e' }, { name: '青瓷', hex: '#5eead4' }, { name: '檀香', hex: '#c2956a' },
    { name: '黛藍', hex: '#3730a3' }, { name: '銀灰', hex: '#94a3b8' }, { name: '苔綠', hex: '#65a30d' },
    { name: '梅紅', hex: '#be185d' },
  ],

  'en-US': [
    { name: 'Indigo', hex: '#4f46e5' }, { name: 'Rose Amber', hex: '#f472b6' }, { name: 'Lake Blue', hex: '#06b6d4' },
    { name: 'Forest Green', hex: '#10b981' }, { name: 'Amber Gold', hex: '#f59e0b' }, { name: 'Misty Violet', hex: '#a78bfa' },
    { name: 'Brick Red', hex: '#ef4444' }, { name: 'Snow White', hex: '#f5f5f7' }, { name: 'Charcoal', hex: '#1f2937' },
    { name: 'Tea Brown', hex: '#92400e' }, { name: 'Celadon', hex: '#5eead4' }, { name: 'Sandalwood', hex: '#c2956a' },
    { name: 'Slate Blue', hex: '#3730a3' }, { name: 'Silver Gray', hex: '#94a3b8' }, { name: 'Moss Green', hex: '#65a30d' },
    { name: 'Plum Red', hex: '#be185d' },
  ],
};

const HOURS = {
  'zh-CN': [
    '清晨 07:00 — 08:30', '上午 09:30 — 11:00', '上午 10:30 — 12:00',
    '正午 12:00 — 13:00', '下午 14:00 — 15:30', '下午 15:30 — 17:00',
    '黄昏 17:30 — 19:00', '夜晚 20:00 — 21:30', '夜晚 21:00 — 22:30',
    '深夜 22:00 — 23:30', '深夜 23:00 — 00:30', '凌晨 05:30 — 07:00',
  ],
  'zh-TW': [
    '清晨 07:00 — 08:30', '上午 09:30 — 11:00', '上午 10:30 — 12:00',
    '正午 12:00 — 13:00', '下午 14:00 — 15:30', '下午 15:30 — 17:00',
    '黃昏 17:30 — 19:00', '夜晚 20:00 — 21:30', '夜晚 21:00 — 22:30',
    '深夜 22:00 — 23:30', '深夜 23:00 — 00:30', '凌晨 05:30 — 07:00',
  ],

  'en-US': [
    'Early morning 07:00 — 08:30', 'Morning 09:30 — 11:00', 'Late morning 10:30 — 12:00',
    'Midday 12:00 — 13:00', 'Afternoon 14:00 — 15:30', 'Afternoon 15:30 — 17:00',
    'Dusk 17:30 — 19:00', 'Evening 20:00 — 21:30', 'Evening 21:00 — 22:30',
    'Late night 22:00 — 23:30', 'Late night 23:00 — 00:30', 'Pre-dawn 05:30 — 07:00',
  ],
};

const MANTRAS = {
  'zh-CN': [
    'It compiles. Ship it.',
    'Make it work, make it right, make it fast.',
    'Done is better than perfect.',
    'Premature optimization is the root of all evil.',
    'Read the source, Luke.',
    'Stay hungry, stay foolish.',
    'Talk is cheap, show me the code.',
    '最好的代码，是不必写的代码。',
    '一次只解决一个问题。',
    '能跑起来，就先跑起来。',
    '相信你的下一个 git commit。',
    '今天的我，不评判过去的我。',
    '简单优于复杂，明确优于聪明。',
    '宁可写两遍，也别错抽象一次。',
    '写给人读的代码，顺便能在机器上跑。',
    '今日少做一些，明天多走一些。',
    '走得慢一点，但别停下来。',
    '允许它先丑陋地工作，再优雅地工作。',
    '名字取得好，bug 就少一半。',
    '与其完美地做一件事，不如做完一件事。',
    '别信"以后会重写"，但允许"现在能用"。',
    '允许自己今天只做一件好事。',
    '怀疑你的假设，不要怀疑你的价值。',
    '今天打动你的，未必能打动半年后的你。',
    '一切代码都是债，今天还一点。',
    '先有反馈，再有完美。',
    'Done > Perfect > Started > Nothing.',
    '相信节奏，相信复利。',
  ],
  'zh-TW': [
    'It compiles. Ship it.',
    'Make it work, make it right, make it fast.',
    'Done is better than perfect.',
    'Premature optimization is the root of all evil.',
    'Read the source, Luke.',
    'Stay hungry, stay foolish.',
    'Talk is cheap, show me the code.',
    '最好的代碼，是不必寫的代碼。',
    '一次只解決一個問題。',
    '能跑起來，就先跑起來。',
    '相信你的下一個 git commit。',
    '今天的我，不評判過去的我。',
    '簡單優於複雜，明確優於聰明。',
    '寧可寫兩遍，也別錯抽象一次。',
    '寫給人讀的代碼，順便能在機器上跑。',
    '今日少做一些，明天多走一些。',
    '走得慢一點，但別停下來。',
    '允許它先醜陋地工作，再優雅地工作。',
    '名字取得好，bug 就少一半。',
    '與其完美地做一件事，不如做完一件事。',
    '別信"以後會重寫"，但允許"現在能用"。',
    '允許自己今天只做一件好事。',
    '懷疑你的假設，不要懷疑你的價值。',
    '今天打動你的，未必能打動半年後的你。',
    '一切代碼都是債，今天還一點。',
    '先有反饋，再有完美。',
    'Done > Perfect > Started > Nothing.',
    '相信節奏，相信複利。',
  ],

  'en-US': [
    'It compiles. Ship it.',
    'Make it work, make it right, make it fast.',
    'Done is better than perfect.',
    'Premature optimization is the root of all evil.',
    'Read the source, Luke.',
    'Stay hungry, stay foolish.',
    'Talk is cheap, show me the code.',
    'The best code is the code you don\'t have to write.',
    'Solve one problem at a time.',
    'Get it running first; then get it right.',
    'Trust your next git commit.',
    'Today\'s me does not judge yesterday\'s me.',
    'Simple beats complex; explicit beats clever.',
    'Better write it twice than abstract it wrong once.',
    'Write code humans read; the machine runs it as a bonus.',
    'Do a little less today; walk a little further tomorrow.',
    'Walk slowly, but don\'t stop.',
    'Let it work ugly first; make it elegant later.',
    'A great name halves the bugs.',
    'Finishing one thing beats perfecting it.',
    'Don\'t bet on "I\'ll rewrite later" — bet on "this works now".',
    'Allow yourself one good thing today.',
    'Question your assumptions, never your worth.',
    'What moves you today may not move you in six months.',
    'All code is debt — pay a little today.',
    'Feedback first, perfection later.',
    'Done > Perfect > Started > Nothing.',
    'Trust rhythm; trust compounding.',
  ],
};

const INSIGHTS = {
  'zh-CN': [
    '今日的注意力比时间更稀缺，请优先分配。',
    '与其追求"今天做完什么"，不如确认"今天往哪走"。',
    '碰到第三次的麻烦，就该把它封装成函数。',
    '与其修十个小 bug，不如挖透一个根因。',
    '一个干净的桌面，常常带来一个干净的思路。',
    '把"我感觉"换成"我看到了"。',
    '当方案太多时，说明问题没问对。',
    '让别人少猜一次，团队就快一倍。',
    '高频小同步，胜过偶尔大对齐。',
    '当代码难写，往往是设计在求救。',
    '今天的反馈循环越短，明天的不确定越少。',
    '如果你想加一个特例，先想想是不是模型错了。',
    '别只问"能不能做"，也问"该不该做"。',
    '每一次 push，都是给未来的自己写信。',
    '小决定靠习惯，大决定靠睡一觉。',
    '一个稳定的工具链，胜过十个炫技。',
    '把会议变小，把文档变好。',
    '今日宜留 10% 的余力给意外。',
    '当兴趣来敲门，请它进来坐 10 分钟。',
    '观察一次自己的拖延，不评判，只记录。',
    '把一段重复操作脚本化，未来你会笑出声。',
    '该写测试时写测试，该睡觉时睡觉。',
    '专注是种练习，今天又是一个 set。',
    '当你想放弃，先去倒一杯水再说。',
    '今天遇到的每一个 stack trace，都是免费的课。',
    '不熟悉的领域，先复述一遍再动手。',
    '当代码评审让你不舒服，多半击中了真问题。',
    '把"难"拆成"先做哪一步"，难就开始消解。',
    '允许自己今天只交付 60 分，明天再迭代。',
    '相信复利，但别忘了今天就是利息。',
  ],
  'zh-TW': [
    '今日的注意力比時間更稀缺，請優先分配。',
    '與其追求"今天做完什麼"，不如確認"今天往哪走"。',
    '碰到第三次的麻煩，就該把它封裝成函數。',
    '與其修十個小 bug，不如挖透一個根因。',
    '一個乾淨的桌面，常常帶來一個乾淨的思路。',
    '把"我感覺"換成"我看到了"。',
    '當方案太多時，說明問題沒問對。',
    '讓別人少猜一次，團隊就快一倍。',
    '高頻小同步，勝過偶爾大對齊。',
    '當代碼難寫，往往是設計在求救。',
    '今天的反饋循環越短，明天的不確定越少。',
    '如果你想加一個特例，先想想是不是模型錯了。',
    '別隻問"能不能做"，也問"該不該做"。',
    '每一次 push，都是給未來的自己寫信。',
    '小決定靠習慣，大決定靠睡一覺。',
    '一個穩定的工具鏈，勝過十個炫技。',
    '把會議變小，把文檔變好。',
    '今日宜留 10% 的餘力給意外。',
    '當興趣來敲門，請它進來坐 10 分鐘。',
    '觀察一次自己的拖延，不評判，只記錄。',
    '把一段重複操作腳本化，未來你會笑出聲。',
    '該寫測試時寫測試，該睡覺時睡覺。',
    '專注是種練習，今天又是一個 set。',
    '當你想放棄，先去倒一杯水再說。',
    '今天遇到的每一個 stack trace，都是免費的課。',
    '不熟悉的領域，先複述一遍再動手。',
    '當代碼評審讓你不舒服，多半擊中了真問題。',
    '把"難"拆成"先做哪一步"，難就開始消解。',
    '允許自己今天只交付 60 分，明天再迭代。',
    '相信複利，但別忘了今天就是利息。',
  ],

  'en-US': [
    'Today, attention is scarcer than time — allocate it first.',
    'Instead of "what to finish today", decide "which way to head today".',
    'When trouble hits a third time, wrap it in a function.',
    'Better to dig through one root cause than patch ten symptoms.',
    'A clean desktop often brings a clean train of thought.',
    'Replace "I feel" with "I saw".',
    'Too many solutions usually means the wrong question.',
    'When others have to guess less, the team moves twice as fast.',
    'High-frequency small syncs beat occasional big alignments.',
    'When code is hard to write, design is asking for help.',
    'Shorter feedback loop today; less uncertainty tomorrow.',
    'If you want to add a special case, ask if the model is wrong.',
    'Don\'t just ask "can we do it" — also ask "should we".',
    'Every push is a letter to your future self.',
    'Small decisions ride habits; big decisions ride a good sleep.',
    'One stable toolchain beats ten flashy tricks.',
    'Make meetings smaller; make docs better.',
    'Reserve 10% of today\'s capacity for surprises.',
    'When curiosity knocks, let it sit for ten minutes.',
    'Observe your procrastination once — no judgment, just notes.',
    'Script a repetitive task; future-you will laugh out loud.',
    'Write tests when you should; sleep when you should.',
    'Focus is a practice; today is another set.',
    'When you want to give up, pour a glass of water first.',
    'Every stack trace today is a free lesson.',
    'In unfamiliar territory, paraphrase first, code second.',
    'When code review makes you uncomfortable, it usually struck a real issue.',
    'Break "hard" into "what\'s the first step" — and hard starts dissolving.',
    'Allow yourself a 60-point delivery today; iterate tomorrow.',
    'Trust compounding — but remember: today is the interest payment.',
  ],
};

const UI_I18N = {
  'zh-CN': {
    title: '每日占卜',
    spreadAria: '今日牌阵',
    fortuneMatrix: '运势矩阵',
    todayGood: '今日宜',
    todayBad: '今日忌',
    omenTitle: '机缘提示',
    luckyColor: '幸运色',
    luckyNumber: '幸运数字',
    luckyHour: '推荐时段',
    mantra: '咒语',
    copyText: '复制运势文本',
    footerHint: '愿你今日的代码无 bug，commit 总能通过 review。',
    greetingFresh: '凝神',
    greetingDrawn: '今日卦象已立',
    subtitleFresh: '轻触一张牌，揭开今日卦象',
    subtitleDrawn: '抽一张牌以重温',
    tipFresh: '每日卦象一旦显现便已注定 · 翌日 00:00 焕新',
    tipDrawn: '卦象已注定 · 仪式仅供回味',
    cardAriaLabel: (i) => `第 ${i} 张牌`,
    todayInsightLabel: '◇ 今日洞察 ◇',
    fortuneOverall: '综合', fortuneWork: '工作', fortuneInspire: '灵感', fortuneWealth: '财运',
    dateFormat: ({ y, m, d }) => `${y} 年 ${m} 月 ${d} 日`,
    shareCardLine: (name, keyword) => `【${name}】 ${keyword}`,
    shareInsight: (text) => `今日洞察：${text}`,
    shareGood: (list) => `今日宜：${list.join('、')}`,
    shareBad: (list) => `今日忌：${list.join('、')}`,
    shareLucky: (color, n, hour) => `幸运色：${color}　幸运数字：${n}　推荐时段：${hour}`,
    shareMantra: (text) => `咒语：${text}`,
    toastCopied: '已复制到剪贴板',
    toastCopyFailed: '复制失败',
  },
  'zh-TW': {
    title: '每日佔卜',
    spreadAria: '今日牌陣',
    fortuneMatrix: '運勢矩陣',
    todayGood: '今日宜',
    todayBad: '今日忌',
    omenTitle: '機緣提示',
    luckyColor: '幸運色',
    luckyNumber: '幸運數字',
    luckyHour: '推薦時段',
    mantra: '咒語',
    copyText: '複製運勢文本',
    footerHint: '願你今日的代碼無 bug，commit 總能通過 review。',
    greetingFresh: '凝神',
    greetingDrawn: '今日卦象已立',
    subtitleFresh: '輕觸一張牌，揭開今日卦象',
    subtitleDrawn: '抽一張牌以重溫',
    tipFresh: '每日卦象一旦顯現便已註定 · 翌日 00:00 煥新',
    tipDrawn: '卦象已註定 · 儀式僅供回味',
    cardAriaLabel: (i) => `第 ${i} 張牌`,
    todayInsightLabel: '◇ 今日洞察 ◇',
    fortuneOverall: '綜合', fortuneWork: '工作', fortuneInspire: '靈感', fortuneWealth: '財運',
    dateFormat: ({ y, m, d }) => `${y} 年 ${m} 月 ${d} 日`,
    shareCardLine: (name, keyword) => `【${name}】 ${keyword}`,
    shareInsight: (text) => `今日洞察：${text}`,
    shareGood: (list) => `今日宜：${list.join('、')}`,
    shareBad: (list) => `今日忌：${list.join('、')}`,
    shareLucky: (color, n, hour) => `幸運色：${color}　幸運數字：${n}　推薦時段：${hour}`,
    shareMantra: (text) => `咒語：${text}`,
    toastCopied: '已複製到剪貼板',
    toastCopyFailed: '複製失敗',
  },

  'en-US': {
    title: 'Daily Divination',
    spreadAria: 'Today\'s spread',
    fortuneMatrix: 'Fortune matrix',
    todayGood: 'Do',
    todayBad: 'Don\'t',
    omenTitle: 'Lucky omens',
    luckyColor: 'Lucky color',
    luckyNumber: 'Lucky number',
    luckyHour: 'Best hours',
    mantra: 'Mantra',
    copyText: 'Copy reading',
    footerHint: 'May your code be bug-free and your commits always pass review.',
    greetingFresh: 'Center yourself',
    greetingDrawn: 'Today\'s reading is set',
    subtitleFresh: 'Tap a card to reveal today\'s fortune',
    subtitleDrawn: 'Draw any card to revisit',
    tipFresh: 'Today\'s fortune is fixed once revealed · refreshes at 00:00 tomorrow',
    tipDrawn: 'The reading is set · the ritual is for reflection',
    cardAriaLabel: (i) => `Card ${i}`,
    todayInsightLabel: '◇ Today\'s Insight ◇',
    fortuneOverall: 'Overall', fortuneWork: 'Work', fortuneInspire: 'Inspiration', fortuneWealth: 'Wealth',
    dateFormat: ({ y, m, d }) => {
      const months = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];
      return `${months[Number(m) - 1]} ${Number(d)}, ${y}`;
    },
    shareCardLine: (name, keyword) => `[${name}] ${keyword}`,
    shareInsight: (text) => `Insight: ${text}`,
    shareGood: (list) => `Do: ${list.join(', ')}`,
    shareBad: (list) => `Don't: ${list.join(', ')}`,
    shareLucky: (color, n, hour) => `Lucky color: ${color}   Lucky number: ${n}   Best hours: ${hour}`,
    shareMantra: (text) => `Mantra: ${text}`,
    toastCopied: 'Copied to clipboard',
    toastCopyFailed: 'Copy failed',
  },
};

function currentLocale() {
  return (window.app && window.app.locale) || 'en-US';
}
function ui(key) {
  const lang = currentLocale();
  const table = UI_I18N[lang] || UI_I18N['en-US'];
  return table[key];
}

function getCards() {
  const lang = currentLocale();
  const strings = CARD_STRINGS[lang] || CARD_STRINGS['en-US'];
  return strings.map((s, i) => ({ ...CARD_VISUALS[i], ...s }));
}

function getFortuneLabels() {
  return [
    { key: 'overall', label: ui('fortuneOverall') },
    { key: 'work',    label: ui('fortuneWork') },
    { key: 'inspire', label: ui('fortuneInspire') },
    { key: 'wealth',  label: ui('fortuneWealth') },
  ];
}

// ── Random utilities (seeded) ────────────────────────
function dateKey(d = new Date()) {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

function hashSeed(s) {
  let h = 2166136261 >>> 0;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

function mulberry32(seed) {
  let t = seed >>> 0;
  return function () {
    t = (t + 0x6d2b79f5) >>> 0;
    let r = Math.imul(t ^ (t >>> 15), 1 | t);
    r = (r + Math.imul(r ^ (r >>> 7), 61 | r)) ^ r;
    return ((r ^ (r >>> 14)) >>> 0) / 4294967296;
  };
}

function pickIdx(rand, len) {
  return Math.floor(rand() * len);
}

function pickIndices(rand, len, n) {
  // Sample `n` distinct indices in [0, len). Order matches the original
  // `pickN(rand, arr, n)` so localized arrays of equal length yield matching
  // selections across languages.
  const pool = [];
  for (let i = 0; i < len; i++) pool.push(i);
  const out = [];
  for (let i = 0; i < n && pool.length > 0; i++) {
    const idx = Math.floor(rand() * pool.length);
    out.push(pool.splice(idx, 1)[0]);
  }
  return out;
}

// ── Fortune generation ───────────────────────────────
// `generateFortune` returns INDICES + raw stars. Localization happens at render
// time so changing language re-renders the same reading in another tongue.
function generateFortuneIndices(date) {
  const seed = hashSeed('bitfun-divination-' + date);
  const rand = mulberry32(seed);

  const cardIdx = Math.floor(rand() * CARD_VISUALS.length);

  const stars = FORTUNE_KEY_IDS.map(() => {
    const r = rand();
    return r < 0.06 ? 1 : r < 0.2 ? 2 : r < 0.55 ? 3 : r < 0.85 ? 4 : 5;
  });

  // Quote index inside the chosen card. CARD_STRINGS for both locales must
  // expose the same number of quotes per card, which is the case here.
  const zhQuotes = CARD_STRINGS['zh-CN'][cardIdx].quotes;
  const quoteIdx = Math.floor(rand() * zhQuotes.length);

  const insightIdx = Math.floor(rand() * INSIGHTS['zh-CN'].length);
  const goodIndices = pickIndices(rand, SUITS_GOOD['zh-CN'].length, 3);
  const badIndices  = pickIndices(rand, SUITS_BAD['zh-CN'].length, 2);
  const colorIdx = Math.floor(rand() * COLORS['zh-CN'].length);
  const luckyNumber = 1 + Math.floor(rand() * 99);
  const hourIdx = Math.floor(rand() * HOURS['zh-CN'].length);
  const mantraIdx = Math.floor(rand() * MANTRAS['zh-CN'].length);

  return { cardIdx, stars, quoteIdx, insightIdx, goodIndices, badIndices, colorIdx, luckyNumber, hourIdx, mantraIdx };
}

function localizeFortune(indices) {
  const cards = getCards();
  const card = cards[indices.cardIdx];
  const lang = currentLocale();
  const insights = INSIGHTS[lang] || INSIGHTS['en-US'];
  const good = SUITS_GOOD[lang] || SUITS_GOOD['en-US'];
  const bad = SUITS_BAD[lang] || SUITS_BAD['en-US'];
  const colors = COLORS[lang] || COLORS['en-US'];
  const hours = HOURS[lang] || HOURS['en-US'];
  const mantras = MANTRAS[lang] || MANTRAS['en-US'];
  const fortunes = getFortuneLabels().map((f, i) => ({ ...f, stars: indices.stars[i] }));
  return {
    card,
    quote: card.quotes[indices.quoteIdx % card.quotes.length],
    insight: insights[indices.insightIdx % insights.length],
    fortunes,
    goods: indices.goodIndices.map((i) => good[i % good.length]),
    bads:  indices.badIndices.map((i) => bad[i % bad.length]),
    color: colors[indices.colorIdx % colors.length],
    luckyNumber: indices.luckyNumber,
    hour: hours[indices.hourIdx % hours.length],
    mantra: mantras[indices.mantraIdx % mantras.length],
  };
}

// ── DOM ──────────────────────────────────────────────
const dom = {
  dateLabel: document.getElementById('date-label'),
  drawStage: document.getElementById('draw-stage'),
  resultStage: document.getElementById('result-stage'),
  cardSpread: document.getElementById('card-spread'),
  greeting: document.getElementById('greeting'),
  drawSubtitle: document.getElementById('draw-subtitle'),
  drawTip: document.getElementById('draw-tip'),
  cardFront: document.getElementById('card-front'),
  cardIndex: document.getElementById('card-index'),
  cardTag: document.getElementById('card-tag'),
  cardArt: document.getElementById('card-art'),
  cardName: document.getElementById('card-name'),
  cardKeyword: document.getElementById('card-keyword'),
  cardQuote: document.getElementById('card-quote'),
  cardInsight: document.getElementById('card-insight'),
  fortunes: document.getElementById('fortunes'),
  suitGood: document.getElementById('suit-good'),
  suitBad: document.getElementById('suit-bad'),
  luckyColorSwatch: document.getElementById('lucky-color-swatch'),
  luckyColorName: document.getElementById('lucky-color-name'),
  luckyNumber: document.getElementById('lucky-number'),
  luckyHour: document.getElementById('lucky-hour'),
  luckyMantra: document.getElementById('lucky-mantra'),
  btnShare: document.getElementById('btn-share'),
  toast: document.getElementById('toast'),
};

// We keep the deterministic *indices* (computed from the date) plus whether the
// reading was already drawn — so a locale change can simply re-render in place.
let currentIndices = null;
let currentDate = null;
let currentDrawn = false;

function fmtDate(date) {
  const [y, m, d] = date.split('-');
  return ui('dateFormat')({ y, m: String(parseInt(m, 10)), d: String(parseInt(d, 10)) });
}

function applyStaticI18n() {
  document.documentElement.setAttribute('lang', currentLocale());
  document.querySelectorAll('[data-i18n]').forEach((node) => {
    const key = node.getAttribute('data-i18n');
    const attr = node.getAttribute('data-i18n-attr');
    const value = ui(key);
    if (typeof value !== 'string') return;
    if (attr) node.setAttribute(attr, value);
    else node.textContent = value;
  });
}

// ── Card-back symbols (purely cosmetic; the actual fortune is fixed by date) ──
const BACK_SYMBOLS = ['✦', '✶', '☾', '✧', '☄', '✺', '◌', '☼', '✤'];

function applySceneTone(tone) {
  // Dye the entire scene (background, aurora, card, accents) with the day's
  // card tone so the room feels monochromatic — no clash between purple bg
  // and a blue card. tone[0] is the bright accent, tone[1] is deep shadow.
  const root = document.querySelector('.div-app') || document.body;
  root.style.setProperty('--card-tone-1', tone[0]);
  root.style.setProperty('--card-tone-2', tone[1]);
  if (dom.cardFront) {
    dom.cardFront.style.setProperty('--card-tone-1', tone[0]);
    dom.cardFront.style.setProperty('--card-tone-2', tone[1]);
  }
  if (dom.resultStage) {
    dom.resultStage.style.setProperty('--card-tone-1', tone[0]);
    dom.resultStage.style.setProperty('--card-tone-2', tone[1]);
  }
}

async function init() {
  applyStaticI18n();
  const today = dateKey();
  currentDate = today;
  dom.dateLabel.textContent = fmtDate(today);

  let saved = null;
  try { saved = await app.storage.get('lastReading'); } catch (_e) { /* ignore */ }
  currentDrawn = !!(saved && saved.date === today);
  setupDraw(today, currentDrawn);

  if (window.app && typeof window.app.onLocaleChange === 'function') {
    window.app.onLocaleChange(() => {
      applyStaticI18n();
      if (currentDate) dom.dateLabel.textContent = fmtDate(currentDate);
      // If the user hasn't picked yet, refresh draw labels.
      if (!currentIndices) {
        setupDraw(currentDate, currentDrawn);
      } else {
        // Otherwise re-render the result card in the new language.
        paintResult(localizeFortune(currentIndices));
      }
    });
  }
}

function setupDraw(today, alreadyDrawn) {
  dom.drawStage.hidden = false;
  dom.resultStage.hidden = true;
  dom.resultStage.classList.remove('is-active');
  if (alreadyDrawn) {
    dom.greeting.textContent = ui('greetingDrawn');
    dom.drawSubtitle.textContent = ui('subtitleDrawn');
    dom.drawTip.textContent = ui('tipDrawn');
  } else {
    dom.greeting.textContent = ui('greetingFresh');
    dom.drawSubtitle.textContent = ui('subtitleFresh');
    dom.drawTip.textContent = ui('tipFresh');
  }

  dom.cardSpread.innerHTML = '';
  const seed = hashSeed('spread-' + today);
  const rand = mulberry32(seed);
  const symbols = BACK_SYMBOLS.slice();
  for (let i = symbols.length - 1; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1));
    [symbols[i], symbols[j]] = [symbols[j], symbols[i]];
  }
  const fan = symbols.slice(0, 5);
  fan.forEach((sym, i) => {
    const angle = (i - 2) * 8;
    const lift = Math.abs(i - 2) * 14;
    const card = document.createElement('div');
    card.className = 'card-pick';
    card.style.setProperty('--rot', angle + 'deg');
    card.style.setProperty('--y', lift + 'px');
    card.style.setProperty('--enter-delay', (i * 90) + 'ms');
    card.style.zIndex = String(10 - Math.abs(i - 2));
    card.tabIndex = 0;
    card.setAttribute('role', 'button');
    card.setAttribute('aria-label', ui('cardAriaLabel')(i + 1));
    card.dataset.idx = String(i);
    card.innerHTML = `
      <div class="card-pick__pattern"></div>
      <div class="card-pick__inner">
        <div class="card-pick__symbol">${sym}</div>
      </div>
      <div class="card-pick__shine"></div>
    `;
    const handler = () => onPick(card, today, alreadyDrawn);
    card.addEventListener('click', handler);
    card.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handler(); }
    });
    dom.cardSpread.appendChild(card);
  });
}

let pickInFlight = false;
function spawnBurst(centerEl) {
  // Center the burst on the chosen card, fall back to viewport center.
  let x = window.innerWidth / 2;
  let y = window.innerHeight / 2;
  if (centerEl && centerEl.getBoundingClientRect) {
    const rect = centerEl.getBoundingClientRect();
    x = rect.left + rect.width / 2;
    y = rect.top + rect.height / 2;
  }
  const burst = document.createElement('div');
  burst.className = 'draw-burst';
  burst.style.left = x + 'px';
  burst.style.top = y + 'px';
  document.body.appendChild(burst);
  const veil = document.createElement('div');
  veil.className = 'draw-veil';
  document.body.appendChild(veil);
  setTimeout(() => { burst.remove(); veil.remove(); }, 1300);
}

function onPick(chosen, today, alreadyDrawn) {
  if (pickInFlight) return;
  pickInFlight = true;
  // Compute scatter directions for the discarded cards so they fly outward.
  const cards = Array.from(dom.cardSpread.children);
  const chosenIdx = cards.indexOf(chosen);
  for (let i = 0; i < cards.length; i++) {
    const card = cards[i];
    card.style.pointerEvents = 'none';
    card.tabIndex = -1;
    if (card !== chosen) {
      const dir = i - chosenIdx;
      const dx = dir * 160 + (dir < 0 ? -80 : 80);
      const rot = dir * 18;
      card.style.setProperty('--scatter-x', dx + 'px');
      card.style.setProperty('--scatter-rot', rot + 'deg');
      card.classList.add('is-discarded');
    }
  }
  chosen.classList.add('is-chosen');
  // Pre-compute the day's card so we can start the scene-tone transition
  // in lockstep with the burst+flip animation. CSS will animate `.div-app`
  // background over ~1.4s, so by the time the result is revealed the room
  // is already breathing the new card's color.
  const indices = generateFortuneIndices(today);
  const tone = CARD_VISUALS[indices.cardIdx].tone;
  // After the lift settles, trigger the flip-into-burst sequence.
  setTimeout(() => {
    spawnBurst(chosen);
    chosen.classList.add('is-flipping');
    applySceneTone(tone);
  }, 380);
  setTimeout(() => revealResult(today, alreadyDrawn), 1280);
}

function revealResult(today, alreadyDrawn) {
  currentIndices = generateFortuneIndices(today);
  const fortune = localizeFortune(currentIndices);
  paintResult(fortune);
  dom.drawStage.hidden = true;
  dom.resultStage.hidden = false;
  // eslint-disable-next-line no-unused-expressions
  dom.resultStage.offsetWidth;
  dom.resultStage.classList.add('is-active');
  if (!alreadyDrawn) {
    app.storage.set('lastReading', { date: today, cardIdx: currentIndices.cardIdx }).catch(() => {});
    currentDrawn = true;
  }
  pickInFlight = false;
}

function paintResult(f) {
  dom.btnShare.hidden = false;

  const idx = f.card._index = (CARD_VISUALS.indexOf({ symbol: f.card.symbol, tone: f.card.tone }) + 1) || 0;
  // Use stable index from currentIndices instead — cleaner.
  const stableIdx = (currentIndices ? currentIndices.cardIdx : 0) + 1;
  dom.cardIndex.textContent = `No. ${String(stableIdx).padStart(2, '0')}`;
  dom.cardTag.textContent = f.card.tag;
  dom.cardArt.textContent = f.card.symbol;
  dom.cardName.textContent = f.card.name;
  dom.cardKeyword.textContent = f.card.keyword;
  dom.cardQuote.textContent = f.quote;
  if (dom.cardInsight) {
    dom.cardInsight.innerHTML = '';
    const label = document.createElement('span');
    label.className = 'card-front__insight-label';
    label.textContent = ui('todayInsightLabel');
    const text = document.createElement('span');
    text.className = 'card-front__insight-text';
    text.textContent = f.insight;
    dom.cardInsight.appendChild(label);
    dom.cardInsight.appendChild(text);
  }
  applySceneTone(f.card.tone);

  dom.fortunes.innerHTML = '';
  for (const item of f.fortunes) {
    const li = document.createElement('li');
    li.className = 'fortune';
    li.innerHTML = `
      <span class="fortune__label">${escapeHtml(item.label)}</span>
      <span class="fortune__bar"><span class="fortune__fill" style="width:0"></span></span>
      <span class="fortune__stars">${'★'.repeat(item.stars)}<span class="ghost">${'★'.repeat(5 - item.stars)}</span></span>
    `;
    dom.fortunes.appendChild(li);
    requestAnimationFrame(() => {
      li.querySelector('.fortune__fill').style.width = `${item.stars * 20}%`;
    });
  }

  dom.suitGood.innerHTML = f.goods.map((s) => `<li>${escapeHtml(s)}</li>`).join('');
  dom.suitBad.innerHTML = f.bads.map((s) => `<li>${escapeHtml(s)}</li>`).join('');

  dom.luckyColorSwatch.style.background = f.color.hex;
  dom.luckyColorName.textContent = f.color.name;
  dom.luckyNumber.textContent = String(f.luckyNumber);
  dom.luckyHour.textContent = f.hour;
  dom.luckyMantra.textContent = f.mantra;
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  }[c]));
}

dom.btnShare.addEventListener('click', async () => {
  if (!currentIndices) return;
  const f = localizeFortune(currentIndices);
  const lines = [];
  lines.push(ui('shareCardLine')(f.card.name, f.card.keyword));
  lines.push(f.quote);
  if (f.insight) lines.push(ui('shareInsight')(f.insight));
  lines.push('');
  for (const item of f.fortunes) {
    lines.push(`${item.label}: ${'★'.repeat(item.stars)}${'☆'.repeat(5 - item.stars)}`);
  }
  lines.push('');
  lines.push(ui('shareGood')(f.goods));
  lines.push(ui('shareBad')(f.bads));
  lines.push('');
  lines.push(ui('shareLucky')(f.color.name, f.luckyNumber, f.hour));
  lines.push(ui('shareMantra')(f.mantra));
  const text = lines.join('\n');
  try {
    await app.clipboard.writeText(text);
    showToast(ui('toastCopied'));
  } catch (_e) {
    showToast(ui('toastCopyFailed'));
  }
});

let toastTimer = null;
function showToast(msg) {
  dom.toast.textContent = msg;
  dom.toast.hidden = false;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { dom.toast.hidden = true; }, 1600);
}

init();
