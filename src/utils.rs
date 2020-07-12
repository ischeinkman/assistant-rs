
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
    fn left<R: Iterator<Item = Item>>(self) -> EitherIter<Item, Self, R> {
        EitherIter::Left(self)
    }
    fn right<L: Iterator<Item = Item>>(self) -> EitherIter<Item, L, Self> {
        EitherIter::Right(self)
    }
}

impl<S, I> IterUtils<I> for S where S: Iterator<Item = I> {}