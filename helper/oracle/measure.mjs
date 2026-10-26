// Text metrics oracle for helper/tests/text_metrics.rs.
//
// Mermaid measures labels in Chrome: `getBBox()` on an SVG `<text>` for
// SVG labels and `getBoundingClientRect()` on a `display: table-cell` div
// inside a `<foreignObject>` for HTML labels. This script opens the same
// page mermaid-cli 12.0.0 opens (dist/index.html), injects the same
// @fontsource stylesheets, and records both measurements for a fixed corpus
// so the Rust measurer can be checked against the real numbers.
//
// Usage: node helper/oracle/measure.mjs [out.json]
// The default output is helper/tests/goldens/metrics.json.
//
// Sweep: node helper/oracle/measure.mjs --sweep out.json measures the much
// wider SWEEP corpus below (one or more strings per Unicode block macOS can
// draw) in the same page, at the families, sizes, and weights mermaid
// labels use. The `sweep_matches_chrome` test in
// helper/tests/text_metrics.rs replays it (`KITTY_MARKDOWN_SWEEP=out.json
// cargo test --test text_metrics sweep -- --ignored --nocapture`). The sweep
// is a search tool for fallback classes the fixed corpus misses; a class it
// finds gets a sample in STRINGS.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';
import { createRequire } from 'node:module';
import puppeteer from 'puppeteer';

const require = createRequire(import.meta.url);
const here = path.dirname(url.fileURLToPath(import.meta.url));
const sweep = process.argv[2] === '--sweep';
const out =
  (sweep ? process.argv[3] : process.argv[2]) ??
  path.join(here, '..', 'tests', 'goldens', 'metrics.json');

const STRINGS = {
  word: 'Hello',
  node: 'Start',
  long_word: 'flowchart',
  kerning: 'AVAWAY Type',
  sentence: 'The quick brown fox jumps over the lazy dog',
  digits: '0123456789',
  punct: '!@#$%^&*()_+-=[]{};\':",./<>?',
  cjk: '中文中 日本語',
  emoji: '🏳️‍⚧️ 🏳️‍🌈',
  empty: '',
  // Fallback text (R-214, R-302): one string per script or symbol class
  // that sends Chrome to another face, or keeps it in the primary face
  // with glyphs outside ASCII.
  latin1: 'Café naïve résumé',
  latin_ext: 'Łódź straße Øresund Ñandú',
  precomposed: 'Café ñ ü',
  combining: 'Café ñ ü',
  quotes: '“quoted” ‘single’ – — …',
  currency: '€ £ ¥ ₹ ₩',
  greek: 'Αλφα βήτα γάμμα Ω',
  cyrillic: 'Привет мир Ёж',
  han: '中文標籤測試',
  kana: 'ひらがな カタカナ',
  hangul: '한국어 라벨',
  arabic: 'مرحبا بالعالم',
  hebrew: 'שלום עולם',
  devanagari: 'नमस्ते दुनिया',
  thai: 'สวัสดีชาวโลก',
  emoji_single: '😀',
  emoji_in_text: 'Done 🎉 now',
  emoji_zwj: '👨‍👩‍👧‍👦',
  emoji_skin: '👍🏽 👋🏿',
  emoji_flag: '🇳🇿 🇯🇵',
  emoji_vs: '❤️ ☺️ ✔️',
  text_vs: '❤︎ ☺',
  symbols: '→ ✓ ★ € ©',
  arrows: '← ↑ ↓ ⇒ ↔',
  math_symbols: '∑ ∞ ≤ ≥ ±',
  box_drawing: '─ ├ └ │',
  mixed: 'Step 1 → résultat ✓ 完成',
  mixed_rtl: 'Order שלום 42',
  mixed_emoji_cjk: 'Ship 🚀 発売',
  // Kerned fallback runs (R-214, R-302): Hiragino Sans kerns kana pairs
  // (グイ, バグ) in its GPOS table, so the kana face and its weight decide
  // the width; Han in the same run goes to PingFang, Hangul to Apple SD
  // Gothic Neo.
  kana_kern: 'ログイン画面のバグを直す',
  kana_pairs: 'グイ バグ ワイヤ タイプ',
  kana_long: 'ダークモードとハイコントラストのテーマを追加する',
  kana_kanji: '作業中のドキュメントの翻訳',
  kana_latin: 'Mixed かな and English',
  kana_punct: '「テスト」、バグ。',
  hangul_text: '안녕하세요 세계 테스트',
  hangul_latin: 'Build 빌드 OK',
  cjk_latin: 'ID番号42のバグ',
  multi_latin: 'Café<br>naïve',
  multi_cjk: '中文<br>Line two',
  multi_emoji: 'Line 😀<br>second ✓',
  // Classes the sweep (`--sweep`) found missing: a sample of each.
  ws_collapse: ' two  spaces\tand tab ',
  unicode_spaces: 'em\u2003space\u3000end',
  combining_stack: 'a\u0301\u0323 e\u0302\u0303',
  general_punct: '\u2010 \u2023 \u203b \u2030 \u2015',
  dingbats: '\u2702 \u270d \u270e \u2713',
  emoji_run: '🎉🎊🎈🎁🚀',
  emoji_modifier_alone: '🏻 🏿 ok',
  keycap_bare: '1\u20e3 #\u20e3',
  private_use: 'Icon \uf015 home',
  cjk_ext_b: '𠀀𡈽 text',
  kana_archaic: 'ア𛀁ゟ',
  open_sans_ranges: 'na\u00efve \u02d8 \u2021 x\u00b2',
  bidi_controls: 'a\u202aemb\u202c \u2066iso\u2069',
  number_forms: '\u2160 \u2161 \u2153',
  cyrillic_opsz: '\u0524 \u052f \u0500',
};

