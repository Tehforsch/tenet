use std::cmp::Ordering;
use std::fmt::Debug;
use std::marker::PhantomData;

use bevy::prelude::debug;
use bevy::prelude::warn;
use bevy::prelude::Resource;

use super::key::Key;
use super::work::Work;
use super::Extent;
use crate::communication::communicator::Communicator;
use crate::communication::Rank;

const LOAD_IMBALANCE_WARN_THRESHOLD: f64 = 0.1;

struct Segment<K> {
    start: K,
    end: K,
}

impl<K: Debug> Debug for Segment<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}-{:?}", self.start, self.end)
    }
}

pub trait LoadCounter<K: Key> {
    fn load_in_range(&mut self, start: K, end: K) -> Work;

    fn total_load(&mut self) -> Work {
        self.load_in_range(K::MIN_VALUE, K::MAX_VALUE)
    }
}

fn binary_search<T: Key>(
    start: T,
    end: T,
    mut eval: impl FnMut(T, usize) -> Ordering,
    depth: usize,
) -> T {
    let pos = T::middle(start, end);
    let res = eval(pos, depth);
    match res {
        Ordering::Less => binary_search(pos, end, eval, depth + 1),
        Ordering::Greater => binary_search(start, pos, eval, depth + 1),
        Ordering::Equal => pos,
    }
}

#[derive(Resource)]
pub struct Decomposition<K> {
    num_ranks: usize,
    cuts: Vec<K>,
    loads: Vec<Work>,
}

impl<K: Key> Decomposition<K> {
    pub fn new<'a, C: LoadCounter<K>>(counter: &'a mut C, num_ranks: usize) -> Self {
        let total_load = counter.total_load();
        let num_segments = num_ranks;
        let load_per_segment = total_load / (num_segments as f64);
        let mut dd = Decomposer {
            counter,
            num_segments,
            load_per_segment,
            _marker: PhantomData,
        };
        let segments = dd.run();
        let loads = dd.get_loads(&segments);
        let cuts = segments.iter().map(|seg| seg.end).collect();
        Self {
            cuts,
            loads,
            num_ranks,
        }
    }

    pub(crate) fn rank_owns_part_of_search_radius(&self, _rank: Rank, _extent: Extent) -> bool {
        todo!()
    }

    pub(crate) fn get_owning_rank(&self, key: K) -> Rank {
        self.cuts
            .binary_search(&key)
            .map(|x| x + 1)
            .unwrap_or_else(|e| e) as i32
    }

    pub fn get_imbalance(&self) -> f64 {
        let min_load = self.min_load();
        let max_load = self.max_load();
        ((max_load - min_load) / max_load).0
    }

    fn min_load(&self) -> Work {
        *self.loads.iter().min().unwrap()
    }

    fn max_load(&self) -> Work {
        *self.loads.iter().max().unwrap()
    }

    pub(crate) fn log_imbalance(&self) {
        let load_imbalance = self.get_imbalance();
        if self.num_ranks != 1 {
            if load_imbalance > LOAD_IMBALANCE_WARN_THRESHOLD {
                warn!(
                    "Load imbalance: {:.1}%, max load: {:.0}, min load: {:.0}",
                    (load_imbalance * 100.0),
                    self.max_load().0,
                    self.min_load().0
                );
            } else {
                debug!("Load imbalance: {:.1}%", (load_imbalance * 100.0));
            }
        }
    }
}

struct Decomposer<'a, K: Key, C: LoadCounter<K>> {
    counter: &'a mut C,
    num_segments: usize,
    load_per_segment: Work,
    _marker: PhantomData<K>,
}

impl<'a, K: Key, C: LoadCounter<K>> Decomposer<'a, K, C> {
    fn run(&mut self) -> Vec<Segment<K>> {
        let segments = self.find_segments();
        segments
    }

    fn find_segments(&mut self) -> Vec<Segment<K>> {
        let mut segments = vec![];
        let mut start = K::MIN_VALUE;
        for _ in 0..self.num_segments - 1 {
            let end = self.find_cut_for_next_segment(start);
            segments.push(Segment { start, end });
            start = end;
        }
        segments.push(Segment {
            start,
            end: K::MAX_VALUE,
        });
        segments
    }

    fn find_cut_for_next_segment(&mut self, start: K) -> K {
        let get_search_result_for_cut = |cut, depth| {
            let load = self.counter.load_in_range(start, cut);
            self.get_search_result(load, depth)
        };
        let cut = binary_search(start, K::MAX_VALUE, get_search_result_for_cut, 0);
        cut
    }

    fn get_search_result(&self, load: Work, depth: usize) -> Ordering {
        if depth == K::MAX_DEPTH {
            Ordering::Equal
        } else {
            load.partial_cmp(&self.load_per_segment).unwrap()
        }
    }

    fn get_loads(&mut self, segments: &[Segment<K>]) -> Vec<Work> {
        segments
            .iter()
            .map(|s| self.counter.load_in_range(s.start, s.end))
            .collect()
    }
}

