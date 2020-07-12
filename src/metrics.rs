pub fn leven_dist(command: &str, text: &str) -> usize {
    let alen = command.len();
    let blen = text.len();
    lev(command, text, alen, blen)
}

fn lev(sa: &str, sb: &str, aidx: usize, bidx: usize) -> usize {
    if aidx.min(bidx) == 0 {
        return aidx.max(bidx);
    }
    let lft = lev(sa, sb, aidx - 1, bidx) + 1;
    let rgt = lev(sa, sb, aidx, bidx - 1) + 1;
    let cur_eq = match (sa.chars().nth(aidx - 1), sb.chars().nth(bidx - 1)) {
        (Some(al), Some(bl)) if al == bl => 0,
        _ => 1,
    };
    let mid = cur_eq + lev(sa, sb, aidx - 1, bidx - 1);
    lft.min(mid).min(rgt)
}
