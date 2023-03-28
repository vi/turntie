use std::net::SocketAddr;

use argh::FromArgs;
use bytes::Bytes;
use futures::{future::try_join, SinkExt, StreamExt, TryStreamExt};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

/// Use TURN server as a communication channel with movable ends
#[derive(FromArgs)]
/// Top-level command.
struct Cmd {
    #[argh(subcommand)]
    cmd: CmdEnum,
}

#[derive(FromArgs)]
#[argh(subcommand)]
enum CmdEnum {
    Tie(Tie),
    Connect(Connect),
}

/// Create two tied allocations and print two specifier lines for usage with 'turntie connect'.
#[derive(FromArgs)]
#[argh(subcommand, name = "tie")]
struct Tie {
    /// address of TURN server to create allocations in
    #[argh(positional)]
    turn_server: SocketAddr,

    /// username to authenticate on TURN server with
    #[argh(positional)]
    username: String,

    /// password to authenticate on TURN server with
    #[argh(positional)]
    password: String,
}

/// Connect to one of the endpoints created by 'turntie tie' and exchange stdin/stdout lines with the peer which connected to the other endpoint.
#[derive(FromArgs)]
#[argh(subcommand, name = "connect")]
struct Connect {
    /// serialized data describing the channel end
    #[argh(positional)]
    specifier: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let cmd: Cmd = argh::from_env();
    match cmd.cmd {
        CmdEnum::Tie(opts) => {
            let (s1, s2) = turntie::tie(opts.turn_server, opts.username, opts.password).await?;
            println!("{}", s1);
            println!("{}", s2);
        }
        CmdEnum::Connect(opts) => {
            let c = turntie::connect(&opts.specifier).await?;

            let si = tokio::io::stdin();
            let so = tokio::io::stdout();

            let lc = LinesCodec::new();
            let r = FramedRead::new(si, lc.clone());
            let w = FramedWrite::new(so, lc);
            let r = r.err_into();
            let w = w.sink_err_into();

            let (cw, cr) = c.split();

            let f1 = cr
                .map_ok(|b| {
                    let s: String = String::from_utf8_lossy(b.as_ref()).into_owned();
                    s
                })
                .forward(w);
            let f2 = r
                .map_ok(|line| {
                    let b: Bytes = line.into();
                    b
                })
                .forward(cw);
            let f = try_join(f1, f2);
            f.await?;
        }
    }
    Ok(())
}
