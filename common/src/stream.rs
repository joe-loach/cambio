use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpStream,
};
use tokio_serde::{formats::SymmetricalJson, Framed};
use tokio_util::codec::{FramedRead, FramedWrite, LengthDelimitedCodec};

pub type Read<Value> =
    Framed<FramedRead<OwnedReadHalf, LengthDelimitedCodec>, Value, Value, SymmetricalJson<Value>>;
pub type Write<Value> =
    Framed<FramedWrite<OwnedWriteHalf, LengthDelimitedCodec>, Value, Value, SymmetricalJson<Value>>;

pub fn split<R, W>(stream: TcpStream) -> (Read<R>, Write<W>) {
    let (r, w) = stream.into_split();

    let read = FramedRead::new(r, LengthDelimitedCodec::new());
    let write = FramedWrite::new(w, LengthDelimitedCodec::new());

    let read = tokio_serde::SymmetricallyFramed::new(read, SymmetricalJson::<R>::default());
    let write = tokio_serde::SymmetricallyFramed::new(write, SymmetricalJson::<W>::default());

    (read, write)
}
