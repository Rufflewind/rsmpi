//! Point to point communication
//!
//! Endpoints of communication are mostly described by types that implement the `Source` and
//! `Destination` trait. Communication operations are implemented as default methods on those
//! traits.
//!
//! # Unfinished features
//!
//! - **3.2.6**: `MPI_STATUS_IGNORE`
//! - **3.6**: Buffer usage, `MPI_Buffer_attach()`, `MPI_Buffer_detach()`
//! - **3.9**: Persistent requests, `MPI_Send_init()`, `MPI_Bsend_init()`, `MPI_Ssend_init()`,
//! `MPI_Rsend_init()`, `MPI_Recv_init()`, `MPI_Start()`, `MPI_Startall()`

use std::{mem, fmt};
use std::os::raw::c_int;

use conv::ConvUtil;

use super::{Count, Tag};

use ffi;
use ffi::{MPI_Status, MPI_Message, MPI_Request};

use datatype::traits::*;
use raw::traits::*;
use request::{Request, Scope, StaticScope};
use topology::{Rank, Process, AnyProcess, CommunicatorRelation};
use topology::traits::*;

// TODO: rein in _with_tag ugliness, use optional tags or make tag part of Source and Destination

/// Point to point communication traits
pub mod traits {
    pub use super::{Source, Destination, MatchedReceiveVec};
}

/// Something that can be used as the source in a point to point receive operation
///
/// # Examples
///
/// - A `Process` used as a source for a receive operation will receive data only from the
/// identified process.
/// - A communicator can also be used as a source via the `AnyProcess` identifier.
///
/// # Standard section(s)
///
/// 3.2.3
pub unsafe trait Source: AsCommunicator {
    /// `Rank` that identifies the source
    fn source_rank(&self) -> Rank;