pub struct KeyCounter<K> {
    keys: Vec<K>,
}

impl<K: Key> KeyCounter<K> {
    pub fn new(mut keys: Vec<K>) -> Self {
        keys.sort();
        Self { keys }
    }
}

impl<K: Key> LoadCounter<K> for KeyCounter<K> {
    fn load_in_range(&mut self, start: K, end: K) -> Work {
        let start = self.keys.binary_search(&start).unwrap_or_else(|e| e);
        let end = self
            .keys
            .binary_search(&end)
            .map(|x| x + 1)
            .unwrap_or_else(|e| e);
        Work(end as f64 - start as f64)
    }
}

pub struct ParallelCounter<'a, K> {
    pub local_counter: KeyCounter<K>,
    pub comm: &'a mut Communicator<Work>,
}

impl<'a, K: Key> LoadCounter<K> for ParallelCounter<'a, K> {
    fn load_in_range(&mut self, start: K, end: K) -> Work {
        let local_work = self.local_counter.load_in_range(start, end);
        let all_work = self.comm.all_gather(&local_work);
        all_work.into_iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::Decomposition;
    use super::Key;
    use super::KeyCounter;
    use crate::domain::Extent;
    use crate::peano_hilbert::PeanoHilbertKey;
    use crate::test_utils::get_particles;
    use crate::units::VecLength;

    #[derive(PartialOrd, Ord, Copy, Clone, PartialEq, Eq, Debug)]
    struct Key1d(pub u64);

    impl Key for Key1d {
        const MIN_VALUE: Key1d = Key1d(0u64);
        const MAX_VALUE: Key1d = Key1d(u64::MAX);
        const MAX_DEPTH: usize = 64;

        fn middle(start: Self, end: Self) -> Self {
            Self(end.0 / 2 + start.0 / 2)
        }
    }

    fn get_counter_1d(vals: Vec<f64>) -> KeyCounter<Key1d> {
        let min = *vals
            .iter()
            .min_by(|x, y| x.partial_cmp(y).unwrap())
            .unwrap();
        let max = *vals
            .iter()
            .max_by(|x, y| x.partial_cmp(y).unwrap())
            .unwrap();
        let keys: Vec<_> = vals
            .into_iter()
            .map(|val| Key1d(((val - min) / (max - min) * u64::MAX as f64) as u64))
            .collect();
        KeyCounter::new(keys)
    }

    fn get_point_set_1(num_points: usize) -> Vec<f64> {
        (0..num_points).map(|x| x as f64).collect()
    }

    fn get_point_set_2(num_points: usize) -> Vec<f64> {
        (0..num_points / 2)
            .map(|x| x as f64)
            .chain((0..num_points / 2).map(|x| x as f64 * 1e-5))
            .collect()
    }

    fn get_point_set_3(num_points: usize) -> Vec<f64> {
        (0..num_points / 3)
            .map(|x| x as f64 * 0.64)
            .chain((0..num_points / 3).map(|x| x as f64 * 0.0000001))
            .chain((0..num_points / 3).map(|x| x as f64 * 1e-15))
            .collect()
    }

    #[test]
    fn domain_decomp_1d() {
        let num_points_per_rank = 5000;
        for get_point_set in [get_point_set_1, get_point_set_2, get_point_set_3] {
            for num_ranks in 1..100 {
                let num_points = num_points_per_rank * num_ranks;
                let vals = get_point_set(num_points);
                let mut counter = get_counter_1d(vals);
                let decomposition = Decomposition::new(&mut counter, num_ranks);
                let imbalance = decomposition.get_imbalance();
                println!("{} {:.3}%", num_ranks, imbalance * 100.0);
                assert!(imbalance < 0.05);
            }
        }
    }

    fn get_counter_3d(vals: Vec<VecLength>) -> KeyCounter<PeanoHilbertKey> {
        let extent = Extent::from_positions(vals.iter()).unwrap();
        let keys: Vec<_> = vals
            .into_iter()
            .map(|val| PeanoHilbertKey::from_point_and_extent_3d(val, &extent))
            .collect();
        KeyCounter::new(keys)
    }

    fn get_point_set_3d_1(num_points: usize) -> Vec<VecLength> {
        let n = (num_points as f64).sqrt() as i32;
        get_particles(n, n).into_iter().map(|p| p.pos).collect()
    }

    #[test]
    fn domain_decomp_3d() {
        let num_points_per_rank = 1000;
        for get_point_set in [get_point_set_3d_1] {
            for num_ranks in 1..100 {
                let num_points = num_points_per_rank * num_ranks;
                let vals = get_point_set(num_points);
                let mut counter = get_counter_3d(vals);
                let decomposition = Decomposition::new(&mut counter, num_ranks);
                let imbalance = decomposition.get_imbalance();
                println!("{} {:.3}%", num_ranks, imbalance * 100.0);
                assert!(imbalance < 0.05);
            }
        }
    }
}
