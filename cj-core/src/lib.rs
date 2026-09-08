//! Shared rules for 倉頡日課: parse the packed Rime table and elect ten
//! characters with a Praos-*shaped* lottery.
//!
//! Same φ(σ) = 1−(1−f)^σ test as Ouroboros Praos, but **not** Cardano Praos:
//! Y is Blake2b of public bytes (no keyed VRF, no proof), f is 0.8 not ~0.05,
//! winners sit out, and stake is letter weights rather than ADA. Digests go
//! through [`pallas_crypto::hash::Hasher`] (Blake2b-256).

use pallas_crypto::hash::Hasher;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

pub const SESSION_N: usize = 10;
pub const ACTIVE_SLOT_F: f64 = 0.8;

const JUNK_HAN: &[&str] = &[
    "丄", "丅", "丆", "丌", "丨", "丩", "丮", "丯", "丶", "丷", "丿", "乀", "乁", "乂",
    "乄", "乆", "乇", "乊", "乚", "乛", "乜", "乢", "乪", "亅", "亇", "亠", "亻", "亼",
    "亽", "亾", "冂", "冖", "冫", "凵", "卩", "厶", "囗", "夂", "夊", "宀", "尢", "屮",
    "巛", "廴", "廾", "弋", "彡", "彳", "忄", "扌", "氵", "犭", "纟", "艹", "辶", "釒",
    "钅", "阝", "饣", "飠",
];

#[derive(Clone, Debug, Deserialize)]
pub struct Table {
    pub source: Option<String>,
    pub radicals: HashMap<String, String>,
    pub chars: Vec<Glyph>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Glyph {
    pub h: String,
    pub c: String,
    pub n: u8,
    pub f: u8,
}

pub fn parse_table(json: &str) -> Result<Table, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn han_playable(g: &Glyph) -> bool {
    if g.h.is_empty() || JUNK_HAN.iter().any(|j| *j == g.h) {
        return false;
    }
    let mut chars = g.h.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => {
            let u = c as u32;
            (0x4E00..=0x9FFF).contains(&u)
        }
        _ => false,
    }
}

fn keep_extra_five(h: &str) -> bool {
    // ~19% of non-common 5-key glyphs — keeps the daily pool a usable size.
    let mut src = Vec::from(b"cj-pool5|".as_slice());
    src.extend_from_slice(h.as_bytes());
    blake2b_256(&src)[0] < 48
}

/// Registered pools: everyday 3–5 key characters, plus a slice of other 5-key codes.
pub fn registered_pool(chars: &[Glyph]) -> Vec<Glyph> {
    let mut have = HashSet::new();
    let mut pool = Vec::new();
    for g in chars {
        if !han_playable(g) || !have.insert(g.h.clone()) {
            continue;
        }
        let take = if g.n == 5 {
            g.f == 1 || keep_extra_five(&g.h)
        } else {
            g.f == 1 && g.n >= 3
        };
        if take {
            pool.push(g.clone());
        }
    }
    pool
}

/// ADA per Cangjie letter `a`–`z`. Inverse of how often the key shows up in
/// 五倉世紀: 重/難/手/田 are scarce, 一/竹/月/人 are everywhere.
pub fn letter_stake(letter: char) -> i32 {
    match letter.to_ascii_lowercase() {
        'a' => 3, // 日
        'b' => 1, // 月
        'c' => 3, // 金
        'd' => 2, // 木
        'e' => 3, // 水
        'f' => 2, // 火
        'g' => 3, // 土
        'h' => 1, // 竹
        'i' => 1, // 戈
        'j' => 2, // 十
        'k' => 2, // 大
        'l' => 2, // 中
        'm' => 1, // 一
        'n' => 1, // 弓
        'o' => 1, // 人
        'p' => 3, // 心
        'q' => 4, // 手
        'r' => 1, // 口
        's' => 3, // 尸
        't' => 2, // 廿
        'u' => 3, // 山
        'v' => 3, // 女
        'w' => 4, // 田
        'x' => 6, // 難
        'y' => 2, // 卜
        'z' => 8, // 重
        _ => 1,
    }
}