// Fallback text under `monospace` alone: Menlo's gaps go to the standard
// font (Times New Roman) before CoreText's cascade.
const MONOSPACE_STRINGS = {
  mono_hebrew: 'שלום עולם',
  mono_arabic: 'مرحبا',
  mono_latin_ext: '\u01c5 \u2113 \u2160 \u1ebf',
  mono_symbols: '┌─┐ ☕',
};

// Emoji at sizes outside SIZES: the bitmap advance and box do not scale
// with the font size (mermaid draws titles at 20 px and more).
const EMOJI_SIZE_STRINGS = {
  emoji_sized: '月別 📈 🇰🇷',
};
const EMOJI_SIZES = [10, 20, 24, 32];

const FAMILIES = {
  mermaid: '"trebuchet ms", verdana, arial, sans-serif',
  open_sans: '"Open Sans", sans-serif',
  generic: 'sans-serif',
};

const SIZES = [12, 14, 16];
const VARIANTS = [
  { weight: 400, style: 'normal' },
  { weight: 700, style: 'normal' },
  { weight: 400, style: 'italic' },
];

const WRAPPED = [
  '[the docs](https://example.com/path) and more \u2014 words\u2026 here',
  'This is a fairly long node label that the browser has to wrap onto several lines',
  'Supercalifragilisticexpialidocious words and short ones mixed in one label',
];
const WRAP_MAX_WIDTH = 200;
// CJK labels break between any two characters, so a kerned pair decides
// where the line ends. 175 px is the kanban card's label width.
const WRAPPED_CJK = [
  'ログイン画面のバグを直す',
  'ダークモードとハイコントラストのテーマを追加する',
  'やまだはなこさんとさとういちろうさん',
  '한국어 라벨을 여러 줄로 나누는 테스트입니다',
  'チェックリスト「テスト」、ショッピング。ジャンプ・ラーメン',
  'テストケース（括弧）を追加しました。次の行へ続きます',
  'Mixed かな and English テキストの折り返し',
];
const WRAP_CJK_MAX_WIDTH = 175;
// The weights the face check asks for: Hiragino Sans and PingFang have a
// face per CSS weight, matched on CoreText's weight, not OS/2's.
const FACE_WEIGHTS = [100, 300, 400, 500, 600, 700, 900];

