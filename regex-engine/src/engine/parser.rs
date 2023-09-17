//! 正規表現の式をパースし、抽象構文木に変換。
use std::{
    error::Error,
    fmt::{self, Display},
    mem::take,
};

/// 抽象構文木を表現するための型。
#[derive(Debug)]
pub enum AST {
    Char(char),
    Plus(Box<AST>),
    Star(Box<AST>),
    Question(Box<AST>),
    Or(Box<AST>, Box<AST>),
    Seq(Vec<AST>),
}

/// パースエラーを表現するための型。
#[derive(Debug)]
pub enum ParseError {
    InvalidEscape(usize, char),
    InvalidRightParen(usize),
    NoPrev(usize),
    NoRightParen,
    Empty,
}

/// パースエラーの表示を実装。
impl Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidEscape(pos, c) => {
                write!(f, "ParseError: invalid escape: pos = {pos}, char = '{c}'",)
            }
            ParseError::InvalidRightParen(pos) => {
                write!(f, "ParseError: invalid right parenthesis: ps  = {pos}")
            }
            ParseError::NoPrev(pos) => write!(f, "ParseError: no previous expression: pos = {pos}"),
            ParseError::NoRightParen => write!(f, "ParseError: noright parenthesis"),
            ParseError::Empty => write!(f, "ParseError: empty expression"),
        }
    }
}

impl Error for ParseError {}

/// 特殊文字のエスケープ。
fn parse_escape(pos: usize, c: char) -> Result<AST, ParseError> {
    match c {
        '\\' | '(' | ')' | '*' | '+' | '?' => Ok(AST::Char(c)),
        _ => Err(ParseError::InvalidEscape(pos, c)),
    }
}

/// parse_plus_star_question利用するための列挙型。
enum PSQ {
    Plus,
    Star,
    Question,
}

// 正規表現を抽象構文木に変換
pub fn parse(expr: &str) -> Result<AST, ParseError> {
    // 内部状態を表現するための型
    // Char状態 : 文字列処理中
    // Escape状態 : エスケープシーケンス処理中
    enum ParseState {
        Char,
        Escape,
    }

    let mut seq = Vec::new(); // 現在のSeqのコンテキスト
    let mut seq_or = Vec::new(); // 現在のOrのコンテキスト
    let mut stack = Vec::new(); // コンテキストのスタック
    let mut state = ParseState::Char; // 現在の状態

    for (i, c) in expr.chars().enumerate() {
        match &state {
            ParseState::Char => {
                match c {
                    '+' => parse_plus_star_question(&mut seq, PSQ::Plus, i)?,
                    '*' => parse_plus_star_question(&mut seq, PSQ::Star, i)?,
                    '?' => parse_plus_star_question(&mut seq, PSQ::Question, i)?,
                    '(' => {
                        // 現在のコンテキストをスタックに追加し、
                        // 現在のコンテキストを空の状態にする
                        let prev = take(&mut seq);
                        let prev_or = take(&mut seq_or);
                        stack.push((prev, prev_or));
                    }
                    ')' => {
                        // 現在のコンテキストをスタックからポップ
                        let Some((mut prev, prev_or)) = stack.pop() else {
                            // "abc)"のように、開き括弧がないのに閉じ括弧がある場合はエラー
                            return Err(ParseError::InvalidRightParen(i));
                        };
                        // "()"のように式が空の場合はpushしない
                        if !seq.is_empty() {
                            seq_or.push(AST::Seq(seq));
                        }

                        // Orを生成
                        if let Some(ast) = fold_or(seq_or) {
                            prev.push(ast);
                        }

                        // 以前のコンテキストを、現在のコンテキストにする
                        seq = prev;
                        seq_or = prev_or;
                    }
                    '|' => {
                        if seq.is_empty() {
                            // "||", "(|abc)"などと、式が空の場合はエラー
                            return Err(ParseError::NoPrev(i));
                        }
                        let prev = take(&mut seq);
                        seq_or.push(AST::Seq(prev));
                    }
                    '\\' => state = ParseState::Escape,
                    _ => seq.push(AST::Char(c)),
                };
            }
            ParseState::Escape => {
                // エスケープシーケンス処理
                let ast = parse_escape(i, c)?;
                seq.push(ast);
                state = ParseState::Char;
            }
        }
    }

    // 閉じ括弧が足りない場合はエラー
    if !stack.is_empty() {
        return Err(ParseError::NoRightParen);
    }

    // "()"のように式が空の場合はpushしない
    if !seq.is_empty() {
        seq_or.push(AST::Seq(seq));
    }

    // Orを生成し、成功した場合はそれを返す
    if let Some(ast) = fold_or(seq_or) {
        Ok(ast)
    } else {
        Err(ParseError::Empty)
    }
}

/// +, *, ?をASTに変換。
///
/// 後置記法で、_, *, ? 前にパターン前前にパターンがない場合はエラー。
///
/// 例: *ab, abc|+ などはエラー。
fn parse_plus_star_question(
    seq: &mut Vec<AST>,
    ast_type: PSQ,
    pos: usize,
) -> Result<(), ParseError> {
    let Some(prev) = seq.pop() else {
        return Err(ParseError::NoPrev(pos));
    };
    let ast = match ast_type {
        PSQ::Plus => AST::Plus(Box::new(prev)),
        PSQ::Star => AST::Star(Box::new(prev)),
        PSQ::Question => AST::Question(Box::new(prev)),
    };
    seq.push(ast);
    Ok(())
}

/// Orで結合された複数の式をASTに変換。
///
/// たとえば、 abc|def|ghi は、AST::Or("abc", AST::Or("def", "ghi")) に変換される。
fn fold_or(mut seq_or: Vec<AST>) -> Option<AST> {
    if seq_or.len() <= 1 {
        return seq_or.pop();
    }
    let mut ast = seq_or.pop().unwrap();
    seq_or.reverse();
    for s in seq_or {
        ast = AST::Or(Box::new(s), Box::new(ast));
    }
    Some(ast)
}