/// Stake is the sum of letter weights in the code. Everyday characters double.
pub fn stake_of(g: &Glyph) -> i32 {
    let mut s: i32 = g.c.chars().map(letter_stake).sum();
    if s < 1 {
        s = 1;
    }
    if g.f == 1 {
        s *= 2;
    }
    s
}

fn blake2b_256(data: &[u8]) -> [u8; 32] {
    *Hasher::<256>::hash(data)
}

/// Uniform in (0, 1] from the first 7 bytes of a digest (56 bits < 2^53).
pub fn unit_y(digest: &[u8]) -> f64 {
    let mut n: u64 = 0;
    for b in digest.iter().take(7) {
        n = n * 256 + u64::from(*b);
    }
    (n as f64 + 1.0) / 72_057_594_037_927_936.0
}

pub fn phi(sigma: f64) -> f64 {
    if sigma <= 0.0 {
        0.0
    } else {
        1.0 - (1.0 - ACTIVE_SLOT_F).powf(sigma)
    }
}

/// Ouroboros Praos *shape*: η = H(date) via pallas Blake2b-256. Slot s, pool i
/// is elected iff Y(η, s, i) < φ(σ_i). Several leaders → lowest Y keeps the slot.
///
/// This is **not** Cardano Praos. Real Praos uses a keyed VRF (you cannot
/// compute Y without a secret, and you publish a proof). Here Y is just
/// Blake2b of public bytes. `f` is 0.8, not Cardano’s ~0.05. A winner sits
/// out (no second slot). Stake is made-up letter weights, not ADA.
pub fn elect_lesson(pool: &[Glyph], day_key: &str, n: usize) -> Vec<Glyph> {
    elect(pool, day_key, n, 0, &HashSet::new())
}

/// Another ten after the daily set. `round` ≥ 1 changes η; `used` characters sit out.
pub fn elect_more(
    pool: &[Glyph],
    day_key: &str,
    n: usize,
    round: u32,
    used: &HashSet<String>,
) -> Vec<Glyph> {
    elect(pool, day_key, n, round, used)
}

fn elect(
    pool: &[Glyph],
    day_key: &str,
    n: usize,
    round: u32,
    used: &HashSet<String>,
) -> Vec<Glyph> {
    let mut eta_src = Vec::from(b"ouroboros|cangjie-daily|".as_slice());
    eta_src.extend_from_slice(day_key.as_bytes());
    if round > 0 {
        eta_src.extend_from_slice(b"|extra=");
        eta_src.extend_from_slice(round.to_string().as_bytes());
    }
    let eta = blake2b_256(&eta_src);
    let mut live: Vec<i32> = pool
        .iter()
        .map(|g| if used.contains(&g.h) { 0 } else { stake_of(g) })
        .collect();
    let mut total: i32 = live.iter().sum();
    let mut lesson = Vec::with_capacity(n);
    let mut slot = 0u32;
    let mut guard = 0u32;
    while lesson.len() < n && total > 0 && guard < 20_000 {
        slot += 1;
        guard += 1;
        let mut best_i: Option<usize> = None;
        let mut best_y = 2.0;
        for i in 0..pool.len() {
            let ada = live[i];
            if ada <= 0 {
                continue;
            }
            let mut vrf = Vec::with_capacity(64);
            vrf.extend_from_slice(&eta);
            vrf.extend_from_slice(b"|vrf|s=");
            vrf.extend_from_slice(slot.to_string().as_bytes());
            vrf.push(b'|');
            vrf.extend_from_slice(pool[i].h.as_bytes());
            vrf.push(b'|');
            vrf.extend_from_slice(pool[i].c.as_bytes());
            let y = unit_y(&blake2b_256(&vrf));
            if y < phi(ada as f64 / total as f64) && y < best_y {
                best_y = y;
                best_i = Some(i);
            }
        }
        if let Some(i) = best_i {
            total -= live[i];
            live[i] = 0;
            lesson.push(pool[i].clone());
        }
    }
    lesson
}

pub fn format_day_key(year: i32, month: u32, day: u32) -> String {
    format!("{year:04}-{month:02}-{day:02}")
}

