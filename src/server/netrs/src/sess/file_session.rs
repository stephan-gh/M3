/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use m3::cap::Selector;
use m3::cell::RefCell;
use m3::col::VarRingBuf;
use m3::com::{GateIStream, MemGate, Perm, RecvGate, SendGate};
use m3::errors::{Code, Error};
use m3::kif::CapRngDesc;
use m3::rc::Rc;
use m3::server::CapExchange;
use m3::tcu;
use m3::vfs::OpenFlags;

use crate::sess::sockets::*;
/*
struct FSWorkItem {
    session_ref: Rc<RefCell<FileSession>>,
    is_active: RefCell<bool>,
}

impl WorkItem for FSWorkItem {
    fn work(&self) {
        //Ticks the different todo states that can occure.
        if let Err(e) = self.session_ref.borrow_mut().handle_send_buffer() {
            log!(crate::LOG_DEF, "Failed to handle send_buffer: {}", e);
        }
        if let Err(e) = self.session_ref.borrow_mut().handle_pending_recv() {
            log!(crate::LOG_DEF, "failed to handle pending_recv: {}", e);
        }
        if let Err(e) = self.session_ref.borrow_mut().handle_pending_send() {
            log!(crate::LOG_DEF, "Failed to handle pending_send: {}", e);
        }
    }

    fn is_active(&self) -> bool {
        *self.is_active.borrow()
    }
}
*/
pub struct FileSession {
    sel: Selector,
    #[allow(dead_code)]
    srv_sel: Selector,
    #[allow(dead_code)]
    sgate: SendGate,
    #[allow(dead_code)]
    socket: Rc<RefCell<Socket>>,
    memory: Option<MemGate>,

    mode: u32,

    rbuf: VarRingBuf, //Probably not needed since there is a rx / tx buffer in the device right?
    sbuf: VarRingBuf,

    last_amount: usize,
    sending: bool,
    pending: Option<&'static m3::tcu::Message>,
    pending_gate: Option<RecvGate>,

    client_memep: Selector,
    client_memgate: Option<MemGate>,
}

impl Drop for FileSession {
    fn drop(&mut self) {
        self.handle_eof().expect("Failed to drop file session");
    }
}

//TODO File session is currently unused and needs to be implemented
#[allow(dead_code)]
impl FileSession {
    pub fn new(
        _crt: usize,
        srv_sel: Selector,
        socket: Rc<RefCell<Socket>>,
        mode: u32,
        rmemsize: usize,
        smemsize: usize,
    ) -> Result<Rc<RefCell<Self>>, Error> {
        //Alloc selector for self,
        let sels = m3::pes::VPE::cur().alloc_sels(2);

        let label = 32 as tcu::Label;
        let new_sgate = SendGate::new_with(
            m3::com::SGateArgs::new(&socket.borrow().socket_session_rgate())
                .label(label)
                .credits(1)
                .sel(sels + 1), //put sgate on sel 1
        )?;

        log!(
            crate::LOG_DEF,
            "WARNING using not unique label in FileSession!"
        );
        let s = Rc::new(RefCell::new(FileSession {
            sel: sels,
            srv_sel,

            sgate: new_sgate,
            socket,
            memory: None,
            mode,
            rbuf: VarRingBuf::new(rmemsize),
            sbuf: VarRingBuf::new(smemsize),
            last_amount: 0,
            sending: false,
            pending: None,
            pending_gate: None,
            client_memep: m3::kif::INVALID_SEL,
            client_memgate: None,
        }));

        Ok(s)
    }

    pub fn is_recv(&self) -> bool {
        (self.mode & OpenFlags::R.bits()) > 0
    }

    pub fn is_send(&self) -> bool {
        (self.mode & OpenFlags::W.bits()) > 0
    }

