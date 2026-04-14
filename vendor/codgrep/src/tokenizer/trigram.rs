use crate::tokenizer::Tokenizer;

pub struct TrigramTokenizer;

impl Tokenizer for TrigramTokenizer {
    fn collect_document_token_hashes(&self, text: &str, out: &mut Vec<u64>) {
        let bytes = text.as_bytes();
        if bytes.len() < 3 {
            return;
        }

        out.reserve(bytes.len().saturating_sub(2));
        for window in bytes.windows(3) {
            out.push(u64::from(crc32fast::hash(window)));
        }
    }
}
