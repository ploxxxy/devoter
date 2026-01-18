use std::{
    cell::RefCell,
    net::{SocketAddr, ToSocketAddrs},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use rand::{Rng, SeedableRng, rngs::SmallRng};
use rsa::{Pkcs1v15Encrypt, RsaPublicKey, pkcs8::DecodePublicKey};
use tokio::{io::AsyncWriteExt, net::TcpSocket, sync::Semaphore};

use crate::{
    Stats,
    config::Config,
    crypto::{CompatRng, format_pem},
};

static USERNAME_IDX: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static RNG: RefCell<SmallRng> = RefCell::new(SmallRng::from_os_rng());
    static PAYLOAD_BUFFER: RefCell<String> = RefCell::new(String::with_capacity(256));
}

#[derive(thiserror::Error, Debug)]
pub enum VoteError {
    #[error("Encryption error: {0}")]
    Encryption(#[from] rsa::errors::Error),
    #[error("Network error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Public key error: {0}")]
    KeyParse(String),
    #[error("Config error: {0}")]
    Config(#[from] serde_json::Error),
    // #[error("Timeout")]
    // Timeout,
    // #[error("Network error: {0}")]
    // Network(std::io::Error),
}

pub struct VoteContext {
    public_key: RsaPublicKey,
    pub addr: SocketAddr,
    pub site: String,
    pub usernames: Vec<String>,
}

impl VoteContext {
    pub fn new(config: Config, usernames: Vec<String>) -> Result<Self, VoteError> {
        let pem_string = format_pem(&config.votifier_key);
        let public_key = RsaPublicKey::from_public_key_pem(&pem_string)
            .map_err(|e| VoteError::KeyParse(e.to_string()))?;

        let mut addrs = format!("{}:{}", config.votifier_host, config.votifier_port)
            .to_socket_addrs()
            .expect("Unable to parse socket address");

        let addr = addrs.next().expect("Unable to resolve socket address");

        Ok(Self {
            public_key,
            addr,
            site: config.site_name,
            usernames,
        })
    }
}

pub fn spawn_vote_task(ctx: Arc<VoteContext>, stats: Arc<Stats>, sem: Arc<Semaphore>) {
    tokio::spawn(async move {
        if let Ok(_permit) = sem.acquire().await {
            process_vote(&ctx, &stats).await;
        }
    });
}

pub async fn process_vote(ctx: &VoteContext, stats: &Stats) {
    let idx = USERNAME_IDX.fetch_add(1, Ordering::Relaxed);
    let username = unsafe { ctx.usernames.get_unchecked(idx % ctx.usernames.len()) };

    let result = PAYLOAD_BUFFER.with(|buf| {
        let mut buf = buf.borrow_mut();
        make_payload(&ctx.site, username, &mut buf);
        RNG.with(|rng| {
            let mut rng = rng.borrow_mut();
            let mut compat = CompatRng(&mut *rng);
            ctx.public_key
                .encrypt(&mut compat, Pkcs1v15Encrypt, buf.as_bytes())
        })
    });

    match result {
        Ok(encrypted) => {
            if (execute_vote_transaction(ctx, &encrypted).await).is_err() {
                stats.errors.fetch_add(1, Ordering::Relaxed);
            } else {
                stats.votes.fetch_add(1, Ordering::Relaxed);
            }
        }
        Err(_) => {
            stats.errors.fetch_add(1, Ordering::Relaxed);
        }
    };
}

pub async fn execute_vote_transaction(
    ctx: &VoteContext,
    encrypted_bytes: &[u8],
) -> Result<(), VoteError> {
    let socket = if ctx.addr.is_ipv4() {
        TcpSocket::new_v4()?
    } else {
        TcpSocket::new_v6()?
    };

    // create socket manually to set Linger BEFORE connect to prevent port exhaustion
    #[allow(deprecated)]
    socket.set_linger(Some(Duration::from_secs(0)))?;
    socket.set_nodelay(true)?;

    let mut stream = socket.connect(ctx.addr).await?;

    stream.write_all(encrypted_bytes).await?;

    // wait until the server acknowledges the transaction
    let mut buffer = [0u8; 1];
    stream.peek(&mut buffer).await?;

    Ok(())
}

pub fn make_payload(site: &str, username: &str, buf: &mut String) {
    buf.clear();

    let suffix: u32 = RNG.with(|rng| rng.borrow_mut().random());

    // pre-calculate safe estimate to avoid re-allocation during write!
    let size = 32 + site.len() + username.len();
    let _ = buf.try_reserve(size);

    use std::fmt::Write;
    let _ = write!(
        buf,
        "VOTE\n{}-{:x}\n{}\n127.0.0.1\n1234567890\n",
        site, suffix, username
    );
}
