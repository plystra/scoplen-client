// SPDX-License-Identifier: Apache-2.0

//! Binary streams to the frontend.
//!
//! Terminal output, transfer data, and other byte streams travel over Tauri
//! channels as raw binary frames, never as JSON
//! (`scoplen-docs/11-client-architecture.md` §4). The frontend receives each
//! frame as an `ArrayBuffer` (`web/app/src/ipc/frames.ts`).

use tauri::ipc::{Channel, InvokeResponseBody};

/// A channel that carries binary frames to the frontend.
pub struct FrameSender {
    channel: Channel<InvokeResponseBody>,
}

/// The frontend closed the channel or its window went away.
#[derive(Debug, thiserror::Error)]
#[error("the frame channel to the frontend is closed")]
pub struct ChannelClosed(#[source] tauri::Error);

impl FrameSender {
    /// Wraps a channel received as a command argument.
    pub fn new(channel: Channel<InvokeResponseBody>) -> FrameSender {
        FrameSender { channel }
    }

    /// Sends one frame. Frames arrive in the order they are sent, each intact.
    pub fn send(&self, frame: impl Into<Vec<u8>>) -> Result<(), ChannelClosed> {
        self.channel.send(InvokeResponseBody::Raw(frame.into())).map_err(ChannelClosed)
    }
}

#[cfg(test)]
mod tests {
    use super::FrameSender;
    use std::sync::{Arc, Mutex};
    use tauri::ipc::{Channel, InvokeResponseBody};

    #[test]
    fn frames_are_sent_raw_and_in_order() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let sink = received.clone();
        let channel = Channel::new(move |body| {
            sink.lock().unwrap().push(body);
            Ok(())
        });
        let sender = FrameSender::new(channel);
        sender.send(b"\x1b[1mbold".to_vec()).unwrap();
        sender.send(vec![0u8, 255, 10]).unwrap();
        sender.send(Vec::new()).unwrap();

        let received = received.lock().unwrap();
        let frames: Vec<&[u8]> = received
            .iter()
            .map(|body| match body {
                InvokeResponseBody::Raw(bytes) => bytes.as_slice(),
                InvokeResponseBody::Json(json) => panic!("frame sent as JSON: {json}"),
            })
            .collect();
        assert_eq!(frames, [&b"\x1b[1mbold"[..], &[0, 255, 10][..], &[][..]]);
    }

    #[test]
    fn a_closed_channel_is_reported() {
        let channel = Channel::new(|_| Err(tauri::Error::FailedToReceiveMessage));
        let error = FrameSender::new(channel).send(vec![1]).unwrap_err();
        assert_eq!(error.to_string(), "the frame channel to the frontend is closed");
    }
}
