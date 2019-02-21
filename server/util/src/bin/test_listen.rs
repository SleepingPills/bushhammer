use mio;
use mio::net::TcpListener;
use std::net::SocketAddr;
use std::io::Read;

fn main() {
    let server_poll = mio::Poll::new().unwrap();
    let read_poll = mio::Poll::new().unwrap();
    let server = TcpListener::bind(&"127.0.0.1:28008".parse::<SocketAddr>().unwrap()).unwrap();

    server_poll
        .register(
            &server,
            mio::Token(0),
            mio::Ready::readable() | mio::Ready::writable(),
            mio::PollOpt::edge(),
        )
        .unwrap();

    let server_poll_box = Box::new(server_poll);

    let mut events = mio::Events::with_capacity(8192);

    server_poll_box.poll(&mut events, None).expect("Listen poll failed");

    let mut streams: Vec<mio::net::TcpStream> = events
        .iter()
        .map(|event| {
            println!("{:?}", event);
            return server.accept().unwrap().0;
        })
        .collect();

    events.clear();

    read_poll
        .register(
            &streams[0],
            mio::Token(0),
            mio::Ready::readable(),
            mio::PollOpt::level(),
        )
        .unwrap();

    loop {
        read_poll.poll(&mut events, None).unwrap();

        let mut data = Vec::new();

        for event in &events {
            println!("{:?}", event);
            let _res = streams[0].read_to_end(&mut data);
        }

        let str = String::from_utf8(data).unwrap();

        println!("{}", str);
    }

    events.clear();
}
