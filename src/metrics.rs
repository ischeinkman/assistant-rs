pub fn leven_dist(command: &str, text: &str) -> usize {
    wagner_lev(command, text, false)
}

// https://en.wikipedia.org/wiki/Wagner%E2%80%93Fischer_algorithm
// http://ginstrom.com/scribbles/2007/12/01/fuzzy-substring-matching-with-levenshtein-distance-in-python/
fn wagner_lev(command: &str, text: &str, substring_match: bool) -> usize {
    let mut buf = TwoDimBuffer::new_with_size(text.len() + 1, command.len() + 1);
    for xidx in 0..text.len() + 1 {
        *buf.get_mut(xidx, 0).unwrap() = if substring_match { 0 } else { xidx };
    }
    for yidx in 0..command.len() + 1 {
        *buf.get_mut(0, yidx).unwrap() = yidx;
    }
    for yidx in 1..command.len() + 1 {
        for xidx in 1..text.len() + 1 {
            let dsub = (command.as_bytes()[yidx - 1] != text.as_bytes()[xidx - 1]) as usize;
            let subs = buf.get(xidx - 1, yidx - 1).copied().unwrap() + dsub;
            let del = buf.get(xidx - 1, yidx).copied().unwrap() + 1;
            let ins = buf.get(xidx, yidx - 1).copied().unwrap() + 1;
            *buf.get_mut(xidx, yidx).unwrap() = ins.min(del).min(subs);
        }
    }
    if substring_match {
        (0..text.len() + 1)
            .map(|xidx| buf.get(xidx, command.len()).copied())
            .min()
            .flatten()
            .unwrap()
    } else {
        buf.buffer.last().copied().unwrap()
    }
}

struct TwoDimBuffer<T> {
    buffer: Vec<T>,
    width: usize,
}

impl<T> TwoDimBuffer<T> {
    pub fn new(width: usize, data: Vec<T>) -> Self {
        Self {
            buffer: data,
            width,
        }
    }
    pub fn get(&self, xidx: usize, yidx: usize) -> Option<&T> {
        self.buffer.get(xidx + yidx * self.width)
    }
    pub fn get_mut(&mut self, xidx: usize, yidx: usize) -> Option<&mut T> {
        self.buffer.get_mut(xidx + yidx * self.width)
    }
}

impl<T: Default> TwoDimBuffer<T> {
    pub fn new_with_size(width: usize, height: usize) -> Self {
        let mut buffer = Vec::with_capacity(width * height);
        buffer.resize_with(width * height, Default::default);
        Self::new(width, buffer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_leven() {
        let wa = "sunday";
        let wb = "saturday";
        assert_eq!(3, leven_dist(&wa, &wb));
    }
    #[test]
    fn test_substring_leven() {
        let wa = "day";
        let wb = "saturd by";
        assert_eq!(2, wagner_lev(&wa, &wb, true));
    }
}
