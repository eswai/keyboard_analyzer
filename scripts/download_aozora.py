#!/usr/bin/env python3
"""
青空文庫から100万字コーパスを作成するスクリプト
著作権切れのテキストを使用

使用方法:
    pip install mecab-python3 unidic-lite requests beautifulsoup4
    python scripts/download_aozora.py
"""

import os
import re
import sys
import time
import random
import zipfile
import io
from pathlib import Path

try:
    import requests
    from bs4 import BeautifulSoup
except ImportError:
    print("必要なライブラリをインストールしてください:")
    print("pip install requests beautifulsoup4")
    sys.exit(1)

try:
    import MeCab
    USE_MECAB = True
except ImportError:
    try:
        from fugashi import Tagger
        USE_MECAB = False
    except ImportError:
        print("MeCabまたはfugashiをインストールしてください:")
        print("pip install mecab-python3 unidic-lite")
        sys.exit(1)

# 青空文庫の著名な著者と作品（著作権切れ）
AOZORA_WORKS = [
    # 夏目漱石
    ("https://www.aozora.gr.jp/cards/000148/files/789_14547.html", "吾輩は猫である"),
    ("https://www.aozora.gr.jp/cards/000148/files/773_14560.html", "坊っちゃん"),
    ("https://www.aozora.gr.jp/cards/000148/files/776_14941.html", "こころ"),
    ("https://www.aozora.gr.jp/cards/000148/files/794_14946.html", "それから"),
    ("https://www.aozora.gr.jp/cards/000148/files/785_14964.html", "三四郎"),
    ("https://www.aozora.gr.jp/cards/000148/files/796_14965.html", "門"),
    # 芥川龍之介
    ("https://www.aozora.gr.jp/cards/000879/files/127_15260.html", "羅生門"),
    ("https://www.aozora.gr.jp/cards/000879/files/128_15261.html", "藪の中"),
    ("https://www.aozora.gr.jp/cards/000879/files/42_15228.html", "蜘蛛の糸"),
    ("https://www.aozora.gr.jp/cards/000879/files/3817_28040.html", "河童"),
    ("https://www.aozora.gr.jp/cards/000879/files/179_15255.html", "鼻"),
    # 太宰治
    ("https://www.aozora.gr.jp/cards/000035/files/301_14912.html", "走れメロス"),
    ("https://www.aozora.gr.jp/cards/000035/files/1567_14913.html", "人間失格"),
    ("https://www.aozora.gr.jp/cards/000035/files/2266_14929.html", "斜陽"),
    ("https://www.aozora.gr.jp/cards/000035/files/42620_21407.html", "津軽"),
    # 宮沢賢治
    ("https://www.aozora.gr.jp/cards/000081/files/456_15050.html", "銀河鉄道の夜"),
    ("https://www.aozora.gr.jp/cards/000081/files/473_42318.html", "風の又三郎"),
    ("https://www.aozora.gr.jp/cards/000081/files/464_15407.html", "注文の多い料理店"),
    # 森鷗外
    ("https://www.aozora.gr.jp/cards/000129/files/693_22804.html", "舞姫"),
    ("https://www.aozora.gr.jp/cards/000129/files/2556_26262.html", "高瀬舟"),
    ("https://www.aozora.gr.jp/cards/000129/files/689_42066.html", "山椒大夫"),
    # 樋口一葉
    ("https://www.aozora.gr.jp/cards/000064/files/388_15342.html", "たけくらべ"),
    ("https://www.aozora.gr.jp/cards/000064/files/392_19868.html", "にごりえ"),
    # 中島敦
    ("https://www.aozora.gr.jp/cards/000119/files/624_14544.html", "山月記"),
    ("https://www.aozora.gr.jp/cards/000119/files/621_14498.html", "李陵"),
    # 谷崎潤一郎
    ("https://www.aozora.gr.jp/cards/001383/files/56646_59172.html", "痴人の愛"),
    ("https://www.aozora.gr.jp/cards/001383/files/56640_58136.html", "春琴抄"),
    # 志賀直哉
    ("https://www.aozora.gr.jp/cards/000023/files/1689_25259.html", "暗夜行路"),
    ("https://www.aozora.gr.jp/cards/000023/files/235_42692.html", "城の崎にて"),
    # 川端康成
    ("https://www.aozora.gr.jp/cards/001475/files/52407_49805.html", "伊豆の踊子"),
    # 島崎藤村
    ("https://www.aozora.gr.jp/cards/000158/files/1497_26789.html", "破戒"),
    ("https://www.aozora.gr.jp/cards/000158/files/1501_24787.html", "夜明け前"),
    # 泉鏡花
    ("https://www.aozora.gr.jp/cards/000050/files/1031_22294.html", "高野聖"),
    ("https://www.aozora.gr.jp/cards/000050/files/4553_14228.html", "婦系図"),
    # 梶井基次郎
    ("https://www.aozora.gr.jp/cards/000074/files/424_19826.html", "檸檬"),
    # 堀辰雄
    ("https://www.aozora.gr.jp/cards/001030/files/4806_14279.html", "風立ちぬ"),
    # 織田作之助
    ("https://www.aozora.gr.jp/cards/000040/files/503_19843.html", "夫婦善哉"),
    # 国木田独歩
    ("https://www.aozora.gr.jp/cards/000038/files/329_15881.html", "武蔵野"),
    # 小泉八雲
    ("https://www.aozora.gr.jp/cards/000258/files/42926_16028.html", "怪談"),
    # 有島武郎
    ("https://www.aozora.gr.jp/cards/000025/files/200_20534.html", "或る女"),
    # 二葉亭四迷
    ("https://www.aozora.gr.jp/cards/000006/files/2212_19935.html", "浮雲"),
    # 正岡子規
    ("https://www.aozora.gr.jp/cards/000305/files/1898_18148.html", "病牀六尺"),
    # 横光利一
    ("https://www.aozora.gr.jp/cards/000168/files/2159_25208.html", "機械"),
    # 井伏鱒二
    ("https://www.aozora.gr.jp/cards/001864/files/57806_64427.html", "山椒魚"),
]


