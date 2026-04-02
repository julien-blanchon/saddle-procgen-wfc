#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainBits {
    words: Vec<u64>,
    bit_len: usize,
}

impl DomainBits {
    pub fn empty(bit_len: usize) -> Self {
        Self {
            words: vec![0; bit_len.div_ceil(64)],
            bit_len,
        }
    }

    pub fn full(bit_len: usize) -> Self {
        let mut bits = Self::empty(bit_len);
        for word in &mut bits.words {
            *word = u64::MAX;
        }
        bits.clear_unused_bits();
        bits
    }

    pub fn singleton(bit_len: usize, index: usize) -> Self {
        let mut bits = Self::empty(bit_len);
        bits.insert(index);
        bits
    }

    pub fn count(&self) -> usize {
        self.words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }

    pub fn is_singleton(&self) -> bool {
        self.count() == 1
    }

    pub fn first_one(&self) -> Option<usize> {
        for (word_index, word) in self.words.iter().enumerate() {
            if *word != 0 {
                return Some(word_index * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    pub fn contains(&self, index: usize) -> bool {
        let word_index = index / 64;
        let bit_index = index % 64;
        self.words
            .get(word_index)
            .is_some_and(|word| (word & (1u64 << bit_index)) != 0)
    }

    pub fn insert(&mut self, index: usize) {
        let word_index = index / 64;
        let bit_index = index % 64;
        if let Some(word) = self.words.get_mut(word_index) {
            *word |= 1u64 << bit_index;
        }
    }

    pub fn and_assign(&mut self, other: &Self) -> bool {
        let mut changed = false;
        for (left, right) in self.words.iter_mut().zip(&other.words) {
            let next = *left & *right;
            changed |= next != *left;
            *left = next;
        }
        changed
    }

    pub fn or_assign(&mut self, other: &Self) {
        for (left, right) in self.words.iter_mut().zip(&other.words) {
            *left |= *right;
        }
    }

    pub fn difference(&self, other: &Self) -> Self {
        let mut next = self.clone();
        for (left, right) in next.words.iter_mut().zip(&other.words) {
            *left &= !*right;
        }
        next
    }

    pub fn to_indices(&self) -> Vec<usize> {
        let mut indices = Vec::with_capacity(self.count());
        for (word_index, mut word) in self.words.iter().copied().enumerate() {
            while word != 0 {
                let bit = word.trailing_zeros() as usize;
                let index = word_index * 64 + bit;
                if index < self.bit_len {
                    indices.push(index);
                }
                word &= word - 1;
            }
        }
        indices
    }

    fn clear_unused_bits(&mut self) {
        let unused = self.words.len() * 64 - self.bit_len;
        if unused == 0 {
            return;
        }
        if let Some(last) = self.words.last_mut() {
            *last &= u64::MAX >> unused;
        }
    }
}