pub fn radical_for(letter: char, radicals: &HashMap<String, String>) -> Option<&str> {
    radicals
        .get(&letter.to_ascii_lowercase().to_string())
        .map(|s| s.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_y_is_in_unit_interval() {
        let d = blake2b_256(b"hello");
        let y = unit_y(&d);
        assert!(y > 0.0 && y <= 1.0);
    }

    #[test]
    fn five_key_codes_hold_more_ada() {
        let short = Glyph {
            h: "明".into(),
            c: "ab".into(),
            n: 2,
            f: 1,
        };
        let five = Glyph {
            h: "經".into(),
            c: "vmfim".into(),
            n: 5,
            f: 1,
        };
        assert!(stake_of(&five) > stake_of(&short));
    }

    #[test]
    fn rare_letters_weigh_more_than_common() {
        assert!(letter_stake('z') > letter_stake('x'));
        assert!(letter_stake('x') > letter_stake('q'));
        assert!(letter_stake('q') > letter_stake('a'));
        assert!(letter_stake('a') > letter_stake('m'));
        let hard = Glyph {
            h: "難".into(),
            c: "x".into(),
            n: 1,
            f: 0,
        };
        let easy = Glyph {
            h: "一".into(),
            c: "m".into(),
            n: 1,
            f: 0,
        };
        assert!(stake_of(&hard) > stake_of(&easy));
    }

    #[test]
    fn election_is_deterministic() {
        let pool: Vec<Glyph> = "一二三四五六七八九十上下不中大小人月日木水火土金"
            .chars()
            .enumerate()
            .map(|(i, h)| Glyph {
                h: h.to_string(),
                c: ((b'a' + (i as u8 % 26)) as char).to_string(),
                n: 1,
                f: 1,
            })
            .collect();
        let a = elect_lesson(&pool, "2026-04-08", SESSION_N);
        let b = elect_lesson(&pool, "2026-04-08", SESSION_N);
        let c = elect_lesson(&pool, "2026-04-09", SESSION_N);
        assert_eq!(a, b);
        assert_eq!(a.len(), SESSION_N);
        assert_ne!(a, c);
    }

    #[test]
    fn junk_and_bmp_filter() {
        let junk = Glyph {
            h: "氵".into(),
            c: "e".into(),
            n: 1,
            f: 0,
        };
        let han = Glyph {
            h: "水".into(),
            c: "e".into(),
            n: 1,
            f: 1,
        };
        assert!(!han_playable(&junk));
        assert!(han_playable(&han));
    }

    #[test]
    fn pool_keeps_five_key_and_drops_short_uncommon() {
        let chars = vec![
            Glyph {
                h: "經".into(),
                c: "vmfim".into(),
                n: 5,
                f: 1,
            },
            Glyph {
                h: "日".into(),
                c: "a".into(),
                n: 1,
                f: 1,
            },
            Glyph {
                h: "明".into(),
                c: "ab".into(),
                n: 2,
                f: 0,
            },
            Glyph {
                h: "事".into(),
                c: "jlll".into(),
                n: 4,
                f: 1,
            },
        ];
        let pool = registered_pool(&chars);
        let hs: Vec<&str> = pool.iter().map(|g| g.h.as_str()).collect();
        assert!(hs.contains(&"經"));
        assert!(hs.contains(&"事"));
        assert!(!hs.contains(&"日"));
        assert!(!hs.contains(&"明"));
    }

    #[test]
    fn extra_round_skips_used_and_differs() {
        let pool: Vec<Glyph> = "一二三四五六七八九十上下不中大小人月日木水火土金"
            .chars()
            .enumerate()
            .map(|(i, h)| Glyph {
                h: h.to_string(),
                c: ((b'a' + (i as u8 % 26)) as char).to_string(),
                n: 1,
                f: 1,
            })
            .collect();
        let daily = elect_lesson(&pool, "2026-04-08", SESSION_N);
        let used: HashSet<String> = daily.iter().map(|g| g.h.clone()).collect();
        let extra = elect_more(&pool, "2026-04-08", SESSION_N, 1, &used);
        let extra2 = elect_more(&pool, "2026-04-08", SESSION_N, 1, &used);
        assert_eq!(extra, extra2);
        assert_eq!(extra.len(), SESSION_N);
        for g in &extra {
            assert!(!used.contains(&g.h));
        }
        assert_ne!(daily, extra);
    }
}