    /// Probe a source for incoming messages.
    ///
    /// Probe `Source` `&self` for incoming messages with a certain tag.
    ///
    /// An ordinary `probe()` returns a `Status` which allows inspection of the properties of the
    /// incoming message, but does not guarantee reception by a subsequent `receive()` (especially
    /// in a multi-threaded set-up). For a probe operation with stronger guarantees, see
    /// `matched_probe()`.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.1
    fn probe_with_tag(&self, tag: Tag) -> Status {
        let mut status: MPI_Status = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Probe(self.source_rank(),
                           tag,
                           self.as_communicator().as_raw(),
                           &mut status);
        };
        Status(status)
    }

    /// Probe a source for incoming messages.
    ///
    /// Probe `Source` `&self` for incoming messages with any tag.
    ///
    /// An ordinary `probe()` returns a `Status` which allows inspection of the properties of the
    /// incoming message, but does not guarantee reception by a subsequent `receive()` (especially
    /// in a multi-threaded set-up). For a probe operation with stronger guarantees, see
    /// `matched_probe()`.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.1
    fn probe(&self) -> Status {
        self.probe_with_tag(unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
    }

    /// Probe a source for incoming messages with guaranteed reception.
    ///
    /// Probe `Source` `&self` for incoming messages with a certain tag.
    ///
    /// A `matched_probe()` returns both a `Status` that describes the properties of a pending
    /// incoming message and a `Message` which can and *must* subsequently be used in a
    /// `matched_receive()` to receive the probed message.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.2
    fn matched_probe_with_tag(&self, tag: Tag) -> (Message, Status) {
        let mut message: MPI_Message = unsafe { mem::uninitialized() };
        let mut status: MPI_Status = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Mprobe(self.source_rank(),
                            tag,
                            self.as_communicator().as_raw(),
                            &mut message,
                            &mut status);
        }
        (Message(message), Status(status))
    }

    /// Probe a source for incoming messages with guaranteed reception.
    ///
    /// Probe `Source` `&self` for incoming messages with any tag.
    ///
    /// A `matched_probe()` returns both a `Status` that describes the properties of a pending
    /// incoming message and a `Message` which can and *must* subsequently be used in a
    /// `matched_receive()` to receive the probed message.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.2
    fn matched_probe(&self) -> (Message, Status) {
        self.matched_probe_with_tag(unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
    }

    /// Receive a message containing a single instance of type `Msg`.
    ///
    /// Receive a message from `Source` `&self` tagged `tag` containing a single instance of type
    /// `Msg`.
    ///
    /// # Standard section(s)
    ///
    /// 3.2.4
    fn receive_with_tag<Msg>(&self, tag: Tag) -> (Msg, Status)
        where Msg: Equivalence
    {
        let mut res: Msg = unsafe { mem::uninitialized() };
        let status = self.receive_into_with_tag(&mut res, tag);
        if status.count(Msg::equivalent_datatype()) == 0 {
            panic!("Received an empty message.");
        }
        (res, status)
    }

    /// Receive a message containing a single instance of type `Msg`.
    ///
    /// Receive a message from `Source` `&self` containing a single instance of type `Msg`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mpi::traits::*;
    ///
    /// let universe = mpi::initialize().unwrap();
    /// let world = universe.world();
    ///
    /// let x = world.any_process().receive::<f64>();
    /// ```
    ///
    /// # Standard section(s)
    ///
    /// 3.2.4
    fn receive<Msg>(&self) -> (Msg, Status)
        where Msg: Equivalence
    {
        self.receive_with_tag(unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
    }

    /// Receive a message into a `Buffer`.
    ///
    /// Receive a message from `Source` `&self` tagged `tag` into `Buffer` `buf`.
    ///
    /// # Standard section(s)
    ///
    /// 3.2.4
    fn receive_into_with_tag<Buf: ?Sized>(&self, buf: &mut Buf, tag: Tag) -> Status
        where Buf: BufferMut
    {
        let mut status: MPI_Status = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Recv(buf.pointer_mut(),
                          buf.count(),
                          buf.as_datatype().as_raw(),
                          self.source_rank(),
                          tag,
                          self.as_communicator().as_raw(),
                          &mut status);
        }
        Status(status)
    }

    /// Receive a message into a `Buffer`.
    ///
    /// Receive a message from `Source` `&self` into `Buffer` `buf`.
    ///
    /// # Standard section(s)
    ///
    /// 3.2.4
    fn receive_into<Buf: ?Sized>(&self, buf: &mut Buf) -> Status
        where Buf: BufferMut
    {
        self.receive_into_with_tag(buf, unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
    }

    /// Receive a message containing multiple instances of type `Msg` into a `Vec`.
    ///
    /// Receive a message from `Source` `&self` tagged `tag` containing multiple instances of type
    /// `Msg` into a `Vec`.
    ///
    /// # Standard section(s)
    ///
    /// 3.2.4
    fn receive_vec_with_tag<Msg>(&self, tag: Tag) -> (Vec<Msg>, Status)
        where Msg: Equivalence
    {
        self.matched_probe_with_tag(tag).matched_receive_vec()
    }

    /// Receive a message containing multiple instances of type `Msg` into a `Vec`.
    ///
    /// Receive a message from `Source` `&self` containing multiple instances of type `Msg` into a
    /// `Vec`.
    ///
    /// # Examples
    /// See `examples/send_receive.rs`
    ///
    /// # Standard section(s)
    ///
    /// 3.2.4
    fn receive_vec<Msg>(&self) -> (Vec<Msg>, Status)
        where Msg: Equivalence
    {
        self.receive_vec_with_tag(unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
    }

    /// Initiate an immediate (non-blocking) receive operation.
    ///
    /// Initiate receiving a message matching `tag` into `buf`.
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_receive_into_with_tag<'a, Sc, Buf: ?Sized>(&self,
                                                            scope: Sc,
                                                            buf: &'a mut Buf,
                                                            tag: Tag)
                                                            -> Request<'a, Sc>
        where Buf: 'a + BufferMut,
              Sc: Scope<'a>
    {
        let mut request: MPI_Request = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Irecv(buf.pointer_mut(),
                           buf.count(),
                           buf.as_datatype().as_raw(),
                           self.source_rank(),
                           tag,
                           self.as_communicator().as_raw(),
                           &mut request);
            Request::from_raw(request, scope)
        }
    }

    /// Initiate an immediate (non-blocking) receive operation.
    ///
    /// Initiate receiving a message into `buf`.
    ///
    /// # Examples
    /// See `examples/immediate.rs`
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_receive_into<'a, Sc, Buf: ?Sized>(&self,
                                                   scope: Sc,
                                                   buf: &'a mut Buf)
                                                   -> Request<'a, Sc>
        where Buf: 'a + BufferMut,
              Sc: Scope<'a>
    {
        self.immediate_receive_into_with_tag(scope, buf, unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
    }

    /// Initiate a non-blocking receive operation for messages matching tag `tag`.
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_receive_with_tag<Msg>(&self, tag: Tag) -> ReceiveFuture<Msg>
        where Msg: Equivalence
    {
        let mut val: Box<Msg> = Box::new(unsafe { mem::uninitialized() });
        let mut req: MPI_Request = unsafe { mem::uninitialized() };

        unsafe {
            ffi::MPI_Irecv((&mut *(val)).pointer_mut(),
                           val.count(),
                           Msg::equivalent_datatype().as_raw(),
                           self.source_rank(),
                           tag,
                           self.as_communicator().as_raw(),
                           &mut req);
            ReceiveFuture {
                val: val,
                req: Request::from_raw(req, StaticScope)
            }
        }
    }

    /// Initiate a non-blocking receive operation.
    ///
    /// # Examples
    /// See `examples/immediate.rs`
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_receive<Msg>(&self) -> ReceiveFuture<Msg>
        where Msg: Equivalence
    {
        self.immediate_receive_with_tag(unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
    }

    /// Asynchronously probe a source for incoming messages.
    ///
    /// Asynchronously probe `Source` `&self` for incoming messages with a certain tag.
    ///
    /// Like `Probe` but returns a `None` immediately if there is no incoming message to be probed.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.1
    fn immediate_probe_with_tag(&self, tag: Tag) -> Option<Status> {
        let mut status: MPI_Status = unsafe { mem::uninitialized() };
        let mut flag: c_int = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Iprobe(self.source_rank(),
                            tag,
                            self.as_communicator().as_raw(),
                            &mut flag,
                            &mut status);
        };
        if flag != 0 {
            Some(Status(status))
        } else {
            None
        }
    }

    /// Asynchronously probe a source for incoming messages.
    ///
    /// Asynchronously probe `Source` `&self` for incoming messages with any tag.
    ///
    /// Like `Probe` but returns a `None` immediately if there is no incoming message to be probed.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.1
    fn immediate_probe(&self) -> Option<Status> {
        self.immediate_probe_with_tag(unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
    }

    /// Asynchronously probe a source for incoming messages with guaranteed reception.
    ///
    /// Asynchronously probe `Source` `&self` for incoming messages with a certain tag.
    ///
    /// Like `MatchedProbe` but returns a `None` immediately if there is no incoming message to be
    /// probed.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.2
    fn immediate_matched_probe_with_tag(&self, tag: Tag) -> Option<(Message, Status)> {
        let mut message: MPI_Message = unsafe { mem::uninitialized() };
        let mut status: MPI_Status = unsafe { mem::uninitialized() };
        let mut flag: c_int = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Improbe(self.source_rank(),
                             tag,
                             self.as_communicator().as_raw(),
                             &mut flag,
                             &mut message,
                             &mut status);
        }
        if flag != 0 {
            Some((Message(message), Status(status)))
        } else {
            None
        }
    }

    /// Asynchronously probe a source for incoming messages with guaranteed reception.
    ///
    /// Asynchronously probe `Source` `&self` for incoming messages with any tag.
    ///
    /// Like `MatchedProbe` but returns a `None` immediately if there is no incoming message to be
    /// probed.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.2
    fn immediate_matched_probe(&self) -> Option<(Message, Status)> {
        self.immediate_matched_probe_with_tag(unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
    }
}

unsafe impl<C> Source for AnyProcess<C> where C: Communicator
{
    fn source_rank(&self) -> Rank {
        unsafe_extern_static!(ffi::RSMPI_ANY_SOURCE)
    }
}

unsafe impl<C> Source for Process<C> where C: Communicator
{
    fn source_rank(&self) -> Rank {
        self.rank()
    }
}

/// Something that can be used as the destination in a point to point send operation
///
/// # Examples
/// - Using a `Process` as the destination will send data to that specific process.
///
/// # Standard section(s)
///
/// 3.2.3
pub trait Destination: AsCommunicator {
    /// `Rank` that identifies the destination
    fn destination_rank(&self) -> Rank;

    /// Blocking standard mode send operation
    ///
    /// Send the contents of a `Buffer` to the `Destination` `&self` and tag it.
    ///
    /// # Standard section(s)
    ///
    /// 3.2.1
    fn send_with_tag<Buf: ?Sized>(&self, buf: &Buf, tag: Tag)
        where Buf: Buffer
    {
        unsafe {
            ffi::MPI_Send(buf.pointer(),
                          buf.count(),
                          buf.as_datatype().as_raw(),
                          self.destination_rank(),
                          tag,
                          self.as_communicator().as_raw());
        }
    }

    /// Blocking standard mode send operation
    ///
    /// Send the contents of a `Buffer` to the `Destination` `&self`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use mpi::traits::*;
    ///
    /// let universe = mpi::initialize().unwrap();
    /// let world = universe.world();
    ///
    /// let v = vec![ 1.0f64, 2.0, 3.0 ];
    /// world.process_at_rank(1).send(&v[..]);
    /// ```
    ///
    /// See also `examples/send_receive.rs`
    ///
    /// # Standard section(s)
    ///
    /// 3.2.1
    fn send<Buf: ?Sized>(&self, buf: &Buf)
        where Buf: Buffer
    {
        self.send_with_tag(buf, Tag::default())
    }

    /// Blocking buffered mode send operation
    ///
    /// Send the contents of a `Buffer` to the `Destination` `&self` and tag it.
    ///
    /// # Standard section(s)
    ///
    /// 3.4
    fn buffered_send_with_tag<Buf: ?Sized>(&self, buf: &Buf, tag: Tag)
        where Buf: Buffer
    {
        unsafe {
            ffi::MPI_Bsend(buf.pointer(),
                           buf.count(),
                           buf.as_datatype().as_raw(),
                           self.destination_rank(),
                           tag,
                           self.as_communicator().as_raw());
        }
    }

    /// Blocking buffered mode send operation
    ///
    /// Send the contents of a `Buffer` to the `Destination` `&self`.
    ///
    /// # Standard section(s)
    ///
    /// 3.4
    fn buffered_send<Buf: ?Sized>(&self, buf: &Buf)
        where Buf: Buffer
    {
        self.buffered_send_with_tag(buf, Tag::default())
    }

    /// Blocking synchronous mode send operation
    ///
    /// Send the contents of a `Buffer` to the `Destination` `&self` and tag it.
    ///
    /// Completes only once the matching receive operation has started.
    ///
    /// # Standard section(s)
    ///
    /// 3.4
    fn synchronous_send_with_tag<Buf: ?Sized>(&self, buf: &Buf, tag: Tag)
        where Buf: Buffer
    {
        unsafe {
            ffi::MPI_Ssend(buf.pointer(),
                           buf.count(),
                           buf.as_datatype().as_raw(),
                           self.destination_rank(),
                           tag,
                           self.as_communicator().as_raw());
        }
    }

    /// Blocking synchronous mode send operation
    ///
    /// Send the contents of a `Buffer` to the `Destination` `&self`.
    ///
    /// Completes only once the matching receive operation has started.
    ///
    /// # Standard section(s)
    ///
    /// 3.4
    fn synchronous_send<Buf: ?Sized>(&self, buf: &Buf)
        where Buf: Buffer
    {
        self.synchronous_send_with_tag(buf, Tag::default())
    }

    /// Blocking ready mode send operation
    ///
    /// Send the contents of a `Buffer` to the `Destination` `&self` and tag it.
    ///
    /// Fails if the matching receive operation has not been posted.
    ///
    /// # Standard section(s)
    ///
    /// 3.4
    fn ready_send_with_tag<Buf: ?Sized>(&self, buf: &Buf, tag: Tag)
        where Buf: Buffer
    {
        unsafe {
            ffi::MPI_Rsend(buf.pointer(),
                           buf.count(),
                           buf.as_datatype().as_raw(),
                           self.destination_rank(),
                           tag,
                           self.as_communicator().as_raw());
        }
    }

    /// Blocking ready mode send operation
    ///
    /// Send the contents of a `Buffer` to the `Destination` `&self`.
    ///
    /// Fails if the matching receive operation has not been posted.
    ///
    /// # Standard section(s)
    ///
    /// 3.4
    fn ready_send<Buf: ?Sized>(&self, buf: &Buf)
        where Buf: Buffer
    {
        self.ready_send_with_tag(buf, Tag::default())
    }

    /// Initiate an immediate (non-blocking) standard mode send operation.
    ///
    /// Initiate sending the data in `buf` in standard mode and tag it.
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_send_with_tag<'a, Sc, Buf: ?Sized>(&self,
                                                    scope: Sc,
                                                    buf: &'a Buf,
                                                    tag: Tag)
                                                    -> Request<'a, Sc>
        where Buf: 'a + Buffer,
              Sc: Scope<'a>
    {
        let mut request: MPI_Request = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Isend(buf.pointer(),
                           buf.count(),
                           buf.as_datatype().as_raw(),
                           self.destination_rank(),
                           tag,
                           self.as_communicator().as_raw(),
                           &mut request);
            Request::from_raw(request, scope)
        }
    }

    /// Initiate an immediate (non-blocking) standard mode send operation.
    ///
    /// Initiate sending the data in `buf` in standard mode.
    ///
    /// # Examples
    /// See `examples/immediate.rs`
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_send<'a, Sc, Buf: ?Sized>(&self, scope: Sc, buf: &'a Buf) -> Request<'a, Sc>
        where Buf: 'a + Buffer,
              Sc: Scope<'a>
    {
        self.immediate_send_with_tag(scope, buf, Tag::default())
    }

    /// Initiate an immediate (non-blocking) buffered mode send operation.
    ///
    /// Initiate sending the data in `buf` in buffered mode and tag it.
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_buffered_send_with_tag<'a, Sc, Buf: ?Sized>(&self,
                                                             scope: Sc,
                                                             buf: &'a Buf,
                                                             tag: Tag)
                                                             -> Request<'a, Sc>
        where Buf: 'a + Buffer,
              Sc: Scope<'a>
    {
        let mut request: MPI_Request = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Ibsend(buf.pointer(),
                            buf.count(),
                            buf.as_datatype().as_raw(),
                            self.destination_rank(),
                            tag,
                            self.as_communicator().as_raw(),
                            &mut request);
            Request::from_raw(request, scope)
        }
    }

    /// Initiate an immediate (non-blocking) buffered mode send operation.
    ///
    /// Initiate sending the data in `buf` in buffered mode.
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_buffered_send<'a, Sc, Buf: ?Sized>(&self, scope: Sc, buf: &'a Buf)
                                                    -> Request<'a, Sc>
        where Buf: 'a + Buffer,
              Sc: Scope<'a>
    {
        self.immediate_buffered_send_with_tag(scope, buf, Tag::default())
    }

    /// Initiate an immediate (non-blocking) synchronous mode send operation.
    ///
    /// Initiate sending the data in `buf` in synchronous mode and tag it.
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_synchronous_send_with_tag<'a, Sc, Buf: ?Sized>(&self,
                                                                scope: Sc,
                                                                buf: &'a Buf,
                                                                tag: Tag)
                                                                -> Request<'a, Sc>
        where Buf: 'a + Buffer,
              Sc: Scope<'a>
    {
        let mut request: MPI_Request = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Issend(buf.pointer(),
                            buf.count(),
                            buf.as_datatype().as_raw(),
                            self.destination_rank(),
                            tag,
                            self.as_communicator().as_raw(),
                            &mut request);
            Request::from_raw(request, scope)
        }
    }

    /// Initiate an immediate (non-blocking) synchronous mode send operation.
    ///
    /// Initiate sending the data in `buf` in synchronous mode.
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_synchronous_send<'a, Sc, Buf: ?Sized>(&self,
                                                       scope: Sc,
                                                       buf: &'a Buf)
                                                       -> Request<'a, Sc>
        where Buf: 'a + Buffer,
              Sc: Scope<'a>
    {
        self.immediate_synchronous_send_with_tag(scope, buf, Tag::default())
    }

    /// Initiate an immediate (non-blocking) ready mode send operation.
    ///
    /// Initiate sending the data in `buf` in ready mode and tag it.
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_ready_send_with_tag<'a, Sc, Buf: ?Sized>(&self,
                                                          scope: Sc,
                                                          buf: &'a Buf,
                                                          tag: Tag)
                                                          -> Request<'a, Sc>
        where Buf: 'a + Buffer,
              Sc: Scope<'a>
    {
        let mut request: MPI_Request = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Irsend(buf.pointer(),
                            buf.count(),
                            buf.as_datatype().as_raw(),
                            self.destination_rank(),
                            tag,
                            self.as_communicator().as_raw(),
                            &mut request);
            Request::from_raw(request, scope)
        }
    }

    /// Initiate an immediate (non-blocking) ready mode send operation.
    ///
    /// Initiate sending the data in `buf` in ready mode.
    ///
    /// # Examples
    ///
    /// See `examples/immediate.rs`
    ///
    /// # Standard section(s)
    ///
    /// 3.7.2
    fn immediate_ready_send<'a, Sc, Buf: ?Sized>(&self, scope: Sc, buf: &'a Buf)
                                                 -> Request<'a, Sc>
        where Buf: 'a + Buffer,
              Sc: Scope<'a>
    {
        self.immediate_ready_send_with_tag(scope, buf, Tag::default())
    }
}

