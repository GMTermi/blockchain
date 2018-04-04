extern crate serde_json;

use std::collections::HashMap;

use exonum::api::{Api, ApiError as ExonumApiError};
use exonum::blockchain::Blockchain;
use exonum::crypto::PublicKey;
use exonum::encoding::serialize::FromHex;
use iron::headers::AccessControlAllowOrigin;
use iron::prelude::*;
use iron::status;
use router::Router;
use hyper::header::ContentType;

use currency::api::ServiceApi;
use currency::api::error::ApiError;
use currency::assets;
use currency::assets::{AssetBundle, AssetInfo, AssetId};
use currency::wallet;
use currency::wallet::Wallet;

pub const PARAMETER_META_DATA_KEY: &str = "meta_data";

#[derive(Clone)]
pub struct WalletApi {
    pub blockchain: Blockchain,
}
#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct WalletInfo {
    pub balance: u64,
    pub count_assets: u64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct ExtendedAsset {
    pub id: AssetId,
    pub amount: u64,
    pub meta_data: Option<AssetInfo>,
}

impl ExtendedAsset {
    pub fn from_asset(asset: &AssetBundle, info: Option<AssetInfo>) -> Self {
        ExtendedAsset { id: asset.id(), amount: asset.amount(), meta_data: info }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct WalletsResponseBody {
    pub total: u64,
    pub count: u64,
    pub wallets: HashMap<PublicKey, WalletInfo>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct WalletAssetsResponseBody {
    pub total: u64,
    pub count: u64,
    pub assets: Vec<ExtendedAsset>,
}

pub type WalletResponse = Result<Wallet, ApiError>;

pub type WalletsResponse = Result<WalletsResponseBody, ApiError>;

pub type WalletAssetsResponse = Result<WalletAssetsResponseBody, ApiError>;

impl WalletApi {
    fn wallet(&self, pub_key: &PublicKey) -> Wallet {
        let view = &mut self.blockchain.fork();
        wallet::Schema(view).fetch(pub_key)
    }

    fn wallets(&self) -> HashMap<PublicKey, WalletInfo> {
        let view = &mut self.blockchain.fork();
        let index = wallet::Schema(view).index();
        let mut result: HashMap<PublicKey, WalletInfo> = HashMap::new();
        for v in index.iter() {

            let wi = WalletInfo {
                balance: v.1.balance(),
                count_assets: v.1.assets().len() as u64,
            };
            result.insert(v.0, wi);
        }

        result
    }

    fn pagination_wallets(&self, offset: u64, limit: u64) -> (HashMap<PublicKey, WalletInfo>, u64, u64){
        let view = &mut self.blockchain.fork();
        let idx = wallet::Schema(view).index();
        let mut total:u64 = 0;
        let mut count:u64 = 0;
        let mut result: HashMap<PublicKey, WalletInfo> = HashMap::new();
        for v in idx.iter() {
            if total < offset || total >= offset + limit {
                total+=1;
                continue;
            }
            let wi = WalletInfo {
                balance: v.1.balance(),
                count_assets: v.1.assets().len() as u64,
            };
            result.insert(v.0, wi);
            count+=1;
            total+=1;
        }

        (result, total, count)
    }

    fn wallets_balance(&self) -> Vec<PublicKey> {
        let view = &mut self.blockchain.fork();
        let index = wallet::Schema(view).index();
        let wallets = index.into_iter().map(|v|{ v.0 }).collect();
        wallets
    }

    fn assets(&self, pub_key: &PublicKey) -> Vec<AssetBundle> {
        self.wallet(pub_key).assets()
    }

    fn asset_info(&self, asset_id: &AssetId) -> Option<AssetInfo> {
        let view = self.blockchain.fork();
        assets::Schema(view).fetch(asset_id)
    }
}

impl Api for WalletApi {
    
    fn wire(&self, router: &mut Router) {
        // Gets status of the wallet corresponding to the public key.
        let self_ = self.clone();
        let wallet_info = move |req: &mut Request| -> IronResult<Response> {
            let path = req.url.path();
            let wallet_key = path.last().unwrap();
            let result: WalletResponse = match PublicKey::from_hex(wallet_key) {
                Ok(public_key) => Ok(self_.wallet(&public_key)),
                Err(_) => Err(ApiError::WalletHexInvalid)
            };

            let mut res = Response::with((
                result.clone().err().map(|e| e.to_status()).unwrap_or(status::Ok),
                serde_json::to_string_pretty(&result).unwrap(),
            ));
            res.headers.set(ContentType::json());
            res.headers.set(AccessControlAllowOrigin::Any);
            Ok(res)
        };

        // Gets status of all wallets.
        let self_ = self.clone();
        let wallets_info = move |req: &mut Request| -> IronResult<Response> {
            let (offset, limit) = ServiceApi::pagination_params(req);
            let (wallets, total, count) = self_.pagination_wallets(offset, limit);

            let result: WalletsResponse = Ok(WalletsResponseBody {total, count, wallets} );

            let mut res = Response::with((
                status::Ok,
                serde_json::to_string_pretty(&result).unwrap(),
            ));
            res.headers.set(ContentType::json());
            res.headers.set(AccessControlAllowOrigin::Any);
            Ok(res)
        };

        let self_ = self.clone();
        let wallet_assets_info = move |req: &mut Request| -> IronResult<Response> {
            let public_key = {
                let wallet_key = req.extensions
                    .get::<Router>()
                    .unwrap()
                    .find("pub_key")
                    .unwrap();
                PublicKey::from_hex(wallet_key).map_err(ExonumApiError::FromHex)?
            };
            let extend_assets = ServiceApi::read_parameter(req, PARAMETER_META_DATA_KEY, false);
            let assets = self_.assets(&public_key);
            // apply pagination parameters if they exist
            let assets_to_send = ServiceApi::apply_pagination(req, &assets);
            let assets_list = if extend_assets {
                let mut extended_assets = Vec::<ExtendedAsset>::new();
                for asset in assets_to_send {
                    let info = self_.asset_info(&asset.id());
                    extended_assets.push(ExtendedAsset::from_asset(asset, info));
                }
                extended_assets
            } else {
                assets_to_send
                    .into_iter()
                    .map(|a| ExtendedAsset::from_asset(a, None))
                    .collect()
            };

            let result: WalletAssetsResponse = Ok(WalletAssetsResponseBody {
                total: assets.len() as u64,
                count: assets_to_send.len() as u64,
                assets: assets_list 
            });

            let mut res = Response::with((
                status::Ok,
                serde_json::to_string_pretty(&result).unwrap(),
            ));
            res.headers.set(ContentType::json());
            res.headers.set(AccessControlAllowOrigin::Any);
            Ok(res)
        };

        router.get("/v1/wallets", wallets_info, "wallets_info");
        router.get("/v1/wallets/:pub_key", wallet_info, "get_balance");
        router.get(
            "/v1/wallets/:pub_key/assets",
            wallet_assets_info,
            "assets_info",
        );
    }
}
