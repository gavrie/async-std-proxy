#![warn(rust_2018_idioms)]

use async_std::io;
use async_std::task;
use async_std::net::{TcpListener, TcpStream, Shutdown};

use futures::future::try_join;
use futures::{FutureExt, AsyncWriteExt};
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
        let transfer = transfer(inbound, server_addr.clone()).map(|r| {
            if let Err(e) = r {
                println!("Failed to transfer; error={}", e);
            }
        });

        task::spawn(transfer);
    }

    Ok(())
}

async fn transfer(inbound: TcpStream, proxy_addr: String) -> Result<(), Box<dyn Error>> {
    let outbound = TcpStream::connect(proxy_addr).await?;

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

    Ok(())
}
