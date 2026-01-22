#!/usr/bin/env python3
"""
Wikipedia日本語コーパス作成スクリプト
10万字のひらがなコーパスを生成する

使用方法:
1. WikiExtractorで抽出済みのtextフォルダがある場合:
   python create_corpus.py --input text/ --output corpus_100k.txt

2. wiki.txtがある場合:
   python create_corpus.py --input wiki.txt --output corpus_100k.txt

3. URLから直接ダウンロードする場合（時間がかかる）:
   python create_corpus.py --download --output corpus_100k.txt
"""

import argparse
import os
import re
import sys
import random
from pathlib import Path

# MeCabまたはfugashiを使用
try:
    import MeCab
    USE_MECAB = True
except ImportError:
    try:
        from fugashi import Tagger
        USE_MECAB = False
    except ImportError:
        print("MeCabまたはfugashiをインストールしてください")
        print("pip install mecab-python3 unidic-lite")
        print("または")
        print("pip install fugashi unidic-lite")
        sys.exit(1)


def clean_text(text):
    """テキストのクリーニング"""
    # XMLタグを削除
    text = re.sub(r'<[^>]+>', '', text)
    # 括弧内の注釈を削除
    text = re.sub(r'（[^）]*）', '', text)
    text = re.sub(r'\([^)]*\)', '', text)
    # 特殊文字を削除
    text = re.sub(r'[【】『』「」《》〈〉［］\[\]｛｝{}]', '', text)
    # 連続する空白を1つに
    text = re.sub(r'\s+', ' ', text)
    # 数字を削除
    text = re.sub(r'[0-9０-９]+', '', text)
    # アルファベットを削除
    text = re.sub(r'[a-zA-Zａ-ｚＡ-Ｚ]+', '', text)
    return text.strip()


def text_to_hiragana(text, tagger):
    """テキストをひらがなに変換"""
    result = []
    
    if USE_MECAB:
        node = tagger.parseToNode(text)
        while node:
            if node.feature:
                features = node.feature.split(',')
                # 読みがある場合はカタカナ→ひらがな変換
                if len(features) > 7 and features[7] != '*':
                    reading = features[7]
                    # カタカナをひらがなに変換
                    hiragana = ''.join(
                        chr(ord(c) - 0x60) if 'ァ' <= c <= 'ン' else c
                        for c in reading
                    )
                    result.append(hiragana)
                elif node.surface:
                    # 読みがない場合はそのまま（記号など）
                    # ひらがな・カタカナ・句読点のみ保持
                    surface = node.surface
                    filtered = ''.join(
                        c for c in surface
                        if '\u3040' <= c <= '\u309f'  # ひらがな
                        or '\u30a0' <= c <= '\u30ff'  # カタカナ
                        or c in '。、'
                    )
                    if filtered:
                        # カタカナをひらがなに
                        hiragana = ''.join(
                            chr(ord(c) - 0x60) if 'ァ' <= c <= 'ン' else c
                            for c in filtered
                        )
                        result.append(hiragana)
            node = node.next
    else:
        # fugashi使用
        for word in tagger(text):
            if hasattr(word, 'feature') and word.feature.kana:
                reading = word.feature.kana
                hiragana = ''.join(
                    chr(ord(c) - 0x60) if 'ァ' <= c <= 'ン' else c
                    for c in reading
                )
                result.append(hiragana)
            elif word.surface:
                surface = word.surface
                filtered = ''.join(
                    c for c in surface
                    if '\u3040' <= c <= '\u309f' or '\u30a0' <= c <= '\u30ff' or c in '。、'
                )
                if filtered:
                    hiragana = ''.join(
                        chr(ord(c) - 0x60) if 'ァ' <= c <= 'ン' else c
                        for c in filtered
                    )
                    result.append(hiragana)
    
    return ''.join(result)


def filter_hiragana(text):
    """ひらがなと句読点のみを抽出"""
    return ''.join(
        c for c in text
        if '\u3040' <= c <= '\u309f' or c in '。、ー'
    )


