//! Age-encryption with user passphrase

use napi::{
    bindgen_prelude::{Buffer, Either},
    Env, JsBuffer, JsBufferValue, Ref, Result, Task,
};
use napi_derive::napi;

/// Shared encrypt input and decrypt input
pub enum EncryptInput {
    String(String),
    Buffer(Ref<JsBufferValue>),
    Bytes(Vec<u8>),
}

impl EncryptInput {
    #[inline]
    pub fn from_either(input: Either<String, JsBuffer>) -> Result<Self> {
        match input {
            Either::A(s) => Ok(Self::String(s)),
            Either::B(b) => Ok(Self::Buffer(b.into_ref()?)),
        }
    }
}

impl AsRef<[u8]> for EncryptInput {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::String(s) => s.as_bytes(),
            Self::Buffer(b) => b.as_ref(),
            Self::Bytes(b) => b.as_slice(),
        }
    }
}

pub struct EncryptTask {
    buf: EncryptInput,
    passphrase: String,
}

impl EncryptTask {
    #[inline]
    pub fn new(passphrase: String, buf: EncryptInput) -> EncryptTask {
        EncryptTask { buf, passphrase }
    }

    #[inline]
    pub fn encrypt(passphrase: &str, buf: &[u8]) -> Result<Vec<u8>> {
        Ok(lsq_encryption::encrypt_with_user_passphrase(passphrase, buf, true)?.to_vec())
    }
}

#[napi]
impl Task for EncryptTask {
    // output of compute fn
    type Output = Vec<u8>;
    // output of resolve fn
    type JsValue = Buffer;

    fn compute(&mut self) -> Result<Self::Output> {
        match &self.buf {
            EncryptInput::String(s) => Self::encrypt(&self.passphrase, s.as_bytes()),
            EncryptInput::Buffer(buf) => Self::encrypt(&self.passphrase, buf.as_ref()),
            EncryptInput::Bytes(b) => Self::encrypt(&self.passphrase, b),
        }
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }

    fn finally(&mut self, env: Env) -> Result<()> {
        if let EncryptInput::Buffer(buf) = &mut self.buf {
            buf.unref(env)?;
        }
        Ok(())
    }
}

// for decryption

pub struct DecryptTask {
    buf: EncryptInput,
    passphrase: String,
}

impl DecryptTask {
    #[inline]
    pub fn new(passphrase: String, buf: EncryptInput) -> DecryptTask {
        DecryptTask { buf, passphrase }
    }

    #[inline]
    pub fn decrypt(passphrase: &str, buf: &[u8]) -> Result<Vec<u8>> {
        Ok(lsq_encryption::decrypt_with_user_passphrase(passphrase, buf)?.to_vec())
    }
}

#[napi]
impl Task for DecryptTask {
    // output of compute fn
    type Output = Vec<u8>;
    // output of resolve fn
    type JsValue = Buffer;

    fn compute(&mut self) -> Result<Self::Output> {
        match &self.buf {
            EncryptInput::String(s) => Self::decrypt(&self.passphrase, s.as_bytes()),
            EncryptInput::Buffer(buf) => Self::decrypt(&self.passphrase, buf.as_ref()),
            EncryptInput::Bytes(b) => Self::decrypt(&self.passphrase, b),
        }
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output.into())
    }

    fn finally(&mut self, env: Env) -> Result<()> {
        if let EncryptInput::Buffer(buf) = &mut self.buf {
            buf.unref(env)?;
        }
        Ok(())
    }
}
