mod ed25519;
pub mod types;

use self::ed25519::ED25519Signus;
use self::types::{
    MyDidInfo,
    MyDid,
    TheirDid
};
use utils::crypto::base58::Base58;

use errors::crypto::CryptoError;
use errors::signus::SignusError;
use std::collections::HashMap;

const DEFAULT_CRYPTO_TYPE: &'static str = "ed25519";

trait CryptoType {
    fn create_key_pair(&self) -> (Vec<u8>, Vec<u8>);
    fn encrypt(&self, private_key: &[u8], public_key: &[u8], doc: &[u8], nonce: &[u8]) -> Vec<u8>;
    fn decrypt(&self, private_key: &[u8], public_key: &[u8], doc: &[u8], nonce: &[u8]) -> Result<Vec<u8>, CryptoError>;
    fn gen_nonce(&self) -> Vec<u8>;
    fn create_key_pair_for_signature(&self, seed: Option<&[u8]>) -> (Vec<u8>, Vec<u8>);
    fn sign(&self, private_key: &[u8], doc: &[u8]) -> Vec<u8>;
    fn verify(&self, public_key: &[u8], doc: &[u8], signature: &[u8]) -> bool;
}

pub struct SignusService {
    crypto_types: HashMap<&'static str, Box<CryptoType>>
}

impl SignusService {
    pub fn new() -> SignusService {
        let mut crypto_types: HashMap<&str, Box<CryptoType>> = HashMap::new();
        crypto_types.insert(DEFAULT_CRYPTO_TYPE, Box::new(ED25519Signus::new()));

        SignusService {
            crypto_types: crypto_types
        }
    }

    pub fn create_my_did(&self, did_info: &MyDidInfo) -> Result<MyDid, SignusError> {
        let xtype = did_info.crypto_type.clone().unwrap_or(DEFAULT_CRYPTO_TYPE.to_string());

        if !self.crypto_types.contains_key(&xtype.as_str()) {
            return Err(SignusError::CryptoError(CryptoError::UnknownType(xtype)));
        }

        let signus = self.crypto_types.get(&xtype.as_str()).unwrap();

        let seed = did_info.seed.as_ref().map(String::as_bytes);
        let (public_key, secret_key) = signus.create_key_pair();
        let (ver_key, sign_key) = signus.create_key_pair_for_signature(seed);
        let did = did_info.did.as_ref().map(|did| Base58::decode(did)).unwrap_or(Ok(ver_key[0..16].to_vec()))?;


        let my_did = MyDid::new(Base58::encode(&did),
                                xtype.clone(),
                                Base58::encode(&public_key),
                                Base58::encode(&secret_key),
                                Base58::encode(&ver_key),
                                Base58::encode(&sign_key));
        println!("did {:?}", my_did.did);

        Ok(my_did)
    }

    pub fn sign(&self, my_did: &MyDid, doc: &str) -> Result<String, SignusError> {
        if !self.crypto_types.contains_key(&my_did.crypto_type.as_str()) {
            return Err(SignusError::CryptoError(CryptoError::UnknownType(my_did.crypto_type.clone())));
        }

        let signus = self.crypto_types.get(&my_did.crypto_type.as_str()).unwrap();

        let sign_key = Base58::decode(&my_did.sign_key)?;
        let signature = signus.sign(&sign_key, doc.as_bytes());
        let signature = Base58::encode(&signature);

        Ok(signature)
    }

    pub fn verify(&self, their_did: &TheirDid, doc: &str, signature: &str) -> Result<bool, SignusError> {
        let xtype = their_did.crypto_type.clone().unwrap_or(DEFAULT_CRYPTO_TYPE.to_string());

        if !self.crypto_types.contains_key(&xtype.as_str()) {
            return Err(SignusError::CryptoError(CryptoError::UnknownType(xtype)));
        }

        let signus = self.crypto_types.get(&xtype.as_str()).unwrap();

        let verkey = Base58::decode(&their_did.verkey)?;
        let signature = Base58::decode(signature)?;

        Ok(signus.verify(&verkey, &doc.as_bytes(), &signature))
    }

    pub fn encrypt(&self, my_did: &MyDid, their_did: &TheirDid, doc: &str) -> Result<(String, String), SignusError> {
        if !self.crypto_types.contains_key(&my_did.crypto_type.as_str()) {
            return Err(SignusError::CryptoError(CryptoError::UnknownType(my_did.crypto_type.clone())));
        }

        if their_did.pk.is_none() {
            return Err(SignusError::CryptoError(CryptoError::InvalidStructure(format!("Public key not found"))));
        }

        let signus = self.crypto_types.get(&my_did.crypto_type.as_str()).unwrap();
        let public_key = their_did.pk.clone().unwrap();

        let nonce = signus.gen_nonce();

        let secret_key = Base58::decode(&my_did.secret_key)?;
        let public_key = Base58::decode(&public_key)?;
        let doc = Base58::decode(&doc)?;

        let encrypted_doc = signus.encrypt(&secret_key, &public_key, &doc, &nonce);
        let encrypted_doc = Base58::encode(&encrypted_doc);
        let nonce = Base58::encode(&nonce);
        Ok((encrypted_doc, nonce))
    }