    pub fn caps(&self) -> CapRngDesc {
        m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, self.sel, 2)
    }

    pub fn delegate(&mut self, xchg: &mut CapExchange) -> Result<(), Error> {
        // Client delegates shared memory to us
        if xchg.in_caps() == 1 && xchg.in_args().size() > 0 {
            let sel = m3::pes::VPE::cur().alloc_sel();
            self.memory = Some(MemGate::new_bind(sel));
            xchg.out_caps(m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, sel, 1));
        // Client delegates a memory endpoint to us for configuration
        }
        else if xchg.in_caps() == 1 && xchg.in_args().size() == 0 {
            let sel = m3::pes::VPE::cur().alloc_sel();
            self.client_memep = sel;
            xchg.out_caps(m3::kif::CapRngDesc::new(m3::kif::CapType::OBJECT, sel, 1));
        }
        else {
            return Err(Error::new(Code::InvArgs));
        }

        Ok(())
    }

    pub fn activate(&mut self) -> Result<(), Error> {
        if self.client_memep != m3::kif::INVALID_SEL {
            if self.memory.is_none() {
                return Err(Error::new(Code::InvArgs));
            }

            if self.client_memgate.is_none() {
                self.client_memgate = Some(self.memory.as_ref().unwrap().derive(
                    0,
                    self.rbuf.size() + self.sbuf.size(),
                    Perm::RW,
                )?);
            }

            m3::syscalls::activate(
                self.client_memep,
                self.client_memgate.as_ref().unwrap().sel(),
                m3::kif::INVALID_SEL,
                0,
            )?;
            self.client_memep = m3::kif::INVALID_SEL;
        }
        Ok(())
    }

    pub fn prepare(&mut self) -> Result<(), Error> {
        if self.pending.is_some() {
            log!(crate::LOG_DEF, "already has a pending request");
            return Err(Error::new(Code::Exists)); //Should be InvState
        }
        self.activate()
    }

    pub fn next_in(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        if !self.is_recv() {
            return Err(Error::new(Code::NotSup));
        }

        self.prepare()?;

        //TODO from C++: Socket is closed
        if false {
            log!(crate::LOG_DEF, "recv: EOF");
            reply_vmsg!(is, 0 as u32, 0 as usize, 0 as usize)?;
            return Ok(());
        }

        //implicitly commit the previous in request
        if !self.sending && self.last_amount != 0 {
            log!(
                crate::LOG_DEF,
                "recv: implicit commit of previous recv ({})",
                self.last_amount
            );
            self.inner_commit(self.last_amount)?;
        }

        self.sending = false;

        let amount = self.get_recv_size();
        if let Some((pos, amount)) = self.rbuf.get_read_pos(amount) {
            self.last_amount = amount;
            log!(crate::LOG_DEF, "recv: {}@{}", amount, pos);
            reply_vmsg!(is, 0 as u32, pos, amount)
        }
        else {
            //Could not allocate
            log!(crate::LOG_DEF, "recv: waiting for data");
            self.mark_pending(is)
        }
    }

    pub fn next_out(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        if !self.is_send() {
            log!(crate::LOG_DEF, "recv: waiting for data");
            return Err(Error::new(Code::NotSup));
        }

        //TODO from C++: socket is closed
        if false {
            log!(crate::LOG_DEF, "send: EOF");
            reply_vmsg!(is, 0 as u32, 0 as usize, 0 as usize)?;
            return Ok(());
        }

        //implicitly commit the previous out request
        if self.last_amount != 0 {
            log!(
                crate::LOG_DEF,
                "recv: implicit commit of previous out recv ({})",
                self.last_amount
            );
            self.inner_commit(self.last_amount)?;
        }

        self.sending = true;

        let amount = self.get_send_size();
        if let Some(pos) = self.rbuf.get_write_pos(amount) {
            self.last_amount = amount;
            log!(crate::LOG_DEF, "send: {}@{}", amount, pos);
            reply_vmsg!(is, 0 as u32, self.rbuf.size() + pos, amount)
        }
        else {
            //Could not allocate
            log!(crate::LOG_DEF, "send: waiting for free memory");
            self.mark_pending(is)
        }
    }

    pub fn close(&self, _is: &mut GateIStream) -> Result<(), Error> {
        Ok(())
    }

    pub fn commit(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        self.prepare()?;

        let amount: usize = is.pop()?;
        if amount == 0 {
            return Err(Error::new(Code::InvArgs));
        }

        let res = match self.inner_commit(amount) {
            Ok(_) => Code::None,
            Err(e) => e.code(),
        };

        if self.sending {
            reply_vmsg!(is, res as u32, self.sbuf.size())
        }
        else {
            reply_vmsg!(is, res as u32, self.rbuf.size())
        }
    }

    fn inner_commit(&mut self, amount: usize) -> Result<(), Error> {
        if amount != 0 && amount > self.last_amount {
            return Err(Error::new(Code::InvArgs));
        }

        if self.sending {
            //Advance write pointer
            self.sbuf.push(self.last_amount, amount);
            log!(crate::LOG_DEF, "push-send: {} -> {:?}", amount, self.sbuf);
        }
        else {
            //advance read pointer
            let pullam = if amount != 0 {
                amount
            }
            else {
                self.last_amount
            };
            self.rbuf.pull(pullam);
            log!(crate::LOG_DEF, "pull-recv: {} -> {:?}", amount, self.rbuf);
        }

        self.last_amount = 0;
        Ok(())
    }

    fn get_recv_size(&self) -> usize {
        self.rbuf.size() / 4
    }

    fn get_send_size(&self) -> usize {
        self.sbuf.size() / 4
    }

    fn handle_recv(&mut self, buf: &[u8]) -> Result<(), Error> {
        if self.memory.is_none() {
            return Err(Error::new(Code::OutOfMem));
        }

        let amount = buf.len();
        if let Some(pos) = self.rbuf.get_write_pos(amount) {
            self.memory.as_ref().unwrap().write(buf, pos as u64)?;
            log!(crate::LOG_DEF, "push-recv: {} -> {:?}", amount, self.rbuf);
            self.rbuf.push(amount, amount);
            Ok(())
        }
        else {
            Err(Error::new(Code::OutOfMem))
        }
    }

    fn mark_pending(&mut self, is: &mut GateIStream) -> Result<(), Error> {
        assert!(self.pending.is_none());

        //Since in Rust we cant just copy the pointer to the stream,
        //we take the message and create a new gate for the same selector with the same size.
        log!(
            crate::LOG_DEF,
            "mark stream pending on gate: {:?}",
            is.rgate()
        );
        let msg = is.take_msg();
        let cloned_gate = RecvGate::new_bind(
            is.rgate().sel(),
            m3::math::next_log2(is.rgate().size()),
            m3::math::next_log2(is.size()),
        );

        self.pending = Some(msg);
        self.pending_gate = Some(cloned_gate);
        Ok(())
    }

    fn handle_eof(&mut self) -> Result<(), Error> {
        if let (Some(pending_msg), Some(pending_gate)) =
            (self.pending.take(), self.pending_gate.take())
        {
            //send eof
            log!(crate::LOG_DEF, "Closing: Sending EOF");

            let mut late_is = GateIStream::new(pending_msg, &pending_gate);

            //TODO encode correctly?
            reply_vmsg!(late_is, 0 as usize, 0 as usize, 0 as usize)
        }
        else {
            log!(crate::LOG_DEF, "Closing: Could not send EOF");
            Ok(())
        }
    }

    fn handle_send_buffer(&mut self) -> Result<(), Error> {
        //Always has a socket

        //Currently processing just one chunk. Might change to process all pending.
        let amount = self.get_send_size();
        if let Some((pos, amount)) = self.sbuf.get_read_pos(amount) {
            log!(
                crate::LOG_DEF,
                "handle_send_buffer: amount={}, pos={}",
                amount,
                pos
            );

            //Read memory from memgate into vec, then send over the socket
            //TODO why is rbug size added to pos when reading?
            let _data = self
                .memory
                .as_ref()
                .unwrap()
                .read_into_vec::<u8>(amount, (self.rbuf.size() + pos) as u64)?;
            panic!("Cannot send data over file session");
            /*
                    match self.socket.borrow_mut().send_data_slice(&data, amount) {
                            Ok(size) => {
                                self.sbuf.pull(size);
                                log!(crate::LOG_DEF, "pull-send: {} -> {:?}", size, self.sbuf);
                            },
                            Err(e) => {
                                log!(crate::LOG_DEF, "Failed to send data over socket: {}", e);
                            },
                        }
            */
        }

        Ok(())
    }

    fn handle_pending_recv(&mut self) -> Result<(), Error> {
        if !self.pending.is_some() || !self.sending {
            return Ok(());
        }

        let amount = self.get_recv_size();
        if let Some((pos, amount)) = self.rbuf.get_read_pos(amount) {
            self.last_amount = amount;
            log!(crate::LOG_DEF, "late-recv: {}@{}", amount, pos);

            if let (Some(pending_msg), Some(pending_gate)) =
                (self.pending.take(), self.pending_gate.take())
            {
                let mut late_is = GateIStream::new(pending_msg, &pending_gate);

                reply_vmsg!(late_is, 0 as u32, pos, amount)
            }
            else {
                log!(crate::LOG_DEF, "Failed to send late reply for pending_recv");
                Ok(())
            }
        }
        else {
            Ok(())
        }
    }

    fn handle_pending_send(&mut self) -> Result<(), Error> {
        if self.pending.is_none() || !self.sending {
            return Ok(());
        }

        let amount = self.get_send_size();
        if let Some(pos) = self.sbuf.get_write_pos(amount) {
            //TODO: from C++:  maybe fallback to a smaller chunk?
            self.last_amount = amount;
            log!(crate::LOG_DEF, "late-send: {}@{}", amount, pos);
            if let (Some(pending_msg), Some(pending_gate)) =
                (self.pending.take(), self.pending_gate.take())
            {
                let mut late_is = GateIStream::new(pending_msg, &pending_gate);
                reply_vmsg!(late_is, 0 as u32, pos, amount)
            }
            else {
                log!(crate::LOG_DEF, "Failed to send late reply for pending_send");
                Ok(())
            }
        }
        else {
            Ok(())
        }
    }
}
