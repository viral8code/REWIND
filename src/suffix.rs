//! Byte suffix arrays using stable radix doubling, plus linear Kasai LCP.
//! LCP[0] is zero; LCP[i] compares SA[i - 1] and SA[i].
pub const MAX_BYTES: usize = 1024 * 1024;
pub fn build(input: &[u8]) -> Option<(Vec<i64>, Vec<i64>)> {
    let n = input.len();
    if n > MAX_BYTES {
        return None;
    }
    let mut order: Vec<usize> = (0..n).collect();
    let mut rank: Vec<usize> = input.iter().map(|b| *b as usize + 1).collect();
    let mut next = vec![0usize; n];
    let mut temp = vec![0usize; n];
    let mut classes = 256usize;
    let mut width = 1usize;
    while width < n {
        for offset in [width, 0] {
            let key = |i: usize| rank.get(i + offset).copied().unwrap_or(0);
            let mut counts = vec![0usize; classes + 1];
            for &i in &order {
                counts[key(i)] += 1;
            }
            let mut total = 0;
            for count in &mut counts {
                let old = *count;
                *count = total;
                total += old;
            }
            for &i in &order {
                let k = key(i);
                temp[counts[k]] = i;
                counts[k] += 1;
            }
            std::mem::swap(&mut order, &mut temp);
        }
        classes = 0;
        let mut previous = None;
        for &i in &order {
            let pair = (rank[i], rank.get(i + width).copied().unwrap_or(0));
            if previous != Some(pair) {
                classes += 1;
                previous = Some(pair);
            }
            next[i] = classes;
        }
        std::mem::swap(&mut rank, &mut next);
        if classes == n {
            break;
        }
        width *= 2;
    }
    let mut positions = vec![0usize; n];
    for (i, &p) in order.iter().enumerate() {
        positions[p] = i;
    }
    let mut lcp = vec![0i64; n];
    let mut matched = 0;
    for i in 0..n {
        let r = positions[i];
        if r == 0 {
            matched = 0;
            continue;
        }
        let j = order[r - 1];
        while i + matched < n && j + matched < n && input[i + matched] == input[j + matched] {
            matched += 1;
        }
        lcp[r] = matched as i64;
        matched = matched.saturating_sub(1);
    }
    Some((order.into_iter().map(|i| i as i64).collect(), lcp))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_sorted_suffixes_and_lcp() {
        let mut seed = 31u64;
        for n in 0..100 {
            for alphabet in [1, 2, 7, 256] {
                let input: Vec<u8> = (0..n)
                    .map(|_| {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                        ((seed >> 32) % alphabet) as u8
                    })
                    .collect();
                let mut expected: Vec<i64> = (0..n as i64).collect();
                expected.sort_by(|a, b| input[*a as usize..].cmp(&input[*b as usize..]));
                let (sa, lcp) = build(&input).unwrap();
                assert_eq!(sa, expected);
                for i in 1..n {
                    let a = sa[i - 1] as usize;
                    let b = sa[i] as usize;
                    let expected = input[a..]
                        .iter()
                        .zip(&input[b..])
                        .take_while(|(a, b)| a == b)
                        .count();
                    assert_eq!(lcp[i], expected as i64);
                }
            }
        }
    }
    #[test]
    fn repeated_bytes_and_bound() {
        let (sa, lcp) = build(&vec![0; 65536]).unwrap();
        assert_eq!(sa[0], 65535);
        assert_eq!(sa[65535], 0);
        assert_eq!(lcp[65535], 65535);
        assert!(build(&vec![0; MAX_BYTES + 1]).is_none());
    }
}
