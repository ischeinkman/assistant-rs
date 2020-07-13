pub fn leven_dist(command: &str, text: &str) -> usize {
    let alen = command.len();
    let blen = text.len();
    let mut buf = vec![0; (alen + 1) * (blen + 1)];
    lev(&mut buf, command, text, alen, blen)
}

fn lev(buf: &mut Vec<usize>, sa: &str, sb: &str, aidx: usize, bidx: usize) -> usize {
    if aidx.min(bidx) == 0 {
        return aidx.max(bidx);
    }
    let buff_idx = bidx * (sa.len() + 1) + aidx;
    if buf[buff_idx] != 0 {
        return buf[buff_idx];
    }
    let lft = lev(buf, sa, sb, aidx - 1, bidx) + 1;
    let rgt = lev(buf, sa, sb, aidx, bidx - 1) + 1;
    let cur_eq = match (sa.chars().nth(aidx - 1), sb.chars().nth(bidx - 1)) {
        (Some(al), Some(bl)) if al == bl => 0,
        _ => 1,
    };
    let mid = cur_eq + lev(buf, sa, sb, aidx - 1, bidx - 1);
    let retvl = lft.min(mid).min(rgt);
    buf[buff_idx] = retvl;
    retvl
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
}
