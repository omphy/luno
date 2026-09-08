use logos::Logos;

// EVERY token in the entire language (warning, regex ahead!)
#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\r\f]+")]
#[logos(skip(r"--[^\n]*", allow_greedy = true))]
pub enum Token {
    #[token("function")]
    Function,
    #[token("return")]
    Return,
    #[token(",")]
    Comma,
    #[token("local")]
    Local,
    #[token("global")]
    Global,
    #[token("=")]
    Assign,
    #[token("*")]
    Multiply,
    #[token("/")]
    Divide,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("^")]
    Exponent,
    #[token("==")]
    Equal,
    #[token("~=")]
    NotEqual,
    #[token("<")]
    Less,
    #[token("<=")]
    LessEq,
    #[token(">")]
    More,
    #[token(">=")]
    MoreEq,
    #[token("(")]
    ParenthesisOpen,
    #[token(")")]
    ParenthesisClose,
    #[token("[")]
    SquareBracketOpen,
    #[token("]")]
    SquareBracketClose,
    #[token("{")]
    CurlyBracketOpen,
    #[token("}")]
    CurlyBracketClose,
    #[token("if")]
    If,
    #[token("then")]
    Then,
    #[token("else")]
    Else,
    #[token("elseif")]
    ElseIf,
    #[token("while")]
    While,
    #[token("do")]
    Do,
    #[token("end")]
    End,
    #[token("nil")]
    Nil,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),
    #[regex(r"[0-9]+(?:\.[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Number(f64),
    #[token("..")]
    Concat,
    #[token(".")]
    Dot,
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let slice = lex.slice();
        let inner = &slice[1..slice.len() - 1];
        let mut unescaped = String::with_capacity(inner.len());
        let mut chars = inner.chars();

        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some('n') => unescaped.push('\n'),
                    Some('t') => unescaped.push('\t'),
                    Some('r') => unescaped.push('\r'),
                    Some('\\') => unescaped.push('\\'),
                    Some('"') => unescaped.push('"'),
                    Some(other) => unescaped.push(other),
                    None => {}
                }
            } else {
                unescaped.push(c);
            }
        }
        unescaped
    })]
    String(String),
}
