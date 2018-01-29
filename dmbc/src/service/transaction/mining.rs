extern crate exonum;

use exonum::blockchain::Transaction;
use exonum::crypto::PublicKey;
use exonum::messages::Message;
use exonum::storage::Fork;
use serde_json::Value;

use service::transaction::TRANSACTION_FEE;

use super::SERVICE_ID;
use super::schema::transaction_status::{TxStatus, TxStatusSchema};
use super::schema::wallet::WalletSchema;

pub const TX_MINING_ID: u16 = 700;
const AMOUNT_MINING_COIN: u64 = 100_000_000_000_000;

message! {
    struct TxMining {
        const TYPE = SERVICE_ID;
        const ID = TX_MINING_ID;
        const SIZE = 40;

        field pub_key:     &PublicKey  [00 => 32]
        field seed:        u64         [32 => 40]
    }
}

impl TxMining {
    pub fn get_fee(&self) -> u64 {
        TRANSACTION_FEE
    }
}

impl Transaction for TxMining {
    fn verify(&self) -> bool {
        if cfg!(fuzzing) {
            return false;
        }

        self.verify_signature(self.pub_key())
    }

    fn execute(&self, view: &mut Fork) {
        WalletSchema::map(view, |mut schema| {
            let mut wallet = schema.wallet(self.pub_key());
            wallet.increase(AMOUNT_MINING_COIN);
            schema.wallets().put(self.pub_key(), wallet);
        });
        TxStatusSchema::map(view, |mut schema| {
            schema.set_status(&self.hash(), TxStatus::Success)
        });
    }

    fn info(&self) -> Value {
        json!({
            "transaction_data": self,
            "tx_fee": 0,
        })
    }
}
#[cfg(test)]
mod tests {
    use exonum::storage::{Database, MemoryDB};
    use exonum::blockchain::Transaction;

    use service::schema::wallet::WalletSchema;
    use service::wallet::Wallet;
    use service::transaction::mining::AMOUNT_MINING_COIN;
    use super::TxMining;

    fn get_json() -> String {
        r#"{
            "body": {
                "pub_key": "e61b4b9945defd1878d7575ddc50993f6a074cdfcafc47d15cba46860cab0060",
                "seed": "43"
            },
            "network_id": 0,
            "protocol_version": 0,
            "service_id": 2,
            "message_id": 700,
            "signature": "671540cb1bf737c109e7ba7f90364cafa4064f8e7d54cdc74ae31711061efc2f3116be128a09d642970980f87beb19f948f5148f0cd544ba926c2acd304b6d09"
        }"#.to_string()
    }

    #[test]
    fn mining_test() {
        let tx: TxMining = ::serde_json::from_str(&get_json()).unwrap();

        let db = Box::new(MemoryDB::new());
        let fork = &mut db.fork();
        let wallet = Wallet::new(tx.pub_key(), 100, vec![]);

        WalletSchema::map(fork, |mut schema| {
            schema.wallets().put(tx.pub_key(), wallet);
        });

        tx.execute(fork);

        let wallet = WalletSchema::map(fork, |mut schema| schema.wallet(tx.pub_key()));
        assert_eq!(AMOUNT_MINING_COIN + 100, wallet.balance());
    }

    #[test]
    fn mining_coin_info_test() {
        let tx: TxMining = ::serde_json::from_str(&get_json()).unwrap();
        assert_eq!(0, tx.info()["tx_fee"]);
    }
}
