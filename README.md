# rs-crypto-com-exchange
This is an unofficial websocket library for the crypto com exchange websocket api https://exchange-docs.crypto.com/exchange/v1/rest-ws/index.html


```rust
use crypto_com_exchange::{CryptoClient, CryptoError, SubscribeResult};
use log::debug;
use serde_json::json;
use tokio;

#[tokio::main]
async fn main() -> Result<(), CryptoError> {
    env_logger::init();

    // callback function
    let callback = |result: Result<SubscribeResult, CryptoError>, _context: ()| async move {
        match result {
            Ok(subscribe_result) => {
                debug!("Subscription successful: {:?}", subscribe_result);
            }
            Err(e) => {
                debug!("Error during subscription: {:?}", e);
            }
        }
    };

    // create new client with callback
    let mut client = CryptoClient::new(callback, ());
    
    // connect to market stream
    client.connect_market().await?;

    let sub = json!({
        "channels": "trade.ETH_USDT"
    });
    debug!("Subscribing to {:?}", sub);

    // Subscribovanie na trh pomocou zoznamu kanálov
    client.subscribe(sub).await?;
    
    // never ending app
    tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");

    Ok(())
}
```

<!-- This is an example `UserClient`. It is currently being developed but at least, you can do the authentication and get the balance -->
