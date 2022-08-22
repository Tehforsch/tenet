mod extent;
pub mod quadtree;

use self::extent::Extent;
use crate::communication::DataByRank;
use crate::communication::Rank;
use crate::units::VecLength;

#[derive(Clone)]
pub struct DomainDistribution {
    domains: DataByRank<Vec<Extent>>,
}

impl DomainDistribution {
    pub fn target_rank(&self, pos: &VecLength) -> Rank {
        *self
            .domains
            .iter()
            .find(|(_, extents)| extents.iter().any(|extent| extent.contains(pos)))
            .map(|(rank, _)| rank)
            .expect("sum of domain extents does not cover all particles")
    }
}

#[cfg(test)]
mod tests {
    use super::extent::Extent;
    use super::DomainDistribution;
    use crate::communication::DataByRank;
    use crate::units::Length;
    use crate::units::VecLength;

    #[test]
    fn target_rank() {
        let total_extents = Extent::new(
            Length::meter(-100.0),
            Length::meter(100.0),
            Length::meter(-100.0),
            Length::meter(100.0),
        );
        let quadrants = total_extents.get_quadrants();
        let mut domains = DataByRank::empty();
        domains.insert(0, vec![quadrants[0].clone(), quadrants[1].clone()]);
        domains.insert(1, vec![quadrants[2].clone(), quadrants[3].clone()]);
        let distribution = DomainDistribution { domains };
        assert_eq!(distribution.target_rank(&VecLength::meter(-70.0, -70.0)), 0);
    }
}