// The sweep corpus (see the usage note at the top). Invisible and control
// characters are written as escapes.
const SWEEP_STRINGS = {
  ascii_word: 'Node label',
  ascii_long: 'A rather long label with many words to measure in one line of text',
  latin1_sup: 'Ångström Æsir façade ¿Qué? ¡Sí! «guillemets» ß þ ð',
  latin_ext_a: 'Ĳssel Œuvre ŧ ŀ Ŋ ĝĥĵŝ Čeština Ğğ İı Şş',
  latin_ext_b: 'ƀ Ɓ Ƈ ǅ ǈ ǋ Ǳ ș ț Ȣ ȴ ɂ',
  latin_ext_add: 'Tiếng Việt ạ ả ấ ầ ẩ ẫ ậ ḍ ṃ ṇ ṛ ṣ ṭ ẞ',
  vietnamese: 'Xin chào thế giới, đây là nhãn',
  ipa: 'ɪntərˈnæʃənəl fəˈnɛtɪk ʃ ʒ ŋ ɾ ʔ',
  spacing_mod: 'ʰ ʲ ʷ ˈ ˌ ː ˑ ˘ ˙ ˚ ˛ ˜ ˝',
  combining_stack: 'ạ́ ễ ȫ Z̵̶ q̣̱̇',
  combining_zalgo: 'Z͑ͫ̓ͪ̂ͫ̽͏̴̙̤̞͉͚̯̞̠͍a',
  combining_alone: '́̈ x',
  greek: 'Ελληνικά κείμενο ΑΒΓΔ αβγδ',
  greek_ext: 'ἀρχὴ ᾅδης ὦ ῥ ἐν ἀρχῇ ἦν ὁ λόγος',
  greek_coptic: 'ϐ ϑ ϒ ϕ ϖ Ϛ Ϝ Ϟ Ϡ ϰ ϱ ϲ',
  cyrillic: 'Съешь же ещё этих мягких булок',
  cyrillic_sup: 'Ԁ ԁ Ԃ ԃ Ԑ ԑ Ԥ ԥ ԯ',
  cyrillic_ext: 'Ꙁ ꙁ Ꙋ ꙋ ᲀ ᲁ ᲂ ⷠ',
  ukrainian: 'Ґанок їжак є Ї',
  armenian: 'Բարեւ աշխարհ Հայերեն',
  georgian: 'გამარჯობა მსოფლიო ᲐᲑᲒ',
  hebrew: 'עברית פשוטה',
  hebrew_points: 'בְּרֵאשִׁית בָּרָא אֱלֹהִים',
  arabic: 'السلام عليكم',
  arabic_harakat: 'بِسْمِ ٱللَّٰهِ ٱلرَّحْمَٰنِ',
  arabic_pres: 'ﷺ ﷲ ﻻ ﻼ ﺍﺏ',
  arabic_digits: 'رقم ١٢٣٤٥ و 678',
  persian: 'سلام دنیا گچپژ',
  urdu: 'اردو زبان ٹھیک',
  syriac: 'ܫܠܡܐ ܥܠܡܐ',
  thaana: 'ދިވެހި ބަސް',
  nko: 'ߒߞߏ ߓߊ߲ߊ',
  devanagari: 'हिन्दी भाषा क्षत्रिय',
  bengali: 'বাংলা ভাষা ক্ষমা',
  gurmukhi: 'ਪੰਜਾਬੀ ਭਾਸ਼ਾ',
  gujarati: 'ગુજરાતી ભાષા',
  oriya: 'ଓଡ଼ିଆ ଭାଷା',
  tamil: 'தமிழ் மொழி ஸ்ரீ',
  telugu: 'తెలుగు భాష',
  kannada: 'ಕನ್ನಡ ಭಾಷೆ',
  malayalam: 'മലയാളം ഭാഷ',
  sinhala: 'සිංහල භාෂාව',
  thai: 'ภาษาไทยง่ายนิดเดียว',
  thai_marks: 'ที่นี่ น้ำ ปั้ ฎ ฏ',
  lao: 'ພາສາລາວ',
  tibetan: 'བོད་ཡིག་ ཀྲ་ཤིས་',
  myanmar: 'မြန်မာဘာသာ',
  khmer: 'ភាសាខ្មែរ',
  mongolian: 'ᠮᠣᠩᠭᠣᠯ ᠬᠡᠯᠡ',
  ethiopic: 'ሰላም ዓለም ኢትዮጵያ',
  cherokee: 'ᏣᎳᎩ ᎦᏬᏂᎯᏍᏗ',
  canadian: 'ᐃᓄᒃᑎᑐᑦ ᑕᑯᑦ',
  runic: 'ᚠᚢᚦᚨᚱᚲ ᛗᛁᚾ',
  ogham: '᚛ᚑᚌᚐᚋ᚜',
  hangul: '안녕하세요 세계',
  hangul_jamo: 'ᄀᄁᄂ ᅡᅢᅣ ᆨᆩᆪ',
  hangul_compat: 'ㄱ ㄴ ㄷ ㅏ ㅑ ㅢ',
  hangul_jamo_seq: '각 한',
  han: '漢字測試標籤',
  han_simplified: '简体中文标签测试',
  cjk_punct: '「引用」『二重』、。・〜〈〉《》【】',
  cjk_symbols: '〒 〓 〆 々 〇 〻 ㍿ ㈱ ㊗',
  fullwidth: 'ＡＢＣ　ａｂｃ　１２３！？',
  halfwidth: 'ｱｲｳｴｵ ﾊﾟﾋﾟ ｶﾞｷﾞ ￠￡',
  cjk_ext_a: '㐀㐁㐂㑇㒀㓁',
  cjk_ext_b: '𠀀𠀁𠀂𡈽𢌌𤭢',
  cjk_compat: '豈 更 車 賈 滑',
  bopomofo: 'ㄅㄆㄇㄈ ㄓㄔㄕ',
  yi: 'ꆈꌠꁱꂷ ꀀꀁ',
  kana_halfwidth_mix: 'ｶﾀｶﾅとカタカナ',
  kana_small: 'ぁぃぅぇぉっゃゅょ ァィゥ',
  kana_ext: 'ㇰㇱㇲ 𛀁 ゟ ヿ',
  nbsp: 'non breaking space',
  nbsp_run: 'a   b',
  zwsp: 'zero​width​space',
  zwj_zwnj: 'zw‍j zw‌nj',
  soft_hyphen: 'soft­hyphen­ated',
  word_joiner: 'word⁠joiner﻿bom',
  bidi_marks: 'left‎right‏ mark',
  bidi_embed: 'a‪emb‬ b‫emb‬ c‭over‬',
  bidi_isolate: 'x ⁦iso⁩ ⁧עב⁩ ⁨fs⁩',
  spaces_unicode: 'en em thin hair fig punct ideo　x',
  narrow_nbsp: '10 km 5 000',
  general_punct: '‐ ‑ ‒ – — ― ‖ † ‡ • ‣ ‰ ‱ ′ ″ ‴ ※ ‼ ⁂ ⁇',
  quotes_all: '„low“ ‚single‘ ‹angle› «double» 〝cjk〟',
  superscripts: 'x² y³ z¹ ⁰⁴⁵⁶⁷⁸⁹ ⁺⁻⁼⁽⁾ ⁿ',
  subscripts: 'H₂O CO₂ ₀₁₂₃₄₅₆₇₈₉ ₊₋ ₐₑₒ',
  currency: '₠ ₡ ₢ ₣ ₤ ₥ ₦ ₧ ₨ ₪ ₫ ₭ ₮ ₱ ₲ ₴ ₵ ₸ ₺ ₼ ₽ ₾ ₿',
  letterlike: '℃ ℉ № ℗ ℞ ℠ ™ Ω ℮ ℵ ℏ ℓ ℝ ℕ ℤ',
  number_forms: '⅓ ⅔ ⅕ ⅛ Ⅰ Ⅱ Ⅲ Ⅳ Ⅻ ⅰ ⅱ ↉',
  fractions_slash: '1⁄2 3⁄4 ½ ¼ ¾',
  arrows: '← ↑ → ↓ ↔ ↕ ↖ ↩ ↪ ⇄ ⇐ ⇑ ⇔ ⇨ ⟶ ⟹ ⤴ ⤵',
  math_ops: '∀ ∂ ∃ ∅ ∇ ∈ ∉ ∏ ∑ − ∓ ∗ √ ∝ ∞ ∠ ∧ ∨ ∩ ∪ ∫ ≈ ≠ ≡ ⊂ ⊃ ⊕ ⊗',
  math_misc: '⟨ ⟩ ⟦ ⟧ ⦃ ⦄ ⧺ ⨀ ⨁ ⩽ ⩾',
  math_alnum: '𝐀𝐁𝐂 𝑥𝑦𝑧 𝔄𝔅 𝕒𝕓𝕔 𝓐𝓑 𝟘𝟙𝟚',
  technical: '⌘ ⌥ ⇧ ⌃ ⌫ ⌦ ⏎ ⏏ ⌚ ⌛ ⏰ ⏳ ⌨',
  control_pictures: '␀ ␉ ␊ ␍ ␛ ␠ ␡ ␤',
  enclosed_alnum: '① ② ⑳ ⓐ ⓩ Ⓐ ⑴ ⒈ ⓪',
  box_drawing: '┌─┬─┐ │ ├─┼─┤ └─┴─┘ ═ ║ ╔ ╗',
  block_elements: '▀ ▄ █ ▌ ▐ ░ ▒ ▓ ▖ ▗ ▘ ▙',
  geometric: '■ □ ▪ ▫ ▲ △ ▶ ▷ ▼ ▽ ◀ ◁ ◆ ◇ ○ ◎ ● ◐ ◑',
  misc_symbols: '☀ ☁ ☂ ☃ ☄ ★ ☆ ☎ ☐ ☑ ☒ ☕ ☘ ☠ ☢ ☮ ☯ ♀ ♂ ♠ ♣ ♥ ♦ ♪ ♫ ⚑ ⚙',
  misc_symbols_emoji: '☺ ☹ ♻ ⚠ ⚡ ⚽ ⛄ ⛔ ⛪',
  dingbats: '✂ ✈ ✉ ✍ ✎ ✏ ✒ ✓ ✔ ✕ ✖ ✗ ✘ ✝ ✡ ✦ ✧ ✩ ❄ ❖ ❤ ➔ ➜ ➤',
  dingbats_emoji: '✅ ✨ ❌ ❎ ❓ ❗ ➕ ➖ ➗ ➰ ➿',
  braille: '⠁⠃⠉⠙⠑ ⠿ ⣿ ⡀',
  musical: '𝄞 𝄢 𝅘𝅥𝅮 𝄪 ♭ ♮ ♯',
  misc_tech_sym: '⏻ ⏼ ⏽ ⭘ ⭐ ⬛ ⬜ ⬆ ⮕',
  emoji_faces: '😀 😂 🥲 🫠 🤖 👻',
  emoji_animals: '🐶 🐱 🦊 🐙 🦄 🐝',
  emoji_objects: '📦 🔧 🚀 💡 📊 🗂',
  emoji_keycap: '1️⃣ 2️⃣ #️⃣ *️⃣ 🔟',
  emoji_keycap_bare: '1⃣ #⃣',
  emoji_tag_flag: '🏴󠁧󠁢󠁥󠁮󠁧󠁿 🏴󠁧󠁢󠁳󠁣󠁴󠁿 🏴󠁧󠁢󠁷󠁬󠁳󠁿',
  emoji_flags: '🇺🇸 🇬🇧 🇩🇪 🇫🇷 🇪🇺 🇺🇳',
  emoji_flag_bad: '🇦 🇿🇿',
  emoji_zwj_family: '👨‍👩‍👧 👩‍👩‍👦‍👦 🧑‍🤝‍🧑',
  emoji_zwj_prof: '👩‍💻 🧑‍🚀 👨‍🍳 🏃‍♀️ 🏳️‍🌈 🐻‍❄️',
  emoji_zwj_new: '🐦‍🔥 🍋‍🟩 ❤️‍🔥 ❤️‍🩹 🙂‍↔️',
  emoji_skin_tones: '👋🏻 👋🏼 👋🏽 👋🏾 👋🏿 🧑🏾‍💻',
  emoji_components: '🏻 🏿 🦰 🦱 🦳 🦲',
  emoji_vs15: '☀︎ ☁︎ ❤︎ ✔︎ ⚠︎',
  emoji_vs16: '☀️ ☁️ ✔️ ⚠️ ↔️ ©️ ®️ ™️',
  emoji_text_default: '© ® ™ ↔ ‼ ⁉ ℹ Ⓜ',
  emoji_pairs: '🤝 💑 👯 🫶',
  emoji_dense: '🎉🎊🎈🎁🎀🎂🍰🧁🥳🎆🎇✨',
  emoji_latin: 'Release v2 🚀 shipped ✅ today',
  pua: '',
  pua_nerd: '  ',
  pua_plane15: '\u{f0000}\u{f0001}',
  unassigned: 'a͸b΀c',
  unassigned_astral: 'x\u{2fffd}y',
  noncharacters: 'a￾b￿c﷐',
  replacement: 'bad � char',
  tab: 'col1\tcol2',
  ws_double: 'two  spaces',
  ws_lead: '  leading',
  ws_trail: 'trailing  ',
  ws_newline: 'new\nline',
  ws_mixed: ' a \t\n b ',
  ws_nbsp_edges: ' edge ',
  ws_ideographic_edges: '　edge　',
  controls: 'a\u0001b\u0007c\u001bd',
  c1_controls: 'a\u0085b\u009fc',
  del: 'a\u007fb',
  linefeed_cr: 'line\u000done',
  long_cyrillic: 'Однажды весною в час небывало жаркого заката в Москве на Патриарших прудах появились два гражданина',
  long_han: '天地玄黃宇宙洪荒日月盈昃辰宿列張寒來暑往秋收冬藏閏餘成歲律呂調陽雲騰致雨露結為霜',
  long_arabic: 'هذا نص طويل باللغة العربية لاختبار القياس في سطر واحد بدون التفاف',
  long_hangul: '다람쥐 헌 쳇바퀴에 타고파 키스의 고유조건은 입술끼리 만나야 하고',
  long_devanagari: 'ऋषियों को सताने वाले दुष्ट राक्षसों के राजा रावण का सर्वनाश करने वाले',
  long_thai: 'เป็นมนุษย์สุดประเสริฐเลิศคุณค่ากว่าบรรดาฝูงสัตว์เดรัจฉาน',
  long_emoji: '😀😃😄😁😆😅🤣😂🙂🙃😉😊😇🥰😍🤩😘😗',
  mixed_scripts: 'Ünïcødé Ελλάδα Россия 日本 한국 العربية',
  mixed_rtl_ltr: 'Hello שלום world עולם 123',
  mixed_rtl_digits: 'עמוד 12 מתוך 345',
  mixed_arabic_latin: 'API مفتاح key 2024',
  mixed_cjk_latin_digits: '第3章 Chapter 3 第三章',
  mixed_emoji_rtl: 'שלום 👋 עולם',
  mixed_symbols_text: 'Price: €10 → $12 (≈ 20%)',
  mixed_indic_latin: 'Hindi हिन्दी and Tamil தமிழ்',
  mixed_thai_latin: 'Thai ภาษาไทย text',
  mixed_kana_emoji: 'テスト✅完了🎉',
  multi_scripts: 'Ελληνικά<br>Русский<br>日本語',
  multi_rtl: 'שלום<br>مرحبا',
  multi_emoji: '🚀 launch<br>✅ done<br>🎉',
  multi_indic: 'हिन्दी<br>বাংলা',
  multi_long: 'First line with Ünicode<br>二行目のテキスト<br>third',
  multi_combining: 'é́<br>ạ̣',
  multi_empty_line: 'top<br><br>bottom',
  br_variants: 'one<br/>two<br />three',
};

