/// Utility struct to wrap multiple compatible iterators into one.
pub enum EitherIter<Itm, A: Iterator<Item = Itm>, B: Iterator<Item = Itm>> {
    Left(A),
    Right(B),
}

impl<Itm, A: Iterator<Item = Itm>, B: Iterator<Item = Itm>> EitherIter<Itm, A, B> {}

impl<Itm, A: Iterator<Item = Itm>, B: Iterator<Item = Itm>> Iterator for EitherIter<Itm, A, B> {
    type Item = Itm;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Left(l) => l.next(),
            Self::Right(r) => r.next(),
        }
    }
}

pub trait IterUtils<Item>: Iterator<Item = Item> + Sized {
    /// Wrap this iterator into an `EitherIter::Left`.
    fn left<R: Iterator<Item = Item>>(self) -> EitherIter<Item, Self, R> {
        EitherIter::Left(self)
    }
    /// Wrap this iterator into an `EitherIter::Right`.
    fn right<L: Iterator<Item = Item>>(self) -> EitherIter<Item, L, Self> {
        EitherIter::Right(self)
    }
}

impl<S, I> IterUtils<I> for S where S: Iterator<Item = I> {}

/// Simple `serde::de::Visitor` impl that just returns a string that it is fed.
/// Useful for deserialization requiring some extra preprocessing on init, like  
/// for example generating the `PhonePart` list from the `raw` text in a `CommandMessage`.
pub struct StringVisitor {}

impl StringVisitor {
    pub const fn new() -> Self {
        Self {}
    }
}
impl<'a> serde::de::Visitor<'a> for StringVisitor {
    type Value = String;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a string.")
    }
    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v)
    }
    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v.to_owned())
    }
}

fn split_at_first<'a>(inp: &'a str, marker: char) -> (&'a str, &'a str) {
    let mut sep_iter = inp.match_indices(marker);
    if let Some((idx, _)) = sep_iter.next() {
        let a = &inp[..idx];
        let b = &inp[idx + 1..];
        (a, b)
    } else {
        (inp, &"")
    }
}

pub trait StrUtils {
    fn split_at_first<'a>(&'a self, marker: char) -> (&'a str, &'a str);
}

impl StrUtils for str {
    fn split_at_first<'a>(&'a self, marker: char) -> (&'a str, &'a str) {
        split_at_first(&self, marker)
    }
}


pub trait StringUtils {
    fn split_owned(self, marker : char) -> SplitOwned;
}

impl StringUtils for String {
    fn split_owned(self, marker: char) -> SplitOwned {
        SplitOwned {marker, buffer : self}
    }
}

pub struct SplitOwned {
    marker : char, 
    buffer : String, 
}

impl Iterator for SplitOwned {
    type Item = String;
    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer.is_empty() {
            return None;
        }
        let spl_idx = self.buffer.find(self.marker);
        match spl_idx {
            Some(idx) => {
                let mut next_buff = self.buffer.split_off(idx + 1);
                std::mem::swap(&mut self.buffer, &mut next_buff);
                next_buff.pop();
                Some(next_buff)
            }
            None => {
                Some( std::mem::take(&mut self.buffer) )
            }
        }
    }
}