impl<C> Destination for Process<C> where C: Communicator
{
    fn destination_rank(&self) -> Rank {
        self.rank()
    }
}

/// Describes the result of a point to point receive operation.
///
/// # Standard section(s)
///
/// 3.2.5
#[derive(Copy, Clone)]
pub struct Status(MPI_Status);

impl Status {
    /// Construct a `Status` value from the raw MPI type
    pub fn from_raw(status: MPI_Status) -> Status {
        Status(status)
    }

    /// The rank of the message source
    pub fn source_rank(&self) -> Rank {
        self.0.MPI_SOURCE
    }

    /// The message tag
    pub fn tag(&self) -> Tag {
        self.0.MPI_TAG
    }

    /// Number of instances of the type contained in the message
    pub fn count<D: Datatype>(&self, d: D) -> Count {
        let mut count: Count = unsafe { mem::uninitialized() };
        unsafe { ffi::MPI_Get_count(&self.0, d.as_raw(), &mut count) };
        count
    }
}

impl fmt::Debug for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f,
               "Status {{ source_rank: {}, tag: {} }}",
               self.source_rank(),
               self.tag())
    }
}

/// Describes a pending incoming message, probed by a `matched_probe()`.
///
/// # Standard section(s)
///
/// 3.8.2
#[must_use]
#[derive(Debug)]
pub struct Message(MPI_Message);