// The families of the label kinds: the trebuchet chain of most diagrams,
// Open Sans of sequence, journey, timeline, and C4, and the generics.
const SWEEP_FAMILIES = {
  mermaid: '"trebuchet ms", verdana, arial, sans-serif',
  open_sans: '"Open Sans", sans-serif',
  arial: 'arial',
  sans_serif: 'sans-serif',
  monospace: 'monospace',
};

const RUN = sweep
  ? {
      STRINGS: SWEEP_STRINGS,
      MONOSPACE_STRINGS: {},
      EMOJI_SIZE_STRINGS: {},
      EMOJI_SIZES,
      FAMILIES: SWEEP_FAMILIES,
      SIZES: [14, 16],
      VARIANTS: [
        { weight: 400, style: 'normal' },
        { weight: 700, style: 'normal' },
      ],
      WRAPPED: [],
      WRAP_MAX_WIDTH,
      WRAPPED_CJK: [],
      WRAP_CJK_MAX_WIDTH,
      FACE_VARIANTS: [
        { weight: 400, style: 'normal' },
        { weight: 700, style: 'normal' },
      ],
    }
  : {
      STRINGS,
      MONOSPACE_STRINGS,
      EMOJI_SIZE_STRINGS,
      EMOJI_SIZES,
      FAMILIES,
      SIZES,
      VARIANTS,
      WRAPPED,
      WRAP_MAX_WIDTH,
      WRAPPED_CJK,
      WRAP_CJK_MAX_WIDTH,
      FACE_VARIANTS: [
        ...FACE_WEIGHTS.map((weight) => ({ weight, style: 'normal' })),
        { weight: 400, style: 'italic' },
        { weight: 700, style: 'italic' },
      ],
    };

