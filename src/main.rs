#![warn(rust_2018_idioms)]

use async_std::io;
use async_std::net::{Shutdown, TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task;

// use futures::future::try_join;
// use futures::{AsyncWriteExt, FutureExt};
use std::env;
use std::error::Error;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listen_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8081".to_string());
    let server_addr = env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string());

    println!("Listening on: {}", listen_addr);
    println!("Proxying to: {}", server_addr);

    let listener = TcpListener::bind(listen_addr).await?;

    while let Ok((inbound, _)) = listener.accept().await {
        let server_addr = server_addr.clone();
        let transfer = async move {
            let res = transfer(inbound, server_addr).await;
            if let Err(e) = res {
                println!("Failed to transfer; error={}", e);
            }
        };

        task::spawn(transfer);
    }

    Ok(())
}

async fn transfer(inbound: TcpStream, proxy_addr: String) -> Result<(), Box<dyn Error>> {
    let stop_source = stop_token::StopSource::new();
    let stop_token = stop_source.stop_token();

    let outbound = TcpStream::connect(proxy_addr);
    let outbound = stop_token.stop_future(outbound);

    let outbound = match outbound.await {
        Some(Ok(stream)) => Ok(stream),
        Some(Err(err)) => {
            println!(
                "Failed creating port forwarder to remote connection: {:?}",
                err
            );
            Err(err)
        }
        None => {
            println!("Cancelled while connecting, returning");
            return Ok(());
        }
    }?;

    let (ri, wi) = &mut (&inbound, &inbound);
    let (ro, wo) = &mut (&outbound, &outbound);

    let client_to_server = async {
        let res = io::copy(ri, wo).await;
        println!("Completed client->server copy with {:?}", res);
        res
    };

    let server_to_client = async {
        let res = io::copy(ro, wi).await;
        println!("Completed server->client copy with {:?}", res);
        res
    };

    let copiers = client_to_server.try_join(server_to_client);
    let copiers = stop_token.stop_future(copiers);

    match copiers.await {
        Some(Ok(_)) => println!("Forwarding task completed"),
        Some(Err(e)) => println!("Error during forwarding task: {:?}", e),
        None => println!("Forwarding task cancelled"),
    }

    /*
    let (ri, wi) = (inbound.clone(), inbound);
    let (ro, wo) = (outbound.clone(), outbound);

    let client_to_server = async {
        io::copy(&ri, &wo).await?;
        wo.shutdown(Shutdown::Write)
    };

    let server_to_client = async {
        io::copy(&ro, &wi).await?;
        wi.shutdown(Shutdown::Write)
    };

    try_join(client_to_server, server_to_client).await?;
    */

    Ok(())
}