impl Message {
    /// True if the `Source` for the probe was the null process.
    pub fn is_no_proc(&self) -> bool {
        self.as_raw() == unsafe_extern_static!(ffi::RSMPI_MESSAGE_NO_PROC)
    }

    /// Receive a previously probed message containing a single instance of type `Msg`.
    ///
    /// Receives the message `&self` which contains a single instance of type `Msg`.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.3
    pub fn matched_receive<Msg>(self) -> (Msg, Status)
        where Msg: Equivalence
    {
        let mut res: Msg = unsafe { mem::uninitialized() };
        let status = self.matched_receive_into(&mut res);
        if status.count(Msg::equivalent_datatype()) == 0 {
            panic!("Received an empty message.");
        }
        (res, status)
    }

    /// Receive a previously probed message into a `Buffer`.
    ///
    /// Receive the message `&self` with contents matching `buf`.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.3
    pub fn matched_receive_into<Buf: ?Sized>(mut self, buf: &mut Buf) -> Status
        where Buf: BufferMut
    {
        let mut status: MPI_Status = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Mrecv(buf.pointer_mut(),
                           buf.count(),
                           buf.as_datatype().as_raw(),
                           self.as_raw_mut(),
                           &mut status);
            assert_eq!(self.as_raw(), ffi::RSMPI_MESSAGE_NULL);
        }
        Status(status)
    }

    /// Asynchronously receive a previously probed message into a `Buffer`.
    ///
    /// Asynchronously receive the message `&self` with contents matching `buf`.
    ///
    /// # Standard section(s)
    ///
    /// 3.8.3
    pub fn immediate_matched_receive_into<'a, Sc, Buf: ?Sized + 'a>(mut self,
                                                                    scope: Sc,
                                                                    buf: &'a mut Buf)
                                                                    -> Request<'a, Sc>
        where Buf: BufferMut,
              Sc: Scope<'a>
    {
        let mut request: MPI_Request = unsafe { mem::uninitialized() };
        unsafe {
            ffi::MPI_Imrecv(buf.pointer_mut(),
                            buf.count(),
                            buf.as_datatype().as_raw(),
                            self.as_raw_mut(),
                            &mut request);
            assert_eq!(self.as_raw(), ffi::RSMPI_MESSAGE_NULL);
            Request::from_raw(request, scope)
        }
    }
}

