use mpi::traits::Equivalence;
use subsweep::communication::DataByRank;
use subsweep::communication::ExchangeCommunicator;
use subsweep::communication::Rank;
use subsweep::hash_map::HashMap;
use subsweep::hash_map::HashSet;
use subsweep::prelude::ParticleId;

use super::UniqueParticleId;

#[derive(Equivalence, Clone, Debug, PartialEq, Eq, Hash)]
struct IdLookupRequest {
    id: UniqueParticleId,
}

#[derive(Equivalence)]
struct IdLookupReply {
    request_id: UniqueParticleId,
    id: ParticleId,
}

pub struct IdCache {
    map: HashMap<UniqueParticleId, ParticleId>,
    rank: Rank,
    requests: HashSet<IdLookupRequest>,
}

impl IdCache {
    pub fn new(map: HashMap<UniqueParticleId, ParticleId>, rank: Rank) -> Self {
        IdCache {
            map,
            rank,
            requests: HashSet::default(),
        }
    }

    pub fn lookup(&mut self, id: UniqueParticleId) -> Option<ParticleId> {
        self.map.get(&id).copied()
    }

    pub fn is_local(&self, id: UniqueParticleId) -> bool {
        self.map
            .get(&id)
            .map(|id| id.rank == self.rank)
            .unwrap_or(false)
    }

    pub fn perform_lookup(&mut self) {
        let mut request_comm: ExchangeCommunicator<IdLookupRequest> = ExchangeCommunicator::new();
        let mut reply_comm: ExchangeCommunicator<IdLookupReply> = ExchangeCommunicator::new();
        // For now: ask everyone everything
        let requests: Vec<_> = self.requests.drain().collect();
        let incoming_requests = request_comm.exchange_same_for_all(requests);
        let mut outgoing_replies = DataByRank::empty();
        for (rank, incoming_requests) in incoming_requests.iter() {
            let outgoing_replies_this_rank: Vec<_> = incoming_requests
                .iter()
                .filter_map(|incoming_request| {
                    self.lookup(incoming_request.id).map(|id| IdLookupReply {
                        request_id: incoming_request.id,
                        id,
                    })
                })
                .collect();
            outgoing_replies.insert(rank, outgoing_replies_this_rank);
        }
        let incoming_replies = reply_comm.exchange_all(outgoing_replies);
        for (_, incoming_replies) in incoming_replies {
            self.map.extend(
                incoming_replies
                    .into_iter()
                    .map(|reply| (reply.request_id, reply.id)),
            );
        }
    }

    pub fn add_lookup_request_if_necessary(&mut self, id: UniqueParticleId) {
        if !self.is_local(id) {
            self.requests.insert(IdLookupRequest { id });
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = ParticleId> + '_ {
        self.map.iter().map(|(_, id)| *id)
    }
}
