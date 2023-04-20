use mpi::request::scope;
use mpi::request::Request;

use super::chemistry::Chemistry;
use super::task::FluxData;
use crate::communication::DataByRank;
use crate::communication::DataCommunicator;
use crate::communication::Rank;
use crate::communication::SizedCommunicator;

type OutstandingRequest = mpi::ffi::MPI_Request;

pub struct SweepCommunicator<'comm, C: Chemistry> {
    communicator: &'comm mut DataCommunicator<FluxData<C>>,
    send_buffers: DataByRank<Vec<FluxData<C>>>,
    requests: DataByRank<Option<OutstandingRequest>>,
}

fn to_unscoped<'a, C: Chemistry>(
    scoped_request: Request<'a, [FluxData<C>], &mpi::request::LocalScope<'a>>,
) -> OutstandingRequest {
    // SAFETY:
    // We only overwrite the data in a send buffer whenever the previous request is finished.
    // We also await all requests before dropping the send buffers.
    unsafe { scoped_request.into_raw().0 }
}

impl<'comm, C: Chemistry> SweepCommunicator<'comm, C> {
    pub fn new(communicator: &'comm mut DataCommunicator<FluxData<C>>) -> Self {
        let send_buffers = DataByRank::from_communicator(communicator);
        let requests = DataByRank::from_communicator(communicator);
        Self {
            communicator,
            send_buffers,
            requests,
        }
    }

    pub fn count_remaining_to_send(&self) -> usize {
        self.send_buffers
            .iter()
            .map(|(_, buffer)| buffer.len())
            .sum()
    }

    pub fn update_pending_requests(&mut self) {
        for rank in self.communicator.other_ranks() {
            if self.requests[rank]
                .map(|request| self.request_completed(request))
                .unwrap_or(true)
            {
                self.requests[rank] = None;
                self.send_buffers[rank].clear();
            }
        }
    }

    pub fn try_send_all(&mut self, to_send: &mut DataByRank<Vec<FluxData<C>>>) {
        self.update_pending_requests();
        for (rank, data) in to_send.iter_mut() {
            if data.is_empty() {
                continue;
            }
            if self.requests[*rank].is_none() {
                self.send_buffers[*rank].append(data);
                self.requests[*rank] = scope(|scope| {
                    let scoped_request = self.communicator.immediate_send_vec(
                        scope,
                        *rank,
                        &self.send_buffers[*rank][..],
                    );
                    scoped_request.map(to_unscoped)
                });
            }
        }
    }

    pub fn try_recv(&mut self, rank: Rank) -> Option<Vec<FluxData<C>>> {
        self.communicator.try_receive_vec(rank)
    }

    fn request_completed(&self, mut request: OutstandingRequest) -> bool {
        use std::mem::MaybeUninit;

        use mpi::ffi;

        unsafe {
            let mut status = MaybeUninit::uninit();
            let mut flag = MaybeUninit::uninit();

            ffi::MPI_Test(&mut request, flag.as_mut_ptr(), status.as_mut_ptr());
            flag.assume_init() != 0
        }
    }

    fn wait_for_request(&self, rank: Rank, request: OutstandingRequest) {
        scope(|s| {
            let data = &self.send_buffers[rank];
            self.to_scoped_request(s, data, request).wait();
        });
    }

    fn to_scoped_request<'a, Sc: mpi::request::Scope<'a>>(
        &self,
        scope: Sc,
        data: &'a Vec<FluxData<C>>,
        request: OutstandingRequest,
    ) -> Request<'a, [FluxData<C>], Sc> {
        unsafe { Request::from_raw(request, data, scope) }
    }
}

// Make sure we cannot accidentally drop the send buffers while
// there are still pending MPI requests.
impl<'comm, C: Chemistry> Drop for SweepCommunicator<'comm, C> {
    fn drop(&mut self) {
        for (rank, request) in self.requests.iter() {
            if let Some(request) = request {
                self.wait_for_request(*rank, *request);
                return;
            }
        }
    }
}

impl<'comm, C: Chemistry> SizedCommunicator for SweepCommunicator<'comm, C> {
    fn size(&self) -> usize {
        self.communicator.size()
    }

    fn rank(&self) -> Rank {
        self.communicator.rank()
    }
}
