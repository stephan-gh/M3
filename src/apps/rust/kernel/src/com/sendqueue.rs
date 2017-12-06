use base::cell::{MutCell, RefCell};
use base::col::{DList, Vec};
use base::dtu;
use base::rc::Rc;
use thread;

use arch::kdtu;
use pes::{State, VPE};

const SRV_REP: dtu::EpId = 2;

struct Entry {
    id: u64,
    rep: dtu::EpId,
    msg: Vec<u8>,
}

impl Entry {
    pub fn new(id: u64, rep: dtu::EpId, msg: Vec<u8>) -> Self {
        Entry {
            id: id,
            rep: rep,
            msg: msg,
        }
    }
}

#[derive(Eq, PartialEq)]
enum QState {
    Idle,
    Waiting,
    Aborted,
}

pub struct SendQueue {
    vpe: Rc<RefCell<VPE>>,
    queue: DList<Entry>,
    cur_event: thread::Event,
    state: QState,
}

fn alloc_qid() -> u64 {
    static NEXT_ID: MutCell<u64> = MutCell::new(0);
    NEXT_ID.set(NEXT_ID.get() + 1);
    *NEXT_ID.get()
}

fn get_event(id: u64) -> thread::Event {
    0x8000_0000_0000_0000 | id
}

impl SendQueue {
    pub fn new(vpe: &Rc<RefCell<VPE>>) -> Self {
        SendQueue {
            vpe: vpe.clone(),
            queue: DList::new(),
            cur_event: 0,
            state: QState::Idle,
        }
    }

    pub fn vpe(&self) -> &Rc<RefCell<VPE>> {
        &self.vpe
    }

    pub fn send(&mut self, rep: dtu::EpId, msg: &[u8], size: usize) -> Option<thread::Event> {
        klog!(SQUEUE, "SendQueue[{}]: trying to send msg", self.vpe.borrow().id());

        if self.state == QState::Aborted {
            return None;
        }

        if self.state == QState::Idle && self.vpe.borrow().state() == State::RUNNING {
            return Some(self.do_send(rep, alloc_qid(), msg, size));
        }

        klog!(SQUEUE, "SendQueue[{}]: queuing msg", self.vpe.borrow().id());

        let qid = alloc_qid();

        // copy message to heap
        let vec = msg.to_vec();
        self.queue.push_back(Entry::new(qid, rep, vec));
        Some(get_event(qid))
    }

    pub fn received_reply(&mut self, msg: &'static dtu::Message) {
        klog!(SQUEUE, "SendQueue[{}]: received reply", self.vpe.borrow().id());

        assert!(self.state == QState::Waiting);
        self.state = QState::Idle;

        thread::ThreadManager::get().notify(self.cur_event, Some(msg));

        // now that we've copied the message, we can mark it read
        dtu::DTU::mark_read(SRV_REP, msg);

        self.send_pending();
    }

    fn send_pending(&mut self) {
        match self.queue.pop_front() {
            None    => return,

            Some(e) => {
                klog!(SQUEUE, "SendQueue[{}]: found pending message", self.vpe.borrow().id());

                self.do_send(e.rep, e.id, &e.msg, e.msg.len());
            }
        }
    }

    fn do_send(&mut self, rep: dtu::EpId, id: u64, msg: &[u8], size: usize) -> thread::Event {
        klog!(SQUEUE, "SendQueue[{}]: sending msg", self.vpe.borrow().id());

        self.cur_event = get_event(id);
        self.state = QState::Waiting;

        let label = self as *mut Self as dtu::Label;
        kdtu::KDTU::get().send_to(
            &self.vpe.borrow().desc(), rep, 0, msg.as_ptr(), size, label, SRV_REP
        ).unwrap();

        self.cur_event
    }
}
