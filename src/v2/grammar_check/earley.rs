//! Earley 認識器。認識だけを行い、構文の森は作らない。
//!
//! - 終端は述語（1 トークンに複数の終端が当てはまってよい）。
//! - Earley 集合は重複を取り除く。
//! - 空になれる記号は Aycock–Horspool の方式（予測時に点を進める）で扱う。
//!   注釈（`ns`/`nf`）に依存して空になる記号は、事前計算に入れず、同じ集合の中で
//!   空に完了した記号を覚えて（`empty_done`）、あとから来る項目にも適用する。
//! - 注釈は、点が注釈に来た時点で次の 1 トークンを見て判定する。

use super::ebnf::{Bnf, Sym, Token};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Item {
    rule: u32,
    dot: u32,
    origin: u32,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    /// 全集合の項目数の合計。
    pub items: usize,
    /// 最大の集合の項目数。
    pub max_set: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RecognizeError {
    /// 受理できなかったトークンの位置（入力の終わりで失敗したときはトークン数）。
    pub pos: usize,
}

pub struct Earley<'b> {
    bnf: &'b Bnf,
    nullable: Vec<bool>,
}

fn push(items: &mut Vec<Item>, seen: &mut HashSet<Item>, it: Item) {
    if seen.insert(it) {
        items.push(it);
    }
}

impl<'b> Earley<'b> {
    pub fn new(bnf: &'b Bnf) -> Self {
        // 注釈や終端を含まない規則だけから、空になれる非終端を求める。
        let mut nullable = vec![false; bnf.nt_names.len()];
        loop {
            let mut changed = false;
            for r in &bnf.rules {
                if !nullable[r.lhs as usize]
                    && r.rhs
                        .iter()
                        .all(|s| matches!(s, Sym::Nt(n) if nullable[*n as usize]))
                {
                    nullable[r.lhs as usize] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Earley { bnf, nullable }
    }

    pub fn recognize<T: Token>(&self, toks: &[T]) -> Result<Stats, RecognizeError> {
        let bnf = self.bnf;
        let n = toks.len();
        let start = bnf.start;
        // 各集合で「次に非終端を待つ項目」を、その非終端の番号で整列して持つ。
        let mut sets: Vec<Vec<(u32, Item)>> = Vec::with_capacity(n + 1);
        let mut seed: Vec<Item> = bnf.by_lhs[start as usize]
            .iter()
            .map(|&r| Item {
                rule: r,
                dot: 0,
                origin: 0,
            })
            .collect();
        let mut stats = Stats::default();
        let mut seen: HashSet<Item> = HashSet::new();
        let mut predicted = vec![usize::MAX; bnf.nt_names.len()];
        for i in 0..=n {
            seen.clear();
            let mut items: Vec<Item> = Vec::with_capacity(seed.len());
            for it in seed.drain(..) {
                push(&mut items, &mut seen, it);
            }
            let mut next: Vec<Item> = Vec::new();
            let mut waiting: Vec<(u32, Item)> = Vec::new();
            let mut empty_done: HashSet<u32> = HashSet::new();
            let mut accepted = false;
            let mut idx = 0;
            while idx < items.len() {
                let it = items[idx];
                idx += 1;
                let rule = &bnf.rules[it.rule as usize];
                let dot = it.dot as usize;
                if dot == rule.rhs.len() {
                    // 完了
                    let lhs = rule.lhs;
                    if lhs == start && it.origin == 0 {
                        accepted = true;
                    }
                    if it.origin as usize == i {
                        // 空の範囲で完了した。この集合の既存の項目を進め、あとから来る項目は
                        // empty_done で進める。
                        empty_done.insert(lhs);
                        let mut j = 0;
                        while j < items.len() {
                            let w = items[j];
                            j += 1;
                            let wr = &bnf.rules[w.rule as usize];
                            if let Some(Sym::Nt(b)) = wr.rhs.get(w.dot as usize) {
                                if *b == lhs {
                                    push(&mut items, &mut seen, Item { dot: w.dot + 1, ..w });
                                }
                            }
                        }
                    } else {
                        let set = &sets[it.origin as usize];
                        let lo = set.partition_point(|&(nt, _)| nt < lhs);
                        for &(nt, w) in &set[lo..] {
                            if nt != lhs {
                                break;
                            }
                            push(&mut items, &mut seen, Item { dot: w.dot + 1, ..w });
                        }
                    }
                    continue;
                }
                match rule.rhs[dot] {
                    Sym::Nt(b) => {
                        waiting.push((b, it));
                        if predicted[b as usize] != i {
                            predicted[b as usize] = i;
                            for &r in &bnf.by_lhs[b as usize] {
                                push(
                                    &mut items,
                                    &mut seen,
                                    Item {
                                        rule: r,
                                        dot: 0,
                                        origin: i as u32,
                                    },
                                );
                            }
                        }
                        if self.nullable[b as usize] || empty_done.contains(&b) {
                            push(&mut items, &mut seen, Item { dot: it.dot + 1, ..it });
                        }
                    }
                    Sym::T(t) => {
                        if i < n && bnf.terms[t as usize].matches(&toks[i]) {
                            next.push(Item { dot: it.dot + 1, ..it });
                        }
                    }
                    Sym::Ns(t) | Sym::Nf(t) => {
                        if !(i < n && bnf.terms[t as usize].matches(&toks[i])) {
                            push(&mut items, &mut seen, Item { dot: it.dot + 1, ..it });
                        }
                    }
                }
            }
            stats.items += items.len();
            stats.max_set = stats.max_set.max(items.len());
            if i == n {
                return if accepted {
                    Ok(stats)
                } else {
                    Err(RecognizeError { pos: n })
                };
            }
            if next.is_empty() {
                return Err(RecognizeError { pos: i });
            }
            waiting.sort_by_key(|&(nt, _)| nt);
            sets.push(waiting);
            seed = next;
        }
        unreachable!()
    }
}
