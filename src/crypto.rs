use rand::TryRngCore;

// minimal compatibility shim for RSA crate which expects specific Rng traits
pub struct CompatRng<R>(pub R);

impl<R: rand::RngCore> rsa::rand_core::RngCore for CompatRng<R> {
    fn next_u32(&mut self) -> u32 {
        self.0.next_u32()
    }
    fn next_u64(&mut self) -> u64 {
        self.0.next_u64()
    }
    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.0.fill_bytes(dest)
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rsa::rand_core::Error> {
        self.0
            .try_fill_bytes(dest)
            .map_err(rsa::rand_core::Error::new)
    }
}
impl<R> rsa::rand_core::CryptoRng for CompatRng<R> {}

pub fn format_pem(input: &str) -> String {
    let mut result = String::with_capacity(input.len() + 64);

    result.push_str("-----BEGIN PUBLIC KEY-----\n");
    for chunk in input.as_bytes().chunks(64) {
        result.push_str(std::str::from_utf8(chunk).unwrap());
        result.push('\n');
    }

    result.push_str("-----END PUBLIC KEY-----\n");
    result
}