def read_wiki_files(input_path):
    """WikiExtractorの出力またはwiki.txtを読み込む"""
    input_path = Path(input_path)
    
    if input_path.is_file():
        # 単一ファイル
        print(f"ファイルを読み込み中: {input_path}")
        with open(input_path, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith('<'):
                    yield line
    elif input_path.is_dir():
        # ディレクトリ（WikiExtractor出力）
        wiki_files = sorted(input_path.rglob('wiki_*'))
        print(f"ファイル数: {len(wiki_files)}")
        for wiki_file in wiki_files:
            with open(wiki_file, 'r', encoding='utf-8') as f:
                for line in f:
                    line = line.strip()
                    if line and not line.startswith('<'):
                        yield line


def create_corpus(input_path, output_path, target_chars=100000):
    """コーパスを作成"""
    print(f"目標文字数: {target_chars:,}字")
    
    # 形態素解析器の初期化
    if USE_MECAB:
        print("MeCabを使用")
        tagger = MeCab.Tagger()
    else:
        print("fugashiを使用")
        tagger = Tagger()
    
    corpus = []
    total_chars = 0
    processed_lines = 0
    
    for line in read_wiki_files(input_path):
        processed_lines += 1
        
        # テキストのクリーニング
        cleaned = clean_text(line)
        if len(cleaned) < 10:
            continue
        
        # ひらがなに変換
        hiragana = text_to_hiragana(cleaned, tagger)
        hiragana = filter_hiragana(hiragana)
        
        if len(hiragana) < 5:
            continue
        
        corpus.append(hiragana)
        total_chars += len(hiragana)
        
        if processed_lines % 10000 == 0:
            print(f"処理行数: {processed_lines:,}, 文字数: {total_chars:,}")
        
        if total_chars >= target_chars * 1.2:  # 少し多めに収集
            break
    
    print(f"\n収集完了: {len(corpus):,}行, {total_chars:,}文字")
    
    # シャッフルして目標文字数に調整
    random.shuffle(corpus)
    
    result = []
    char_count = 0
    for text in corpus:
        if char_count + len(text) > target_chars:
            # 残りの文字数分だけ追加
            remaining = target_chars - char_count
            if remaining > 0:
                result.append(text[:remaining])
            break
        result.append(text)
        char_count += len(text)
    
    # 出力
    output_text = '\n'.join(result)
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(output_text)
    
    final_chars = len(output_text.replace('\n', ''))
    print(f"\n出力: {output_path}")
    print(f"最終文字数: {final_chars:,}字")
    print(f"行数: {len(result):,}行")
    
    return output_path


def create_sample_corpus(output_path, target_chars=100000):
    """サンプルテキストからコーパスを作成（Wikipediaなしの場合）"""
    print("サンプルテキストからコーパスを生成します")
    
    # 青空文庫などから取得可能な著名なテキスト（著作権切れ）のサンプル
    sample_texts = [
        "吾輩は猫である。名前はまだ無い。どこで生れたかとんと見当がつかぬ。何でも薄暗いじめじめした所でニャーニャー泣いていた事だけは記憶している。",
        "国境の長いトンネルを抜けると雪国であった。夜の底が白くなった。信号所に汽車が止まった。",
        "メロスは激怒した。必ず、かの邪智暴虐の王を除かなければならぬと決意した。",
        "祇園精舎の鐘の声、諸行無常の響きあり。沙羅双樹の花の色、盛者必衰の理をあらはす。",
        "春はあけぼの。やうやう白くなりゆく山際、少し明かりて、紫だちたる雲の細くたなびきたる。",
        "いづれの御時にか、女御、更衣あまたさぶらひたまひけるなかに、いとやむごとなき際にはあらぬが、すぐれて時めきたまふありけり。",
        "人間が増えすぎた人口を宇宙に移民させるようになって、既に半世紀が過ぎていた。地球の周りの巨大な人工都市は人類の第二の故郷となり、人々はそこで子を産み、育て、そして死んでいった。",
    ]
    
    # 形態素解析器の初期化
    if USE_MECAB:
        tagger = MeCab.Tagger()
    else:
        tagger = Tagger()
    
    corpus = []
    for text in sample_texts:
        hiragana = text_to_hiragana(text, tagger)
        hiragana = filter_hiragana(hiragana)
        corpus.append(hiragana)
    
    # 目標文字数になるまで繰り返し
    result = []
    char_count = 0
    while char_count < target_chars:
        for text in corpus:
            if char_count >= target_chars:
                break
            result.append(text)
            char_count += len(text)
    
    output_text = '\n'.join(result)
    with open(output_path, 'w', encoding='utf-8') as f:
        f.write(output_text[:target_chars])
    
    print(f"出力: {output_path}")
    print(f"文字数: {min(len(output_text), target_chars):,}字")


def main():
    parser = argparse.ArgumentParser(description='Wikipedia日本語コーパス作成')
    parser.add_argument('--input', '-i', help='入力ファイル/ディレクトリ（wiki.txtまたはtextフォルダ）')
    parser.add_argument('--output', '-o', default='corpus_100k.txt', help='出力ファイル')
    parser.add_argument('--chars', '-c', type=int, default=100000, help='目標文字数（デフォルト: 100000）')
    parser.add_argument('--sample', action='store_true', help='サンプルテキストから生成')
    
    args = parser.parse_args()
    
    if args.sample:
        create_sample_corpus(args.output, args.chars)
    elif args.input:
        create_corpus(args.input, args.output, args.chars)
    else:
        print("使用方法:")
        print("  Wikipediaから: python create_corpus.py -i wiki.txt -o corpus_100k.txt")
        print("  サンプルから: python create_corpus.py --sample -o corpus_100k.txt")
        sys.exit(1)


if __name__ == '__main__':
    main()