unsafe impl AsRaw for Message {
    type Raw = MPI_Message;
    fn as_raw(&self) -> Self::Raw {
        self.0
    }
}

unsafe impl AsRawMut for Message {
    fn as_raw_mut(&mut self) -> *mut <Self as AsRaw>::Raw {
        &mut self.0
    }
}

impl Drop for Message {
    fn drop(&mut self) {
        assert_eq!(self.as_raw(), unsafe_extern_static!(ffi::RSMPI_MESSAGE_NULL),
                "matched message dropped without receiving.");
    }
}

/// Receive a previously probed message containing multiple instances of type `Msg` into a `Vec`.
///
/// # Standard section(s)
///
/// 3.8.3
pub trait MatchedReceiveVec {
    /// Receives the message `&self` which contains multiple instances of type `Msg` into a `Vec`.
    fn matched_receive_vec<Msg>(self) -> (Vec<Msg>, Status) where Msg: Equivalence;
}

impl MatchedReceiveVec for (Message, Status) {
    fn matched_receive_vec<Msg>(self) -> (Vec<Msg>, Status)
        where Msg: Equivalence
    {
        let (message, status) = self;
        let count = status.count(Msg::equivalent_datatype())
                          .value_as()
                          .expect("Message element count cannot be expressed as a usize.");
        let mut res = Vec::with_capacity(count);
        unsafe {
            res.set_len(count);
        }
        let status = message.matched_receive_into(&mut res[..]);
        (res, status)
    }
}

