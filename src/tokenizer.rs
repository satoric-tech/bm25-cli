use std::collections::VecDeque;
use tantivy::tokenizer::{
    Language, LowerCaser, SimpleTokenizer, Stemmer, TextAnalyzer, Token, TokenFilter, TokenStream,
    Tokenizer,
};

pub const TOKENIZER_NAME: &str = "code";

pub fn build() -> TextAnalyzer {
    TextAnalyzer::builder(SimpleTokenizer::default())
        .filter(CamelSplitter)
        .filter(LowerCaser)
        .filter(Stemmer::new(Language::English))
        .build()
}

#[derive(Clone)]
struct CamelSplitter;

impl TokenFilter for CamelSplitter {
    type Tokenizer<T: Tokenizer> = CamelSplitterFilter<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> CamelSplitterFilter<T> {
        CamelSplitterFilter { tokenizer }
    }
}

#[derive(Clone)]
struct CamelSplitterFilter<T> {
    tokenizer: T,
}

impl<T: Tokenizer> Tokenizer for CamelSplitterFilter<T> {
    type TokenStream<'a> = CamelSplitterStream<'a, T::TokenStream<'a>>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        CamelSplitterStream {
            tail: self.tokenizer.token_stream(text),
            buffer: VecDeque::new(),
            current: Token::default(),
            _marker: std::marker::PhantomData,
        }
    }
}

struct CamelSplitterStream<'a, T> {
    tail: T,
    buffer: VecDeque<Token>,
    current: Token,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a, T: TokenStream> TokenStream for CamelSplitterStream<'a, T> {
    fn advance(&mut self) -> bool {
        if let Some(t) = self.buffer.pop_front() {
            self.current = t;
            return true;
        }
        while self.tail.advance() {
            let splits = split_camel(self.tail.token());
            if splits.is_empty() {
                continue;
            }
            let mut iter = splits.into_iter();
            self.current = iter.next().unwrap();
            self.buffer.extend(iter);
            return true;
        }
        false
    }

    fn token(&self) -> &Token {
        &self.current
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.current
    }
}

fn split_camel(token: &Token) -> Vec<Token> {
    let word = &token.text;
    let chars: Vec<(usize, char)> = word.char_indices().collect();
    if chars.is_empty() {
        return vec![];
    }

    let mut segments: Vec<(usize, usize)> = vec![];
    let mut seg_start = 0usize;

    let mut i = 1usize;
    while i < chars.len() {
        let prev = chars[i - 1].1;
        let (byte_i, curr) = chars[i];
        let next_lower = chars
            .get(i + 1)
            .map(|(_, c)| c.is_lowercase())
            .unwrap_or(false);

        let split =
            curr.is_uppercase() && (prev.is_lowercase() || (prev.is_uppercase() && next_lower));

        if split {
            segments.push((chars[seg_start].0, byte_i));
            seg_start = i;
        }

        i += 1;
    }
    segments.push((chars[seg_start].0, word.len()));

    if segments.len() == 1 {
        return vec![token.clone()];
    }

    segments
        .into_iter()
        .enumerate()
        .map(|(i, (start, end))| Token {
            offset_from: token.offset_from + start,
            offset_to: token.offset_from + end,
            position: token.position + i,
            position_length: 1,
            text: word[start..end].to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(text: &str) -> Vec<String> {
        let mut tokenizer = build();
        let mut stream = tokenizer.token_stream(text);
        let mut tokens = vec![];
        while stream.advance() {
            tokens.push(stream.token().text.clone());
        }
        tokens
    }

    #[test]
    fn test_camel_case() {
        let tokens = tok("PaymentHandler");
        assert!(tokens.contains(&"payment".to_string()));
        assert!(tokens.contains(&"handler".to_string()));
    }

    #[test]
    fn test_uppercase_run() {
        let tokens = tok("HTMLParser");
        assert!(tokens.contains(&"html".to_string()));
        assert!(tokens.contains(&"parser".to_string()));
    }

    #[test]
    fn test_snake_case() {
        let tokens = tok("handle_payment");
        assert!(tokens.contains(&"payment".to_string()));
    }

    #[test]
    fn test_dot_separated() {
        let tokens = tok("payment.succeeded");
        assert!(tokens.contains(&"payment".to_string()));
        assert!(tokens.contains(&"succeed".to_string()));
    }

    #[test]
    fn test_screaming_snake() {
        let tokens = tok("PAYMENT_STATUS");
        assert!(tokens.contains(&"payment".to_string()));
        assert!(tokens.contains(&"status".to_string()));
    }

    #[test]
    fn test_stemming() {
        let a = tok("authentication");
        let b = tok("authenticating");
        let c = tok("authenticated");
        assert_eq!(a, b);
        assert_eq!(b, c);
    }

    #[test]
    fn test_mixed() {
        let tokens = tok("fn handle_payment(amount: u64)");
        assert!(tokens.contains(&"fn".to_string()));
        assert!(tokens.contains(&"payment".to_string()));
        assert!(tokens.contains(&"amount".to_string()));
    }
}
