use httparse::{Request, EMPTY_HEADER};
use rvimage_domain::{rverr, to_rv, RvResult};
use std::{
    fmt::Debug,
    io::{prelude::*, Read},
    net::TcpListener,
    str,
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};
use tracing::info;

#[derive(Debug, PartialEq)]
enum HandleResult {
    Path(String),
    Terminate,
}

fn handle_connection(buffer: &[u8]) -> RvResult<HandleResult> {
    let mut headers = [EMPTY_HEADER; 64];

    let mut req = Request::new(&mut headers);
    let res = req.parse(buffer);
    res.map_err(to_rv)?;
    match req.path {
        Some(p) => Ok(if p == "/TERMINATE" {
            HandleResult::Terminate
        } else {
            HandleResult::Path(
                percent_encoding::percent_decode_str(&p[1..])
                    .decode_utf8()
                    .map_err(to_rv)?
                    .to_string(),
            )
        }),
        None => {
            let msg = "could not find path in stream";
            match str::from_utf8(buffer) {
                Ok(b) => Err(rverr!("{} '{}'", msg, b)),
                Err(e) => Err(rverr!("{}, {:?}", msg, e)),
            }
        }
    }
}

fn to_rv_or_send<T, E>(tx: &Sender<RvResult<String>>, x: Result<T, E>) -> RvResult<T>
where
    E: Debug,
{
    match x {
        Ok(r) => Ok(r),
        Err(e) => {
            let error_str = format!("{e:?}");
            match tx.send(Err(to_rv(e))) {
                Ok(()) => Err(rverr!("error in http server, {}", error_str)),
                Err(e) => Err(rverr!("error {}, send error {:?}", error_str, e)),
            }
        }
    }
}
pub type LaunchResultType = RvResult<(JoinHandle<RvResult<()>>, Receiver<RvResult<String>>)>;
pub fn launch(address: String) -> LaunchResultType {
    info!("spawning httpserver at {address}");
    let (tx_from_server, rx_from_server) = mpsc::channel();
    let handle = thread::spawn(move || -> RvResult<()> {
        let bind_result = TcpListener::bind(address);
        let listener = to_rv_or_send(&tx_from_server, bind_result)?;
        let mut buffer = vec![0; 4096];
        // since the requests will only change the shown image, they will be handled sequentially
        for stream in listener.incoming() {
            let stream_processing_result: RvResult<HandleResult> = {
                let mut stream = stream.map_err(to_rv)?;
                stream.read(&mut buffer).map_err(to_rv)?;
                // we don't care about headers and bodies and strip everything after the first CRLF
                let buffer_slice = if let Some(idx) =
                    (0..(buffer.len() - 1)).find(|idx| &buffer[*idx..*idx + 2] == b"\r\n")
                {
                    &buffer[..idx + 2]
                } else {
                    &buffer
                };
                let response = "HTTP/1.1 200 OK\r\n\r\n";
                stream.write(response.as_bytes()).map_err(to_rv)?;
                stream.flush().map_err(to_rv)?;
                let res = handle_connection(buffer_slice);
                let path = res?;
                Ok(path)
            };
            // send recieved path
            if let Ok(p) = to_rv_or_send(&tx_from_server, stream_processing_result) {
                match p {
                    HandleResult::Terminate => {
                        info!("terminating httpserver");
                        return Ok(());
                    }
                    HandleResult::Path(p_) => {
                        info!("tcp listener sending result...");
                        let send_result = tx_from_server.send(Ok(p_));
                        info!("done. {send_result:?}");
                        to_rv_or_send(&tx_from_server, send_result)?;
                    }
                }
            }
            info!("tcp listener waiting for new input");
        }
        Ok(())
    });
    info!("...done");
    Ok((handle, rx_from_server))
}

fn increase_port(address: &str) -> RvResult<String> {
    let address_wo_port = address.split(':').next();
    let port = address.split(':').last();
    if let Some(port) = port {
        if let Some(address_wo_port) = address_wo_port {
            Ok(format!(
                "{}:{}",
                address_wo_port,
                (port.parse::<usize>().map_err(to_rv)? + 1)
            ))
        } else {
            Err(rverr!("is address of {} missing?", address))
        }
    } else {
        Err(rverr!("is port of address {} missing?", address))
    }
}

pub fn restart_with_increased_port(
    http_addr: &str,
) -> RvResult<(String, Option<Receiver<RvResult<String>>>)> {
    let http_addr = increase_port(http_addr)?;

    info!("restarting http server with increased port");
    Ok(if let Ok((_, rx)) = launch(http_addr.clone()) {
        (http_addr, Some(rx))
    } else {
        (http_addr, None)
    })
}
#[test]
fn test_handler() -> RvResult<()> {
    let buffer = b"garbage";
    assert!(handle_connection(buffer.as_slice()).is_err());

    let buffer = b"GET /index.html HTTP/1.1\r\nHost:";
    assert_eq!(
        handle_connection(buffer.as_slice()),
        Ok(HandleResult::Path("index.html".to_string()))
    );

    let buffer = b"GET /folder%20name/file%20name.png HTTP/1.1\r\nHost:";
    assert_eq!(
        handle_connection(buffer.as_slice()),
        Ok(HandleResult::Path("folder name/file name.png".to_string()))
    );
    let buffer = b"GET /TERMINATE HTTP/1.1\r\nHost:";
    assert_eq!(
        handle_connection(buffer.as_slice()),
        Ok(HandleResult::Terminate)
    );

    Ok(())
}
#[cfg(test)]
use std::{net::TcpStream, time::Duration};
#[test]
fn test_launch() -> RvResult<()> {
    let address = "127.0.0.1:7942";
    println!("launching server...");
    let (handle, rx) = launch(address.to_string())?;
    thread::sleep(Duration::from_millis(10));
    assert!(!handle.is_finished());
    println!("...done");

    let send_request = |req| -> RvResult<()> {
        let mut stream = TcpStream::connect(address).map_err(to_rv)?;
        stream.write(req).map_err(to_rv)?;
        stream.flush().map_err(to_rv)?;
        Ok(())
    };
    println!("writing to stream...");
    let input_stream = b"GET /some_path.png HTTP/1.1\r\nHost:";
    send_request(input_stream.as_slice())?;
    println!("...done");
    println!("writing to stream...");
    let input_stream = b"GET /some_other_path.png HTTP/1.1\r\nHost:";
    send_request(input_stream.as_slice())?;
    println!("...done");
    thread::sleep(Duration::from_millis(1500));
    println!("checking results...");
    let result1 = rx.recv().map_err(to_rv)?;
    let result2 = rx.recv().map_err(to_rv)?;
    assert_eq!(result1, Ok("some_path.png".to_string()));
    assert_eq!(result2, Ok("some_other_path.png".to_string()));
    println!("...done");
    println!("terminate...");
    let terminate_stream = b"GET /TERMINATE HTTP/1.1\r\n";
    send_request(terminate_stream.as_slice())?;
    thread::sleep(Duration::from_millis(500));
    assert!(handle.is_finished());
    println!("...done");
    Ok(())
}
#[test]
fn test_increase_port() -> RvResult<()> {
    assert_eq!(increase_port("address:1234")?, "address:1235");
    Ok(())
}