/// Sends `msg` to `destination` tagging it `sendtag` and simultaneously receives an
/// instance of `R` tagged `receivetag` from `source`.
///
/// # Standard section(s)
///
/// 3.10
pub fn send_receive_with_tags<M, D, R, S>(msg: &M,
                                          destination: &D,
                                          sendtag: Tag,
                                          source: &S,
                                          receivetag: Tag)
                                          -> (R, Status)
    where M: Equivalence,
          D: Destination,
          R: Equivalence,
          S: Source
{
    let mut res: R = unsafe { mem::uninitialized() };
    let status = send_receive_into_with_tags(msg,
                                             destination,
                                             sendtag,
                                             &mut res,
                                             source,
                                             receivetag);
    (res, status)
}

/// Sends `msg` to `destination` and simultaneously receives an instance of `R` from
/// `source`.
///
/// # Examples
/// See `examples/send_receive.rs`
///
/// # Standard section(s)
///
/// 3.10
pub fn send_receive<R, M, D, S>(msg: &M, destination: &D, source: &S) -> (R, Status)
    where M: Equivalence,
          D: Destination,
          R: Equivalence,
          S: Source
{
    send_receive_with_tags(msg, destination, Tag::default(), source, unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
}

/// Sends the contents of `msg` to `destination` tagging it `sendtag` and
/// simultaneously receives a message tagged `receivetag` from `source` into
/// `buf`.
///
/// # Standard section(s)
///
/// 3.10
pub fn send_receive_into_with_tags<M: ?Sized, D, B: ?Sized, S>(msg: &M,
                                                               destination: &D,
                                                               sendtag: Tag,
                                                               buf: &mut B,
                                                               source: &S,
                                                               receivetag: Tag)
                                                               -> Status
    where M: Buffer,
          D: Destination,
          B: BufferMut,
          S: Source
{
    assert_eq!(source.as_communicator().compare(destination.as_communicator()),
               CommunicatorRelation::Identical);
    let mut status: MPI_Status = unsafe { mem::uninitialized() };
    unsafe {
        ffi::MPI_Sendrecv(msg.pointer(),
                          msg.count(),
                          msg.as_datatype().as_raw(),
                          destination.destination_rank(),
                          sendtag,
                          buf.pointer_mut(),
                          buf.count(),
                          buf.as_datatype().as_raw(),
                          source.source_rank(),
                          receivetag,
                          source.as_communicator().as_raw(),
                          &mut status);
    }
    Status(status)
}

/// Sends the contents of `msg` to `destination` and
/// simultaneously receives a message from `source` into
/// `buf`.
///
/// # Standard section(s)
///
/// 3.10
pub fn send_receive_into<M: ?Sized, D, B: ?Sized, S>(msg: &M,
                                                     destination: &D,
                                                     buf: &mut B,
                                                     source: &S)
                                                     -> Status
    where M: Buffer,
          D: Destination,
          B: BufferMut,
          S: Source
{
    send_receive_into_with_tags(msg,
                                destination,
                                Tag::default(),
                                buf,
                                source,
                                unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
}

/// Sends the contents of `buf` to `destination` tagging it `sendtag` and
/// simultaneously receives a message tagged `receivetag` from `source` and replaces the
/// contents of `buf` with it.
///
/// # Standard section(s)
///
/// 3.10
pub fn send_receive_replace_into_with_tags<B: ?Sized, D, S>(buf: &mut B,
                                                            destination: &D,
                                                            sendtag: Tag,
                                                            source: &S,
                                                            receivetag: Tag)
                                                            -> Status
    where B: BufferMut,
          D: Destination,
          S: Source
{
    assert_eq!(source.as_communicator().compare(destination.as_communicator()),
               CommunicatorRelation::Identical);
    let mut status: MPI_Status = unsafe { mem::uninitialized() };
    unsafe {
        ffi::MPI_Sendrecv_replace(buf.pointer_mut(),
                                  buf.count(),
                                  buf.as_datatype().as_raw(),
                                  destination.destination_rank(),
                                  sendtag,
                                  source.source_rank(),
                                  receivetag,
                                  source.as_communicator().as_raw(),
                                  &mut status);
    }
    Status(status)
}

/// Sends the contents of `buf` to `destination` and
/// simultaneously receives a message from `source` and replaces the contents of
/// `buf` with it.
///
/// # Standard section(s)
///
/// 3.10
pub fn send_receive_replace_into<B: ?Sized, D, S>(buf: &mut B,
                                                  destination: &D,
                                                  source: &S)
                                                  -> Status
    where B: BufferMut,
          D: Destination,
          S: Source
{
    send_receive_replace_into_with_tags(buf,
                                        destination,
                                        Tag::default(),
                                        source,
                                        unsafe_extern_static!(ffi::RSMPI_ANY_TAG))
}

/// Will contain a value of type `T` received via a non-blocking receive operation.
#[must_use]
#[derive(Debug)]
pub struct ReceiveFuture<T> {
    val: Box<T>,
    req: Request<'static>,
}

impl<T> ReceiveFuture<T> where T: Equivalence {
    /// Wait for the receive operation to finish and return the received data.
    pub fn get(self) -> (T, Status) {
        let status = self.req.wait();
        if status.count(T::equivalent_datatype()) == 0 {
            panic!("Received an empty message into a ReceiveFuture.");
        }
        (*self.val, status)
    }

    /// Check whether the receive operation has finished.
    ///
    /// If the operation has finished, the data received is returned. Otherwise the future itself
    /// is returned.
    pub fn try(mut self) -> Result<(T, Status), Self> {
        match self.req.test() {
            Ok(status) => {
                if status.count(T::equivalent_datatype()) == 0 {
                    panic!("Received an empty message into a ReceiveFuture.");
                }
                Ok((*self.val, status))
            },
            Err(request) => {
                self.req = request;
                Err(self)
            }
        }
    }
}
