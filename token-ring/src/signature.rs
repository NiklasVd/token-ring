use std::io::Cursor;
use ed25519_dalek::{PublicKey, Signature as S, Keypair, Signer, Verifier, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, ed25519::signature::Signature};
use crate::{serialize::{Serializable, read_byte_arr, write_byte_arr, write_byte_vec, read_byte_vec}, err::TResult};

#[derive(Debug, Clone, PartialEq)]
pub struct Signed<T: Serializable> {
    /* Alternative layout: keypair, val stored on initialization,
    while val_bytes and signature are kept in Option types. Then,
    signature may be generated on the fly and not when Signed instance is created.

    Unclear but potentially massive drawback: keypair is kept unneccessarily long in
    storage. Current layout merely keeps public key. */

    key: PublicKey,
    signature: S,
    pub val: T,
    val_bytes: Vec<u8>
}

impl<T: Serializable> Signed<T> {
    pub fn new(keypair: &Keypair, val: T) -> TResult<Self> {
        // Upon init the value is serialized immediately, in order to
        // generate signature (and to drop private key from memory).
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
        // Serialization steps differ here:
        // Inner value is already serialized and merely its bytes are written
        // into stream.
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

pub fn generate_keypair() -> Keypair {
    let mut rng = rand::rngs::OsRng;
    Keypair::generate(&mut rng)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use crate::{serialize::{Serializable, write_string, read_string}, err::TResult};
    use super::{generate_keypair, Signed};

    #[derive(Debug, Clone, PartialEq)]
    struct Stub(String);

    impl Serializable for Stub {
        type Output = Stub;

        fn write(&self, buf: &mut Vec<u8>) -> TResult {
            write_string(buf, &self.0)
        }

        fn read(buf: &mut Cursor<&[u8]>) -> TResult<Self::Output> {
            let string = read_string(buf)?;
            Ok(Stub(string))
        }

        fn size(&self) -> usize {
            self.0.len()
        }
    }

    fn create_stub() -> Signed<Stub> {
        let keypair = generate_keypair();
        let val = Stub("Test".to_owned());
        Signed::new(&keypair, val).unwrap()
    }

    #[test]
    fn serialize_stub() {
        let stub = Stub("Test".to_owned());
        let mut buf = vec![];
        assert!(stub.write(&mut buf).is_ok());

        let mut cursor = Cursor::new(buf.as_slice());
        let new_stub = Stub::read(&mut cursor).unwrap();
        assert_eq!(stub, new_stub)
    }

    #[test]
    fn sign() {
        let signed_stub = create_stub();
        assert!(signed_stub.verify());
    }

    #[test]
    fn verify() {
        let signed_stub = create_stub();
        let mut buf = vec![];
        signed_stub.write(&mut buf).unwrap();
        
        let mut cursor = Cursor::new(buf.as_slice());
        let deserialized_stub = Signed::<Stub>::read(&mut cursor).unwrap();
        assert!(deserialized_stub.verify());
    }
}
