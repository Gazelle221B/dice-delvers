#!/bin/sh
# NotoSansJP-Subset.ttf の再生成手順。UI の日本語文言を変えたら実行する。
# 必要: pip install fonttools brotli、元フォント NotoSansJP[wght].ttf (https://github.com/google/fonts/tree/main/ofl/notosansjp)
# 使い方: sh subset-font.sh /path/to/NotoSansJP[wght].ttf
set -e
cd "$(dirname "$0")"
SRC="$1"
# src/ 内の全文字 + ひらがな・カタカナ・全角記号・ASCII を残す
python3 - "$SRC" <<'PY'
import sys, glob
from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools import subset
chars = set()
for f in glob.glob('../src/*.rs'):
    chars |= set(open(f, encoding='utf-8').read())
chars |= {chr(c) for c in range(0x20, 0x7f)}
chars |= {chr(c) for c in range(0x3000, 0x3100)}   # 全角記号・かな
chars |= {chr(c) for c in range(0xff01, 0xff60)}   # 全角英数
font = instancer.instantiateVariableFont(TTFont(sys.argv[1]), {"wght": 400})
opt = subset.Options()
opt.layout_features = []
opt.notdef_outline = True
opt.name_IDs = [0, 1, 2, 3, 4, 6, 13, 14]
sub = subset.Subsetter(opt)
sub.populate(text=''.join(sorted(chars)))
sub.subset(font)
font.save('NotoSansJP-Subset.ttf')
PY
ls -l NotoSansJP-Subset.ttf
