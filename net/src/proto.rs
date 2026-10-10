use crate::Delivery;

pub const CHANNELS: [Delivery; 3] = [
    Delivery::ReliableOrdered,     // 0: chat, spawns, events
    Delivery::UnreliableSequenced, // 1: state / positions
    Delivery::ReliableOrdered,     // 2: bulk data, so it can't stall chat
];
