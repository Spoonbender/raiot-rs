use std::mem;

use raiot_protocol::{SubError, SubRes, qos::PacketId};

pub(crate) type SubErrorHandler = dyn Fn(SubError);
pub(crate) type MsgHandler<M> = dyn Fn(M);

/// Topic subscription state
pub(crate) enum SubState<M> {
    /// Not subscribed
    Unsubscribed,

    /// In process of subscribing (SUBSCRIBE sent)
    Subscribing(Box<MsgHandler<M>>, Box<SubErrorHandler>, PacketId),

    /// Subscribed (SUBACK received with positive code)
    Subscribed(Box<MsgHandler<M>>),
}

impl<M> SubState<M> {
    /// Completes the subscription in case the packet ID matches the subscription's packet ID
    pub fn try_complete(&mut self, res: &SubRes) -> bool {
        *self = if let SubState::Subscribing(ref mut msg_handler, 
                                             ref mut error_handler, 
                                             packet_id) = *self {
            if packet_id == res.packet_id {
                match res.result {
                    Ok(()) => {
                        SubState::Subscribed(mem::replace(msg_handler, Box::new(|_| { })))
                    }
                    Err(e) => { 
                        error_handler(e);
                        SubState::Unsubscribed
                    }
                }
            } else {
                return false;
            }
        } else {
            return false;
        };

        return true;
    }
}