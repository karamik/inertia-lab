use ed25519_dalek::{Keypair, PublicKey, SecretKey};
use rand::rngs::OsRng;

pub struct KeyPair(Keypair);

impl KeyPair {
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        Self(Keypair::generate(&mut csprng))
    }
    
    pub fn public(&self) -> PublicKey {
        self.0.public
    }
    
    pub fn secret(&self) -> SecretKey {
        self.0.secret.clone()
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        Keypair::from_bytes(bytes).map(Self).ok()
    }
    
    pub fn to_bytes(&self) -> [u8; 64] {
        self.0.to_bytes()
    }
}
