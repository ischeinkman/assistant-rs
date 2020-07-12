use crate::error::PhonemeConvertionError;
use crate::utils::IterUtils;
use arpabet::{phoneme::Phoneme, Arpabet};

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

#[derive(PartialEq, Debug, Clone)]
pub enum PhonePart {
    Phoneme(Phoneme),
    Space,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Utterance {
    phones: Vec<PhonePart>,
}

impl Utterance {
    pub fn parse(raw_msg: &str) -> Result<Self, PhonemeConvertionError> {
        let phones = conv(raw_msg)
            .map(|p| match p {
                PhonePart::Unknown(inner) => Err(PhonemeConvertionError { raw: inner }),
                other => Ok(other),
            })
            .collect::<Result<_, PhonemeConvertionError>>()?;
        Ok(Self { phones })
    }
    pub fn parse_with_unknowns(raw_msg: &str) -> Self {
        let phones = conv(raw_msg).collect();
        Self { phones }
    }
}