def clean_aozora_text(html_content):
    """青空文庫HTMLからテキストを抽出"""
    soup = BeautifulSoup(html_content, 'html.parser')
    
    # 本文を取得
    main_text = soup.find('div', class_='main_text')
    if not main_text:
        # 古い形式
        main_text = soup.find('body')
    
    if not main_text:
        return ""
    
    # ルビなどを処理
    for ruby in main_text.find_all('ruby'):
        # ルビを削除してテキストのみ残す
        rb = ruby.find('rb')
        if rb:
            ruby.replace_with(rb.get_text())
        else:
            # rbタグがない場合はruby全体のテキストから
            text = ruby.get_text()
            ruby.replace_with(text)
    
    # 注釈を削除
    for note in main_text.find_all(['span', 'div'], class_=['notes', 'bibliographical_information', 'notation_notes']):
        note.decompose()
    
    text = main_text.get_text()
    
    # クリーニング
    text = re.sub(r'［[^］]*］', '', text)  # 注記を削除
    text = re.sub(r'《[^》]*》', '', text)  # ルビ表記を削除
    text = re.sub(r'｜', '', text)  # ルビ開始記号
    text = re.sub(r'[#[^]]*]', '', text)  # 入力者注
    text = re.sub(r'\s+', '', text)  # 空白を削除
    
    return text


def text_to_hiragana(text, tagger):
    """テキストをひらがなに変換"""
    result = []
    
    if USE_MECAB:
        node = tagger.parseToNode(text)
        while node:
            if node.feature:
                features = node.feature.split(',')
                if len(features) > 7 and features[7] != '*':
                    reading = features[7]
                    hiragana = katakana_to_hiragana(reading)
                    result.append(hiragana)
                elif node.surface:
                    surface = node.surface
                    filtered = filter_kana(surface)
                    if filtered:
                        hiragana = katakana_to_hiragana(filtered)
                        result.append(hiragana)
            node = node.next
    else:
        for word in tagger(text):
            if hasattr(word, 'feature') and word.feature.kana:
                reading = word.feature.kana
                hiragana = katakana_to_hiragana(reading)
                result.append(hiragana)
            elif word.surface:
                filtered = filter_kana(word.surface)
                if filtered:
                    result.append(katakana_to_hiragana(filtered))
    
    return ''.join(result)


def katakana_to_hiragana(text):
    """カタカナをひらがなに変換"""
    return ''.join(
        chr(ord(c) - 0x60) if 'ァ' <= c <= 'ヶ' else c
        for c in text
    )


def filter_kana(text):
    """ひらがな・カタカナ・句読点のみを抽出"""
    return ''.join(
        c for c in text
        if '\u3040' <= c <= '\u309f'  # ひらがな
        or '\u30a0' <= c <= '\u30ff'  # カタカナ
        or c in '。、ー'
    )


def download_work(url, title):
    """作品をダウンロード"""
    try:
        headers = {
            'User-Agent': 'Mozilla/5.0 (compatible; CorpusBuilder/1.0)'
        }
        response = requests.get(url, headers=headers, timeout=30)
        response.encoding = 'shift_jis'
        return response.text
    except Exception as e:
        print(f"  警告: {title}のダウンロードに失敗: {e}")
        return None


def main():
    target_chars = 1_000_000
    output_path = Path(__file__).parent.parent / "corpus_1m.txt"
    
    print(f"青空文庫から{target_chars:,}字のコーパスを作成します")
    print(f"出力先: {output_path}")
    print()
    
    # 形態素解析器の初期化
    if USE_MECAB:
        print("MeCabを使用")
        tagger = MeCab.Tagger()
    else:
        print("fugashiを使用")
        tagger = Tagger()
    
    all_hiragana = []
    total_chars = 0
    
    # 作品をダウンロード・処理
    for i, (url, title) in enumerate(AOZORA_WORKS):
        if total_chars >= target_chars * 1.1:
            break
        
        print(f"[{i+1}/{len(AOZORA_WORKS)}] {title}をダウンロード中...")
        
        html = download_work(url, title)
        if not html:
            continue
        
        # テキスト抽出
        text = clean_aozora_text(html)
        if len(text) < 100:
            print(f"  警告: テキストが短すぎます ({len(text)}字)")
            continue
        
        # ひらがな変換
        hiragana = text_to_hiragana(text, tagger)
        hiragana = filter_kana(hiragana)
        
        if hiragana:
            all_hiragana.append(hiragana)
            total_chars += len(hiragana)
            print(f"  完了: {len(hiragana):,}字 (累計: {total_chars:,}字)")
        
        # サーバーに優しく
        time.sleep(1)
    
    print()
    print(f"収集完了: {total_chars:,}字")
    
    # 結合してシャッフル
    combined = '\n'.join(all_hiragana)
    
    # 目標文字数に調整
    if len(combined) > target_chars:
        combined = combined[:target_chars]
    
    # 保存
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(combined)
    
    final_chars = len(combined.replace('\n', ''))
    print(f"保存完了: {output_path}")
    print(f"最終文字数: {final_chars:,}字")
    
    return output_path


if __name__ == '__main__':
    main()
