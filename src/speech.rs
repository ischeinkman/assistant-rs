use crate::error::PhonemeConvertionError;
use crate::utils::IterUtils;
use arpabet::{phoneme::Phoneme, Arpabet};


/// Attempt to convert a string into its pronounciation. 
///
/// Currently this can only be done on strings composed of words in the CMUdict database.
fn conv<'a>(raw_msg: &'a str) -> impl Iterator<Item = PhonePart> + 'a {
    let ap = Arpabet::load_cmudict();
    raw_msg
        .split_whitespace()
        .flat_map(move |w| {
            let nxt = match ap.get_polyphone(w) {
                Some(pw) => pw.into_iter().map(|p| PhonePart::Phoneme(p)).left(),
                None => std::iter::once(PhonePart::Unknown(w.to_owned())).right(),
            };
            std::iter::once(PhonePart::Space).chain(nxt)
        })
        .skip(1)
}


/// A pronounciation unit in an audio transcript.
#[derive(PartialEq, Debug, Clone)]
pub enum PhonePart {

    /// An ARPABET Phoneme, including both consonants and vowels.
    Phoneme(Phoneme),

    /// A divider between words in a transcript.
    Space,

    /// A words whose pronounciation cannot (yet) be found.
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Utterance {
    phones: Vec<PhonePart>,
}

impl Utterance {

    /// Attemts to parse a string into its pronounciation, erroring if `raw_msg` contains a word
    /// whose pronounciation cannot be found.
    #[allow(unused)]
    pub fn parse(raw_msg: &str) -> Result<Self, PhonemeConvertionError> {
        let phones = conv(raw_msg)
            .map(|p| match p {
                PhonePart::Unknown(inner) => Err(PhonemeConvertionError { raw: inner }),
                other => Ok(other),
            })
            .collect::<Result<_, PhonemeConvertionError>>()?;
        Ok(Self { phones })
    }
    
    /// Attemts to parse a string into its pronounciation, returning a `PhonePart::Unknown` for words
    /// whose pronounciation cannot yet be found.
    #[allow(unused)]
    pub fn parse_with_unknowns(raw_msg: &str) -> Self {
        let phones = conv(raw_msg).collect();
        Self { phones }
    }
}
