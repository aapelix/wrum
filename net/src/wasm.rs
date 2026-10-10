use std::{cell::Cell, rc::Rc};

use futures_channel::mpsc::{TryRecvError, UnboundedReceiver, UnboundedSender, unbounded};
use futures_util::StreamExt;
use js_sys::{Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    ReadableStream, ReadableStreamDefaultReader, WebTransport, WebTransportReceiveStream,
    WebTransportSendStream, WritableStreamDefaultWriter,
};

use crate::{Channel, Packet, Transport};

const MAX_PACKET_SIZE: usize = 1024 * 1024;

pub struct WasmTransport {
    transport: WebTransport,
    reliable_tx: UnboundedSender<Vec<u8>>,
    unreliable_tx: UnboundedSender<Vec<u8>>,
    incoming: UnboundedReceiver<Packet>,
    connected: Rc<Cell<bool>>,
}

impl WasmTransport {
    pub async fn open(url: &str) -> anyhow::Result<Self> {
        let transport = WebTransport::new(url).map_err(js_error)?;
        JsFuture::from(transport.ready()).await.map_err(js_error)?;

        let stream: WebTransportSendStream =
            JsFuture::from(transport.create_unidirectional_stream())
                .await
                .map_err(js_error)?
                .unchecked_into();

        let reliable_writer: WritableStreamDefaultWriter = stream.get_writer().map_err(js_error)?;
        let dg_writer = transport
            .datagrams()
            .writable()
            .get_writer()
            .map_err(js_error)?;

        let (reliable, mut reliable_tx) = unbounded::<Vec<u8>>();
        let (unreliable, mut unreliable_tx) = unbounded::<Vec<u8>>();
        let (incoming_tx, incoming) = unbounded::<Packet>();

        let connected = Rc::new(Cell::new(true));

        // reliable send
        {
            let connected = connected.clone();
            spawn_local(async move {
                while let Some(bytes) = reliable_tx.next().await {
                    let result = {
                        let mut framed = Vec::with_capacity(4 + bytes.len());
                        framed.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                        framed.extend_from_slice(&bytes);
                        write(&reliable_writer, &framed).await
                    };

                    if result.is_err() {
                        connected.set(false);
                        break;
                    }
                }
            });
        }

        // unreliable send
        {
            spawn_local(async move {
                while let Some(bytes) = unreliable_tx.next().await {
                    let _ = write(&dg_writer, &bytes).await;
                }
            });
        }

        // unreliable recv
        {
            let reader: ReadableStreamDefaultReader = transport
                .datagrams()
                .readable()
                .get_reader()
                .unchecked_into();

            let tx = incoming_tx.clone();
            let connected = connected.clone();

            spawn_local(async move {
                loop {
                    let res = match read(&reader).await {
                        Ok(Some(bytes)) => bytes,
                        Ok(None) | Err(_) => {
                            connected.set(false);
                            break;
                        }
                    };

                    if tx.unbounded_send((Channel::Unreliable, res)).is_err() {
                        break;
                    }
                }
            });
        }

        // reliable recv
        {
            let reader: ReadableStreamDefaultReader = transport
                .incoming_unidirectional_streams()
                .get_reader()
                .unchecked_into();

            let tx = incoming_tx;
            let connected = connected.clone();

            spawn_local(async move {
                loop {
                    let value = match read_value(&reader).await {
                        Ok(Some(value)) => value,
                        _ => {
                            connected.set(false);
                            break;
                        }
                    };
                    let stream: WebTransportReceiveStream = value.unchecked_into();
                    let tx = tx.clone();
                    spawn_local(async move {
                        if let Err(e) = read_realiable_stream(stream, tx).await {
                            web_sys::console::error_1(
                                &format!("reliable stream error: {e}").into(),
                            );
                        }
                    });
                }
            })
        }

        Ok(Self {
            transport,
            reliable_tx: reliable,
            unreliable_tx: unreliable,
            incoming,
            connected,
        })
    }
}

impl Transport for WasmTransport {
    fn send(&mut self, ch: Channel, bytes: &[u8]) -> anyhow::Result<()> {
        let sender = match ch {
            Channel::Reliable => {
                let max = self.transport.datagrams().max_datagram_size() as usize;
                anyhow::ensure!(
                    bytes.len() <= max,
                    "datagram too large: {} > {max}",
                    bytes.len()
                );
                &self.reliable_tx
            }
            Channel::Unreliable => &self.unreliable_tx,
        };
        sender
            .unbounded_send(bytes.to_vec())
            .map_err(|e| anyhow::anyhow!("send queue closed: {e}"))
    }
    fn poll(&mut self) -> anyhow::Result<Option<(Channel, Vec<u8>)>> {
        let r = self.incoming.try_recv();
        match r {
            Ok(pck) => {
                return Ok(Some(pck));
            }
            Err(TryRecvError::Empty) => {
                return Ok(None);
            }
            Err(e) => {
                return Err(anyhow::anyhow!("recv queue error: {e}"));
            }
        }
    }
    fn is_connected(&self) -> bool {
        self.connected.get()
    }
}

impl Drop for WasmTransport {
    fn drop(&mut self) {
        self.connected.set(false);
        self.transport.close();
    }
}

async fn write(writer: &WritableStreamDefaultWriter, bytes: &[u8]) -> anyhow::Result<()> {
    let chunk = Uint8Array::from(bytes);
    JsFuture::from(writer.write_with_chunk(&chunk))
        .await
        .map_err(js_error)?;
    Ok(())
}
async fn read(reader: &ReadableStreamDefaultReader) -> anyhow::Result<Option<Vec<u8>>> {
    let result = JsFuture::from(reader.read()).await.map_err(js_error)?;
    let done = Reflect::get(&result, &"done".into())
        .map_err(js_error)?
        .as_bool()
        .unwrap_or(false);
    if done {
        return Ok(None);
    }
    let value = Reflect::get(&result, &"value".into()).map_err(js_error)?;
    Ok(Some(Uint8Array::new(&value).to_vec()))
}
async fn read_value(reader: &ReadableStreamDefaultReader) -> anyhow::Result<Option<JsValue>> {
    let result = JsFuture::from(reader.read()).await.map_err(js_error)?;

    if Reflect::get(&result, &"done".into())
        .map_err(js_error)?
        .as_bool()
        .unwrap_or(false)
    {
        return Ok(None);
    }

    Ok(Some(
        Reflect::get(&result, &"value".into()).map_err(js_error)?,
    ))
}
async fn read_realiable_stream(
    stream: WebTransportReceiveStream,
    tx: UnboundedSender<Packet>,
) -> anyhow::Result<()> {
    let readable: ReadableStream = stream.unchecked_into();
    let reader: ReadableStreamDefaultReader = readable.get_reader().unchecked_into();

    let mut buffer = Vec::<u8>::new();

    while let Some(chunk) = read(&reader).await? {
        buffer.extend_from_slice(&chunk);
        loop {
            if buffer.len() < 4 {
                break;
            }
            let len = u32::from_le_bytes(buffer[..4].try_into()?) as usize;
            anyhow::ensure!(len <= MAX_PACKET_SIZE, "reliable packet too large");
            if buffer.len() < 4 + len {
                break;
            }
            let packet = buffer[4..4 + len].to_vec();
            buffer.drain(..4 + len);
            tx.unbounded_send((Channel::Reliable, packet))
                .map_err(|_| anyhow::anyhow!("recv queue closed"))?;
        }
    }

    Ok(())
}

fn js_error(e: wasm_bindgen::JsValue) -> anyhow::Error {
    anyhow::anyhow!("{e:?}")
}
