use std::io::Cursor;
use ed25519_dalek::{PublicKey, Signature as S, Keypair, Signer, Verifier, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, ed25519::signature::Signature};
use crate::{serialize::{Serializable, read_byte_arr, write_byte_arr, write_byte_vec, read_byte_vec}, err::TResult};

pub struct Signed<T: Serializable> {
    key: PublicKey,
    signature: S,
    val: T,
    val_bytes: Vec<u8>
}

impl<T: Serializable> Signed<T> {
    pub fn new(keypair: Keypair, val: T) -> TResult<Self> {
        let mut val_bytes = vec![];
        val.write(&mut val_bytes)?;

        let signature = keypair.sign(&val_bytes);
        Ok(Self {
            key: keypair.public, signature, val, val_bytes
        })
    }

    pub fn verify(&self) -> bool {
        self.key.verify(&self.val_bytes, &self.signature).is_ok()
    }
}

impl<T: Serializable<Output = T>> Serializable for Signed<T> {
    type Output = Signed<T>;

    fn write(&self, buf: &mut Vec<u8>) -> TResult {
        write_byte_arr(buf, &self.key.to_bytes())?;
        write_byte_arr(buf, &self.signature.to_bytes())?;
        write_byte_vec(buf, &self.val_bytes)
    }

    fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
        let key = PublicKey::from_bytes(&read_byte_arr::<PUBLIC_KEY_LENGTH>(buf)?)?;
        let signature = Signature::from_bytes(&read_byte_arr::<SIGNATURE_LENGTH>(buf)?)?;
        let val_bytes = read_byte_vec(buf)?;
        let val = T::read(&mut Cursor::new(&val_bytes))?;
        
        Ok(Self {
            key, signature, val, val_bytes
        })
    }

    fn size(&self) -> usize {
        PUBLIC_KEY_LENGTH + SIGNATURE_LENGTH + self.val_bytes.len()
    }
}