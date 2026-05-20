//! ZSet 有序索引：跳表（对齐 Redis `t_zset.c`）
//!
//! 按 `(score, member)` 字典序排序；member→score 的 O(1) 查找由 [`super::ZSetInner::scores`] 承担。
use ordered_float::OrderedFloat;
use rand::Rng;
use std::cmp::Ordering;

const MAX_LEVEL: usize = 32;
const P: f64 = 0.25;
const NIL: usize = usize::MAX;

#[derive(Debug, Clone)]
struct Node {
    member: String,
    score: f64,
    forward: Vec<usize>,
}

/// 跳表：有序 `(score, member)` 索引
#[derive(Debug, Clone)]
pub struct SkipList {
    nodes: Vec<Node>,
    /// 当前有效层数（1..=MAX_LEVEL）
    level: usize,
    length: usize,
}

impl Default for SkipList {
    fn default() -> Self {
        Self::new()
    }
}

impl SkipList {
    pub fn new() -> Self {
        let mut sl = SkipList {
            nodes: Vec::new(),
            level: 1,
            length: 0,
        };
        // 头结点：占槽 0，member 为空
        sl.nodes.push(Node {
            member: String::new(),
            score: 0.0,
            forward: vec![NIL; MAX_LEVEL],
        });
        sl
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub fn insert(&mut self, score: f64, member: String) {
        let mut update = [NIL; MAX_LEVEL];
        let mut x = 0usize;

        for i in (0..self.level).rev() {
            loop {
                let fwd = self.nodes[x].forward[i];
                if fwd == NIL {
                    break;
                }
                let n = &self.nodes[fwd];
                if key_cmp(n.score, &n.member, score, &member) == Ordering::Less {
                    x = fwd;
                } else {
                    break;
                }
            }
            update[i] = x;
        }

        let next0 = self.nodes[x].forward[0];
        if next0 != NIL {
            let n = &self.nodes[next0];
            if n.score == score && n.member == member {
                return;
            }
        }

        let lvl = random_level();
        if lvl > self.level {
            update[self.level..lvl].fill(0);
            self.level = lvl;
        }

        let idx = self.nodes.len();
        self.nodes.push(Node {
            member,
            score,
            forward: vec![NIL; lvl],
        });

        for (i, &pred) in update.iter().enumerate().take(lvl) {
            self.nodes[idx].forward[i] = self.nodes[pred].forward[i];
            self.nodes[pred].forward[i] = idx;
        }
        self.length += 1;
    }

    pub fn remove(&mut self, score: f64, member: &str) -> bool {
        let mut update = [NIL; MAX_LEVEL];
        let mut x = 0usize;

        for i in (0..self.level).rev() {
            loop {
                let fwd = self.nodes[x].forward[i];
                if fwd == NIL {
                    break;
                }
                let n = &self.nodes[fwd];
                if key_cmp(n.score, &n.member, score, member) == Ordering::Less {
                    x = fwd;
                } else {
                    break;
                }
            }
            update[i] = x;
        }

        let target = self.nodes[x].forward[0];
        if target == NIL {
            return false;
        }
        let n = &self.nodes[target];
        if n.score != score || n.member != member {
            return false;
        }

        for (i, &pred) in update.iter().enumerate().take(self.level) {
            if self.nodes[pred].forward[i] != target {
                break;
            }
            self.nodes[pred].forward[i] = self.nodes[target].forward[i];
        }

        while self.level > 1 && self.nodes[0].forward[self.level - 1] == NIL {
            self.level -= 1;
        }
        self.length -= 1;
        true
    }

    /// 0-based rank：严格小于 `(score, member)` 的元素个数
    pub fn rank_of(&self, score: f64, member: &str) -> Option<usize> {
        let mut rank = 0usize;
        let mut idx = self.nodes[0].forward[0];
        while idx != NIL {
            let n = &self.nodes[idx];
            match key_cmp(n.score, &n.member, score, member) {
                Ordering::Less => {
                    rank += 1;
                    idx = n.forward[0];
                }
                Ordering::Equal => return Some(rank),
                Ordering::Greater => return None,
            }
        }
        None
    }

    pub fn iter(&self) -> impl Iterator<Item = (String, f64)> + '_ {
        SkipListIter {
            sl: self,
            idx: self.nodes[0].forward[0],
        }
    }

    pub fn iter_rev(&self) -> impl Iterator<Item = (String, f64)> + '_ {
        let mut chain = Vec::with_capacity(self.length);
        let mut idx = self.nodes[0].forward[0];
        while idx != NIL {
            chain.push(idx);
            idx = self.nodes[idx].forward[0];
        }
        chain.into_iter().rev().map(|i| {
            let n = &self.nodes[i];
            (n.member.clone(), n.score)
        })
    }
}

struct SkipListIter<'a> {
    sl: &'a SkipList,
    idx: usize,
}

impl Iterator for SkipListIter<'_> {
    type Item = (String, f64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx == NIL {
            return None;
        }
        let n = &self.sl.nodes[self.idx];
        let item = (n.member.clone(), n.score);
        self.idx = n.forward[0];
        Some(item)
    }
}

fn key_cmp(s1: f64, m1: &str, s2: f64, m2: &str) -> Ordering {
    OrderedFloat(s1)
        .cmp(&OrderedFloat(s2))
        .then_with(|| m1.cmp(m2))
}

fn random_level() -> usize {
    let mut lvl = 1usize;
    let mut rng = rand::rng();
    while lvl < MAX_LEVEL && rng.random::<f64>() < P {
        lvl += 1;
    }
    lvl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_rank_range() {
        let mut sl = SkipList::new();
        sl.insert(1.0, "a".into());
        sl.insert(2.0, "b".into());
        sl.insert(3.0, "c".into());
        assert_eq!(sl.len(), 3);
        assert_eq!(sl.rank_of(2.0, "b"), Some(1));
        let members: Vec<_> = sl.iter().map(|(m, _)| m).collect();
        assert_eq!(members, vec!["a", "b", "c"]);
    }

    #[test]
    fn remove_and_reinsert() {
        let mut sl = SkipList::new();
        sl.insert(1.0, "a".into());
        sl.insert(2.0, "b".into());
        assert!(sl.remove(1.0, "a"));
        assert_eq!(sl.len(), 1);
        sl.insert(1.0, "a".into());
        assert_eq!(sl.rank_of(1.0, "a"), Some(0));
    }

    #[test]
    fn same_score_lex_member() {
        let mut sl = SkipList::new();
        sl.insert(1.0, "b".into());
        sl.insert(1.0, "a".into());
        let members: Vec<_> = sl.iter().map(|(m, _)| m).collect();
        assert_eq!(members, vec!["a", "b"]);
        assert_eq!(sl.rank_of(1.0, "b"), Some(1));
    }
}