// mermaid-cli injects these from its own node_modules; the fonts are loaded
// through file:// here, so Chrome needs --allow-file-access-from-files.
const CSS = [
  '@fontsource-variable/recursive/index.css',
  '@fontsource/open-sans/400.css',
  '@fontsource/open-sans/400-italic.css',
].map((p) => url.pathToFileURL(require.resolve(p)).href);

const puppeteerConfig = JSON.parse(fs.readFileSync(path.join(here, 'puppeteer.json'), 'utf8'));
// The package's exports map hides dist/, so walk from its entry point.
const pageUrl = url.pathToFileURL(
  path.join(path.dirname(require.resolve('@mermaid-js/mermaid-cli')), '..', 'dist', 'index.html')
).href;

const browser = await puppeteer.launch({
  ...puppeteerConfig,
  args: [...(puppeteerConfig.args ?? []), '--allow-file-access-from-files'],
});
try {
  const page = await browser.newPage();
  await page.goto(pageUrl);
  for (const href of CSS) {
    await page.evaluate((href) => {
      const link = document.createElement('link');
      link.setAttribute('crossorigin', 'anonymous');
      link.setAttribute('rel', 'stylesheet');
      link.setAttribute('href', href);
      const loaded = new Promise((resolve, reject) => {
        link.onload = () => resolve();
        link.onerror = () => reject(new Error(`Failed to load stylesheet: ${href}`));
      });
      document.head.appendChild(link);
      return loaded;
    }, href);
  }
  // mermaid's root stylesheet sets `p { margin: 0 }`; HTML labels wrap their
  // text in a <p>, so the rule decides the measured height.
  await page.addStyleTag({ content: 'p { margin: 0 }' });
  await page.evaluate(() => Promise.all(Array.from(document.fonts, (font) => font.load())));

  const result = await page.evaluate(
    ({
      STRINGS,
      MONOSPACE_STRINGS,
      EMOJI_SIZE_STRINGS,
      EMOJI_SIZES,
      FAMILIES,
      SIZES,
      VARIANTS,
      WRAPPED,
      WRAP_MAX_WIDTH,
      WRAPPED_CJK,
      WRAP_CJK_MAX_WIDTH,
    }) => {
      const SVG_NS = 'http://www.w3.org/2000/svg';
      const ZERO_WIDTH_SPACE = '​';
      const body = document.body;

      const svg = document.createElementNS(SVG_NS, 'svg');
      svg.setAttribute('width', '2000');
      svg.setAttribute('height', '2000');
      body.appendChild(svg);

      function font(el, family, size, weight, style) {
        el.style.fontFamily = family;
        el.style.fontSize = `${size}px`;
        el.style.fontWeight = String(weight);
        el.style.fontStyle = style;
      }

      // utils.ts drawSimpleText: <text> with one <tspan x=0>, measured with getBBox.
      function measureSvg(text, family, size, weight, style) {
        const textEl = document.createElementNS(SVG_NS, 'text');
        textEl.setAttribute('x', '0');
        textEl.setAttribute('y', '0');
        textEl.style.textAnchor = 'start';
        font(textEl, family, size, weight, style);
        const span = document.createElementNS(SVG_NS, 'tspan');
        span.setAttribute('x', '0');
        // drawSimpleText replaces every `<br>` with a space.
        span.textContent = text.replace(/<br\s*\/?>/gi, ' ') || ZERO_WIDTH_SPACE;
        textEl.appendChild(span);
        svg.appendChild(textEl);
        const bbox = textEl.getBBox();
        const textLength = span.getComputedTextLength();
        svg.removeChild(textEl);
        return { width: bbox.width, height: bbox.height, text_length: textLength };
      }

      // createText.ts addHtmlSpan: foreignObject > div(table-cell, nowrap,
      // line-height 1.5) > span > p (from markdownToHTML).
      function htmlLabel(html, family, size, weight, style, width) {
        const fo = document.createElementNS(SVG_NS, 'foreignObject');
        fo.setAttribute('width', `${Math.min(10 * width, 16384)}px`);
        fo.setAttribute('height', `${Math.min(10 * width, 16384)}px`);
        const div = document.createElementNS('http://www.w3.org/1999/xhtml', 'div');
        const span = document.createElement('span');
        span.innerHTML = html;
        span.className = 'nodeLabel';
        div.appendChild(span);
        div.style.display = 'table-cell';
        div.style.whiteSpace = 'nowrap';
        div.style.lineHeight = '1.5';
        if (width !== Number.POSITIVE_INFINITY) {
          div.style.maxWidth = `${width}px`;
          div.style.textAlign = 'center';
        }
        font(div, family, size, weight, style);
        fo.appendChild(div);
        svg.appendChild(fo);
        return { fo, div, span };
      }

      function measureHtml(text, family, size, weight, style) {
        const html = text ? `<p>${text}</p>` : '';
        const { fo, div } = htmlLabel(html, family, size, weight, style, Number.POSITIVE_INFINITY);
        const r = div.getBoundingClientRect();
        svg.removeChild(fo);
        return { width: r.width, height: r.height };
      }

      // The lines Chrome produced, read back from per-character client rects.
      function readLines(p) {
        const node = p.firstChild;
        const text = node.textContent;
        const lines = [];
        let lastTop = null;
        for (let i = 0; i < text.length; i++) {
          const range = document.createRange();
          range.setStart(node, i);
          range.setEnd(node, i + 1);
          const rects = range.getClientRects();
          if (rects.length === 0) {
            continue;
          }
          const top = Math.round(rects[0].top * 100) / 100;
          if (lastTop === null || Math.abs(top - lastTop) > 0.5) {
            lines.push('');
            lastTop = top;
          }
          lines[lines.length - 1] += text[i];
        }
        return lines.map((l) => l.trim());
      }

      function measureWrapped(text, family, size, weight, style, width) {
        const html = `<p>${text}</p>`;
        const { fo, div, span } = htmlLabel(html, family, size, weight, style, width);
        let r = div.getBoundingClientRect();
        const nowrap = { width: r.width, height: r.height };
        if (r.width === width) {
          div.style.display = 'table';
          div.style.whiteSpace = 'break-spaces';
          div.style.width = `${width}px`;
          r = div.getBoundingClientRect();
        }
        const lines = readLines(span.firstChild);
        svg.removeChild(fo);
        return { nowrap, width: r.width, height: r.height, lines };
      }

      // A family resolved when it measures differently from the generic it
      // would fall back to.
      function available(family) {
        const probe = 'The quick brown fox 0123456789';
        const a = measureSvg(probe, `"${family}", monospace`, 16, 400, 'normal').width;
        const b = measureSvg(probe, 'monospace', 16, 400, 'normal').width;
        return a !== b;
      }
      // Arial and Helvetica share advance widths, so compare heights too.
      function sameFamily(a, b) {
        const probe = 'The quick brown fox 0123456789 AVAWAY';
        const ma = measureSvg(probe, a, 16, 400, 'normal');
        const mb = measureSvg(probe, b, 16, 400, 'normal');
        return ma.width === mb.width && ma.height === mb.height;
      }

      const fonts = {};
      for (const f of ['Trebuchet MS', 'Verdana', 'Arial', 'Open Sans', 'Helvetica']) {
        fonts[f] = available(f);
      }
      const generics = {
        'sans-serif': ['Helvetica', 'Arial', 'DejaVu Sans', 'Liberation Sans'].find((f) =>
          sameFamily('sans-serif', `"${f}"`)
        ),
        serif: ['Times', 'Times New Roman', 'DejaVu Serif', 'Liberation Serif'].find((f) =>
          sameFamily('serif', `"${f}"`)
        ),
        monospace: ['Courier', 'Courier New', 'Menlo', 'DejaVu Sans Mono', 'Liberation Mono'].find(
          (f) => sameFamily('monospace', `"${f}"`)
        ),
      };

      const cases = [];
      const groups = [
        ...Object.entries(FAMILIES).map(([familyId, family]) => [familyId, family, STRINGS, SIZES]),
        ['monospace', 'monospace', MONOSPACE_STRINGS, SIZES],
        ['mermaid', FAMILIES.mermaid, EMOJI_SIZE_STRINGS, EMOJI_SIZES],
      ];
      for (const [familyId, family, strings, sizes] of groups) {
        for (const size of sizes) {
          for (const { weight, style } of VARIANTS) {
            for (const [textId, text] of Object.entries(strings)) {
              cases.push({
                id: `${textId}/${familyId}/${size}/${weight}/${style}`,
                text,
                family,
                size,
                weight,
                style,
                svg: measureSvg(text, family, size, weight, style),
                html: measureHtml(text, family, size, weight, style),
              });
            }
          }
        }
      }

      const wrapped = [];
      for (const [familyId, family] of Object.entries(FAMILIES)) {
        for (const size of SIZES) {
          for (const text of WRAPPED) {
            wrapped.push({
              id: `${familyId}/${size}`,
              text,
              family,
              size,
              weight: 400,
              style: 'normal',
              max_width: WRAP_MAX_WIDTH,
              ...measureWrapped(text, family, size, 400, 'normal', WRAP_MAX_WIDTH),
            });
          }
          for (const { weight, style } of VARIANTS) {
            for (const text of WRAPPED_CJK) {
              wrapped.push({
                id: `cjk/${familyId}/${size}/${weight}/${style}`,
                text,
                family,
                size,
                weight,
                style,
                max_width: WRAP_CJK_MAX_WIDTH,
                ...measureWrapped(text, family, size, weight, style, WRAP_CJK_MAX_WIDTH),
              });
            }
          }
        }
      }

      // break-spaces probe: a line whose text fits but whose trailing space
      // does not. Reports where the next line starts so the port can tell
      // whether Chrome wraps the space onto the next line or hangs it.
      const probeFamily = FAMILIES.mermaid;
      const probeSize = 16;
      const w = (t) => measureSvg(t, probeFamily, probeSize, 400, 'normal').text_length;
      const longWord = 'Supercalifragilisticexpialidocious';
      const avail = w(longWord);
      let n = 1;
      while (w('i'.repeat(n + 1)) <= avail) {
        n++;
      }
      const probeText = `${longWord} ${'i'.repeat(n)} next end`;
      const { fo, div, span } = htmlLabel(
        `<p>${probeText}</p>`,
        probeFamily,
        probeSize,
        400,
        'normal',
        WRAP_MAX_WIDTH
      );
      div.style.display = 'table';
      div.style.whiteSpace = 'break-spaces';
      div.style.width = `${WRAP_MAX_WIDTH}px`;
      const divRect = div.getBoundingClientRect();
      const node = span.firstChild.firstChild;
      const lineStarts = [];
      let lastTop = null;
      for (let i = 0; i < node.textContent.length; i++) {
        const range = document.createRange();
        range.setStart(node, i);
        range.setEnd(node, i + 1);
        const rect = range.getClientRects()[0];
        if (!rect) {
          continue;
        }
        if (lastTop === null || Math.abs(rect.top - lastTop) > 0.5) {
          lineStarts.push({ char: node.textContent[i], left: rect.left - divRect.left });
          lastTop = rect.top;
        }
      }
      svg.removeChild(fo);
      const probe = {
        text: probeText,
        family: probeFamily,
        size: probeSize,
        avail,
        fits: w('i'.repeat(n)),
        fits_with_space: w(`${'i'.repeat(n)} `),
        width: divRect.width,
        height: divRect.height,
        line_starts: lineStarts,
        next_end_width: w('next end'),
        space_width: w('i i') - w('ii'),
      };

      body.removeChild(svg);
      return { fonts, generics, cases, wrapped, probe };
    },
    RUN
  );

  // The face Chrome draws each grapheme of a fallback string in, read off
  // CSS.getPlatformFontsForNode with one span per grapheme. The measurer's
  // fallback choice is checked against these names before any width.
  const cdp = await page.createCDPSession();
  await cdp.send('DOM.enable');
  await cdp.send('CSS.enable');
  const faces = [];
  for (const [textId, raw] of Object.entries(RUN.STRINGS)) {
    const text = raw.replace(/<br\s*\/?>/gi, ' ');
    if (/^[\x00-\x7f]*$/.test(text)) {
      continue;
    }
    for (const [familyId, family] of Object.entries(RUN.FAMILIES)) {
      for (const { weight, style } of RUN.FACE_VARIANTS) {
        const graphemes = await page.evaluate(
          ({ text, family, weight, style }) => {
            const holder = document.createElement('div');
            holder.id = 'faces';
            const segmenter = new Intl.Segmenter(undefined, { granularity: 'grapheme' });
            const out = [];
            for (const { segment } of segmenter.segment(text)) {
              const span = document.createElement('span');
              span.style.fontFamily = family;
              span.style.fontSize = '16px';
              span.style.fontWeight = String(weight);
              span.style.fontStyle = style;
              span.textContent = segment;
              holder.appendChild(span);
              out.push(segment);
            }
            document.body.appendChild(holder);
            void holder.offsetHeight;
            return out;
          },
          { text, family, weight, style }
        );
        const { root } = await cdp.send('DOM.getDocument', { depth: -1 });
        const { nodeIds } = await cdp.send('DOM.querySelectorAll', {
          nodeId: root.nodeId,
          selector: '#faces > span',
        });
        const perGrapheme = [];
        for (let i = 0; i < graphemes.length; i++) {
          if (/^\s$/.test(graphemes[i])) {
            continue;
          }
          const { fonts } = await cdp.send('CSS.getPlatformFontsForNode', { nodeId: nodeIds[i] });
          perGrapheme.push({
            grapheme: graphemes[i],
            fonts: fonts.map((f) => f.familyName),
            postscript: fonts.map((f) => f.postScriptName),
          });
        }
        await page.evaluate(() => document.getElementById('faces').remove());
        faces.push({ id: `${textId}/${familyId}/${weight}/${style}`, family, weight, style, graphemes: perGrapheme });
      }
    }
  }
  result.faces = faces;

  // Read package.json directly: some exports maps hide it from require.
  const version = (name) =>
    JSON.parse(fs.readFileSync(path.join(here, 'node_modules', name, 'package.json'), 'utf8'))
      .version;
  const metrics = {
    generated_by: 'helper/oracle/measure.mjs',
    platform: process.platform,
    chrome: await browser.version(),
    puppeteer: version('puppeteer'),
    mermaid_cli: version('@mermaid-js/mermaid-cli'),
    mermaid: version('mermaid'),
    fontsource_open_sans: version('@fontsource/open-sans'),
    ...result,
  };
  fs.mkdirSync(path.dirname(out), { recursive: true });
  fs.writeFileSync(out, JSON.stringify(metrics, null, 1) + '\n');
  console.log(`wrote ${metrics.cases.length} cases and ${metrics.wrapped.length} wrapped cases to ${out}`);
} finally {
  await browser.close();
}
