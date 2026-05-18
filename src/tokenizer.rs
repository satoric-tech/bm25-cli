use tantivy::tokenizer::{
    AsciiFoldingFilter, Language, LowerCaser, RawTokenizer, RemoveLongFilter, SimpleTokenizer,
    Stemmer, TextAnalyzer, TextAnalyzerBuilder, Tokenizer, WhitespaceTokenizer,
};

pub const TOKENIZER_NAME: &str = "bm25";
const REMOVE_LONG_LIMIT: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub enum Filter {
    Stem,
    AsciiFold,
    RemoveLong,
}

pub fn parse_filter(s: &str) -> Option<Filter> {
    match s {
        "stem" => Some(Filter::Stem),
        "ascii-fold" => Some(Filter::AsciiFold),
        "remove-long" => Some(Filter::RemoveLong),
        _ => None,
    }
}

pub fn parse_lang(s: &str) -> Option<Language> {
    match s.to_lowercase().as_str() {
        "arabic" => Some(Language::Arabic),
        "danish" => Some(Language::Danish),
        "dutch" => Some(Language::Dutch),
        "english" => Some(Language::English),
        "finnish" => Some(Language::Finnish),
        "french" => Some(Language::French),
        "german" => Some(Language::German),
        "greek" => Some(Language::Greek),
        "hungarian" => Some(Language::Hungarian),
        "italian" => Some(Language::Italian),
        "norwegian" => Some(Language::Norwegian),
        "portuguese" => Some(Language::Portuguese),
        "romanian" => Some(Language::Romanian),
        "russian" => Some(Language::Russian),
        "spanish" => Some(Language::Spanish),
        "swedish" => Some(Language::Swedish),
        "tamil" => Some(Language::Tamil),
        "turkish" => Some(Language::Turkish),
        _ => None,
    }
}

pub fn build(tokenizer: &str, filters: &[Filter], lang: Option<Language>) -> TextAnalyzer {
    let af = filters.contains(&Filter::AsciiFold);
    let rl = filters.contains(&Filter::RemoveLong);
    let stem = filters.contains(&Filter::Stem);
    let lang = if stem { lang } else { None };

    match tokenizer {
        "whitespace" => chain(TextAnalyzer::builder(WhitespaceTokenizer::default()), af, rl, lang),
        "raw" => chain(TextAnalyzer::builder(RawTokenizer::default()), af, rl, lang),
        _ => chain(TextAnalyzer::builder(SimpleTokenizer::default()), af, rl, lang),
    }
}

fn chain<T: Tokenizer>(
    base: TextAnalyzerBuilder<T>,
    af: bool,
    rl: bool,
    lang: Option<Language>,
) -> TextAnalyzer {
    match (af, rl, lang) {
        (false, false, None) => base.filter(LowerCaser).build(),
        (true,  false, None) => base.filter(LowerCaser).filter(AsciiFoldingFilter).build(),
        (false, true,  None) => base.filter(LowerCaser).filter(RemoveLongFilter::limit(REMOVE_LONG_LIMIT)).build(),
        (true,  true,  None) => base.filter(LowerCaser).filter(AsciiFoldingFilter).filter(RemoveLongFilter::limit(REMOVE_LONG_LIMIT)).build(),
        (false, false, Some(l)) => base.filter(LowerCaser).filter(Stemmer::new(l)).build(),
        (true,  false, Some(l)) => base.filter(LowerCaser).filter(AsciiFoldingFilter).filter(Stemmer::new(l)).build(),
        (false, true,  Some(l)) => base.filter(LowerCaser).filter(RemoveLongFilter::limit(REMOVE_LONG_LIMIT)).filter(Stemmer::new(l)).build(),
        (true,  true,  Some(l)) => base.filter(LowerCaser).filter(AsciiFoldingFilter).filter(RemoveLongFilter::limit(REMOVE_LONG_LIMIT)).filter(Stemmer::new(l)).build(),
    }
}
