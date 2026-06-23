//! In-memory DNS response cache keyed by question.

use simple_dns::{Packet, Question, RCODE};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CacheKey {
    qname: String,
    qtype: u16,
    qclass: u16,
}

struct Entry {
    response: Vec<u8>,
    expires: Instant,
}

pub struct DnsCache {
    entries: HashMap<CacheKey, Entry>,
    max_entries: usize,
}

impl DnsCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries: max_entries.max(16),
        }
    }

    pub fn lookup(&self, query: &[u8]) -> Option<Vec<u8>> {
        let (key, id) = cache_key_and_id(query)?;
        let entry = self.entries.get(&key)?;
        if Instant::now() >= entry.expires {
            return None;
        }
        Some(rewrite_id(&entry.response, id))
    }

    pub fn store(&mut self, query: &[u8], response: &[u8]) {
        let Ok(packet) = Packet::parse(response) else {
            return;
        };
        if !is_cacheable_rcode(packet.rcode()) {
            return;
        }
        let Some(key) = cache_key(query) else {
            return;
        };
        if self.entries.len() >= self.max_entries {
            self.evict_one();
        }
        let ttl = min_answer_ttl(&packet).max(1);
        self.entries.insert(
            key,
            Entry {
                response: response.to_vec(),
                expires: Instant::now() + Duration::from_secs(u64::from(ttl)),
            },
        );
    }

    fn evict_one(&mut self) {
        if let Some(key) = self.entries.keys().next().cloned() {
            self.entries.remove(&key);
        }
    }
}

fn cache_key_and_id(query: &[u8]) -> Option<(CacheKey, u16)> {
    let id = u16::from_be_bytes(query.get(0..2)?.try_into().ok()?);
    cache_key(query).map(|k| (k, id))
}

fn cache_key(query: &[u8]) -> Option<CacheKey> {
    let packet = Packet::parse(query).ok()?;
    let question = packet.questions.first()?;
    Some(key_from_question(question))
}

fn key_from_question(question: &Question<'_>) -> CacheKey {
    CacheKey {
        qname: question.qname.to_string(),
        qtype: question.qtype.into(),
        qclass: question.qclass.into(),
    }
}

fn rewrite_id(response: &[u8], id: u16) -> Vec<u8> {
    let mut out = response.to_vec();
    if out.len() >= 2 {
        out[0..2].copy_from_slice(&id.to_be_bytes());
    }
    out
}

fn min_answer_ttl(packet: &Packet<'_>) -> u32 {
    packet.answers.iter().map(|rr| rr.ttl).min().unwrap_or(60)
}

fn is_cacheable_rcode(rcode: RCODE) -> bool {
    matches!(rcode, RCODE::NoError | RCODE::NameError)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rewrites_response_id() {
        assert_eq!(
            rewrite_id(&[0, 1, 0x81, 0], 0xabcd),
            vec![0xab, 0xcd, 0x81, 0]
        );
    }

    use simple_dns::{Name, CLASS, QCLASS, QTYPE, TYPE};

    #[test]
    fn cache_key_from_query_bytes() {
        let mut packet = Packet::new_query(1);
        packet.questions.push(Question::new(
            Name::new("example.com").unwrap(),
            QTYPE::TYPE(TYPE::A),
            QCLASS::CLASS(CLASS::IN),
            false,
        ));
        let bytes = packet.build_bytes_vec().unwrap();
        let key = cache_key(&bytes).unwrap();
        assert_eq!(key.qname, "example.com");
        assert_eq!(key.qtype, u16::from(QTYPE::TYPE(TYPE::A)));
    }
}
