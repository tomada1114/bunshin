//! A conservative Unicode-scalar estimate, independent of the model tokenizer.

/// One token per non-ASCII scalar, plus rounded-up ASCII groups. A zero group
/// size uses one character per token, preserving a conservative estimate.
#[must_use]
pub fn estimate(text: &str, ascii_chars_per_token: usize) -> usize {
    let (mut ascii, mut other) = (0_usize, 0_usize);
    for character in text.chars() {
        if character.is_ascii() {
            ascii += 1;
        } else {
            other += 1;
        }
    }
    other + ascii.div_ceil(ascii_chars_per_token.max(1))
}