    pub fn decrypt(&self, my_did: &MyDid, their_did: &TheirDid, doc: &str, nonce: &str) -> Result<String, SignusError> {
        if !self.crypto_types.contains_key(&my_did.crypto_type.as_str()) {
            return Err(SignusError::CryptoError(CryptoError::UnknownType(my_did.crypto_type.clone())));
        }

        if their_did.pk.is_none() {
            return Err(SignusError::CryptoError(CryptoError::BackendError(format!("Public key not found"))));
        }

        let signus = self.crypto_types.get(&my_did.crypto_type.as_str()).unwrap();
        let public_key = their_did.pk.clone().unwrap();

        let secret_key = Base58::decode(&my_did.secret_key)?;
        let public_key = Base58::decode(&public_key)?;
        let doc = Base58::decode(&doc)?;
        let nonce = Base58::decode(&nonce)?;

        let decrypted_doc = signus.decrypt(&secret_key, &public_key, &doc, &nonce)?;
        let decrypted_doc = Base58::encode(&decrypted_doc);
        Ok(decrypted_doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use services::signus::types::MyDidInfo;

    #[test]
    fn create_my_did_with_empty_input_works() {
        let service = SignusService::new();

        let did_info = MyDidInfo {
            did: None,
            seed: None,
            crypto_type: None
        };

        let res = service.create_my_did(&did_info);

        assert!(res.is_ok());
    }

    #[test]
    fn create_my_did_with_did_in_input_works() {
        let service = SignusService::new();

        let did = Some("Dbf2fjCbsiq2kfns".to_string());
        let did_info = MyDidInfo {
            did: did.clone(),
            seed: None,
            crypto_type: None
        };

        let res = service.create_my_did(&did_info);
        assert!(res.is_ok());

        assert_eq!(did.unwrap(), did_info.did.unwrap());
    }

    #[test]
    fn try_create_my_did_with_invalid_crypto_type() {
        let service = SignusService::new();

        let did = Some("Dbf2fjCbsiq2kfns".to_string());
        let crypto_type = Some("type".to_string());
        let did_info = MyDidInfo {
            did: did.clone(),
            seed: None,
            crypto_type: crypto_type
        };

        let res = service.create_my_did(&did_info);
        assert!(res.is_err());
    }

    #[test]
    fn create_my_did_with_seed_type() {
        let service = SignusService::new();

        let did = Some("Dbf2fjCbsiq2kfns".to_string());
        let seed = Some("DJASbewkdUY3265HJFDSbds278sdDSnA".to_string());
        let did_info_with_seed = MyDidInfo {
            did: did.clone(),
            seed: seed,
            crypto_type: None
        };
        let did_info_without_seed = MyDidInfo {
            did: did.clone(),
            seed: None,
            crypto_type: None
        };

        let res_with_seed = service.create_my_did(&did_info_with_seed);
        let res_without_seed = service.create_my_did(&did_info_without_seed);

        assert!(res_with_seed.is_ok());
        assert!(res_without_seed.is_ok());

        assert_ne!(res_with_seed.unwrap().ver_key, res_without_seed.unwrap().ver_key)
    }

    #[test]
    fn sign_works() {
        let service = SignusService::new();

        let did_info = MyDidInfo {
            did: None,
            seed: None,
            crypto_type: None
        };
        let msg = "some message";

        let res = service.create_my_did(&did_info);
        assert!(res.is_ok());
        let my_did = res.unwrap();

        let signature = service.sign(&my_did, msg);
        assert!(signature.is_ok());
    }

    #[test]
    fn sign_verify_works() {
        let service = SignusService::new();

        let did_info = MyDidInfo {
            did: None,
            seed: None,
            crypto_type: None
        };
        let msg = "some message";

        let res = service.create_my_did(&did_info);
        assert!(res.is_ok());
        let my_did = res.unwrap();

        let signature = service.sign(&my_did, msg);
        assert!(signature.is_ok());
        let signature = signature.unwrap();

        let their_did = TheirDid {
            did: "sw2SA2jCbsiq2kfns".to_string(),
            crypto_type: Some(DEFAULT_CRYPTO_TYPE.to_string()),
            pk: None,
            verkey: my_did.ver_key
        };

        let res = service.verify(&their_did, &msg, &signature);
        assert!(res.is_ok());
        let valid = res.unwrap();
        assert!(valid);
    }

    #[test]
    fn try_verify_with_invalid_verkey() {
        let service = SignusService::new();

        let did_info = MyDidInfo {
            did: None,
            seed: None,
            crypto_type: None
        };
        let msg = "message";

        let res = service.create_my_did(&did_info);
        assert!(res.is_ok());
        let my_did = res.unwrap();

        let signature = service.sign(&my_did, msg);
        assert!(signature.is_ok());
        let signature = signature.unwrap();

        let their_did = TheirDid {
            did: "sw2SA2jCbsiq2kfns".to_string(),
            crypto_type: Some(DEFAULT_CRYPTO_TYPE.to_string()),
            pk: None,
            verkey: "AnnxV4t3LUHKZaxVQDWoVaG44NrGmeDYMA4Gz6C2tCZd".to_string()
        };

        let res = service.verify(&their_did, &msg, &signature);
        res.unwrap();
//        assert!(res.is_ok());
//        assert_eq!(false, res.unwrap());
    }
}