#![deny(warnings)]
mod cleanup;
mod execute_code;
mod parse_body;
mod tokiort;

use std::convert::Infallible;
use std::io::{self, Write};
use std::net::SocketAddr;

use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use tokio::net::TcpListener;

use crate::parse_body::parse_body_service;
use crate::tokiort::{TokioIo, TokioTimer};

// An async function that consumes a request, does nothing with it and returns a
// response.
async fn hello(req: Request<impl hyper::body::Body>) -> Result<Response<Full<Bytes>>, Infallible> {
    println!("Received a request with URI: {}", req.uri()); // Log the request URI

    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => {
            match execute_code::execute_code("public class Solution{public static void main(String args[]){System.out.println(\"Hello World\");}}").await {
                Ok(output) => Ok(Response::new(Full::new(Bytes::from(output)))),
                Err(err) => Ok(Response::new(Full::new(Bytes::from(format!(
                    "Error: {:?}",
                    err
                ))))),
            }
        }
        (&Method::GET, "/cleanup") => {
            cleanup::remove_containers().await;
            Ok(Response::new(Full::new(Bytes::from("👍"))))
        }
        (&Method::GET, "/logs") => {
            let container_id = req.uri().query().unwrap_or("");
            let container_logs = format!("docker logs {}", container_id);
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(container_logs)
                .output()
                .expect("Failed to execute command");
            io::stdout().write_all(&output.stdout).unwrap();
            let output = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(Response::new(Full::new(Bytes::from(output))))
        }
        _ => Ok(Response::builder() // Handle requests to other paths
            .status(404)
            .body(Full::new(Bytes::from("Not Found")))
            .unwrap()),
    }
}

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    pretty_env_logger::init();

    // This address is localhost
    let addr: SocketAddr = ([127, 0, 0, 1], 3000).into();

    // Bind to the port and listen for incoming TCP connections
    let listener = TcpListener::bind(addr).await?;
    println!("Listening on http://{}", addr);
    loop {
        let (tcp, _) = listener.accept().await?;
        let io = TokioIo::new(tcp);
        tokio::task::spawn(async move {
            // Handle the connection from the client using HTTP1 and pass any
            // HTTP requests received on that connection to the `hello` function
            if let Err(err) = http1::Builder::new()
                .timer(TokioTimer)
                .serve_connection(io, service_fn(parse_body_service))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
        // When an incoming TCP connection is received grab a TCP stream for
        // client<->server communication.
        //
        // Note, this is a .await point, this loop will loop forever but is not a busy loop. The
        // .await point allows the Tokio runtime to pull the task off of the thread until the task
        // has work to do. In this case, a connection arrives on the port we are listening on and
        // the task is woken up, at which point the task is then put back on a thread, and is
        // driven forward by the runtime, eventually yielding a TCP stream.
        let (tcp, _) = listener.accept().await?;
        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(tcp);

        // Spin up a new task in Tokio so we can continue to listen for new TCP connection on the
        // current task without waiting for the processing of the HTTP1 connection we just received
        // to finish
        tokio::task::spawn(async move {
            // Handle the connection from the client using HTTP1 and pass any
            // HTTP requests received on that connection to the `hello` function
            if let Err(err) = http1::Builder::new()
                .timer(TokioTimer)
                .serve_connection(io, service_fn(hello))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}
