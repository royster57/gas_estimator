use std::fmt;

use serde::{Deserialize, Serialize};

use thiserror::Error;

use reqwest::Client;

use lazy_static::lazy_static;

use alloy_primitives::Address;
use alloy_primitives::U256;

const INFURA_KEY: &str = "cfdfffb93d6a470e97b67bf871f8a347";

lazy_static! {
    static ref RPC_URL: String = format!("https://mainnet.infura.io/v3/{INFURA_KEY}");
}

#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'a str,
    method: &'a str,
    params: Vec<serde_json::Value>,
    id: u64,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize, Debug, Error)]
struct JsonRpcError {
    code: i64,
    message: String,
}

// Implement fmt::Display
impl fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON-RPC Error {}: {}", self.code, self.message)
    }
}

async fn send_rpc<T: for<'de> Deserialize<'de>>(
    rpc_url: &str,
    method: &str,
    params: Vec<serde_json::Value>,
) -> Result<T, Box<dyn std::error::Error>> {
    let client = Client::new();
    let request = JsonRpcRequest {
        jsonrpc: "2.0",
        method,
        params,
        id: 1,
    };

    let response = client
        .post(rpc_url)
        .json(&request)
        .send()
        .await?
        .json::<JsonRpcResponse<T>>()
        .await?;

    if let Some(result) = response.result {
        Ok(result)
    } else if let Some(error) = response.error {
        Err(format!("RPC Error: {} - {}", error.code, error.message).into())
    } else {
        Err("Unknown RPC Error".into())
    }
}

// We call it "Embellished" because the "from" field has been recovered from v,r,s.
#[derive(Debug, Serialize)]
pub struct Transaction {
    pub nonce: U256,
    pub gas_price: Option<U256>,
    pub gas_limit: U256,
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub data: Vec<u8>,
    pub v: u64, // Recovery ID
    pub r: U256,
    pub s: U256,
}

pub async fn estimate_gas(
    tx: &Transaction,
    rpc_url: &str,
) -> Result<U256, Box<dyn std::error::Error>> {
    // Construct JSON-RPC parameters
    let params = serde_json::json!(
        {
            "from": tx.from.to_string(),
            "to": tx.to.to_string(),
            "value": tx.value,
            "data": format!("0x{}", hex::encode(tx.data.clone())),
        }
    );

    let gas_estimate: String = send_rpc(&rpc_url, "eth_estimateGas", vec![params]).await?;

    Ok(U256::from_str_radix(
        gas_estimate.trim_start_matches("0x"),
        16,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[tokio::test]
    async fn gas_estimation_eth_transfer() {
        let from = Address::from_str("0x84d82ac02AdE3a3d8a636Fb06E442ab701aA7BB4").unwrap();
        let to = Address::from_str("0x1111111111111111111111111111111111111111").unwrap();

        let tx = Transaction {
            nonce: U256::from(123),
            gas_price: Some(U256::from(1000)),
            gas_limit: U256::from(1_000_000_000),
            from,
            to,
            value: "0x0000000000000000000000000000000000000001"
                .parse()
                .unwrap(),
            data: Default::default(),
            v: 777,
            r: U256::from(987654321),
            s: U256::from(121212),
        };

        let expected = U256::from(21_000);
        assert_eq!(
            estimate_gas(&tx, &RPC_URL).await.expect("should succeed"),
            expected
        );
    }
}
