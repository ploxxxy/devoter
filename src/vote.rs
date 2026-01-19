use fastrand::Rng;
use openssl::{
    pkey::Public,
    rsa::{Padding, Rsa},
};
use std::{
    cell::RefCell,
    net::{SocketAddr, ToSocketAddrs},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{io::AsyncWriteExt, net::TcpSocket, sync::OwnedSemaphorePermit};

use crate::{Stats, config::Config};

static USERNAME_IDX: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static RNG: RefCell<Rng> = RefCell::new(Rng::new());
}

#[derive(thiserror::Error, Debug)]
pub enum VoteError {
    #[error("Encryption error: {0}")]
    Encryption(#[from] openssl::error::ErrorStack),
    #[error("Network error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Config error: {0}")]
    Config(#[from] serde_json::Error),
}

pub struct VoteContext {
    public_key: Rsa<Public>,
    pub addr: SocketAddr,
    pub site: String,
    pub usernames: Vec<String>,
}

impl VoteContext {
    pub fn new(config: Config, usernames: Vec<String>) -> Result<Self, VoteError> {
        let pem_string = format_pem(&config.votifier_key);
        let public_key = Rsa::public_key_from_pem(pem_string.as_bytes())?;

        let mut addrs = format!("{}:{}", config.votifier_host, config.votifier_port)
            .to_socket_addrs()
            .expect("Unable to parse socket address");

        if addrs.len() > 1 {
            println!("Found more than 1 socket address, this might be important!");
        }

        let addr = addrs.next().expect("Unable to resolve socket address");

        Ok(Self {
            public_key,
            addr,
            site: config.site_name,
            usernames,
        })
    }
}

pub fn spawn_vote_task(permit: OwnedSemaphorePermit, ctx: Arc<VoteContext>, stats: Arc<Stats>) {
    tokio::spawn(async move {
        process_vote(&ctx, &stats).await;
        drop(permit);
    });
}

pub async fn process_vote(ctx: &VoteContext, stats: &Stats) {
    let idx = USERNAME_IDX.fetch_add(1, Ordering::Relaxed);
    let username = unsafe {
        ctx.usernames
            .get_unchecked(idx.wrapping_rem(ctx.usernames.len()))
    };

    let mut payload_buf = [0u8; 256];
    let payload_len: usize;

    {
        let mut cursor = std::io::Cursor::new(&mut payload_buf[..]);
        let suffix: u32 = RNG.with(|rng| rng.borrow_mut().u32(..));

        use std::io::Write;

        let _ = write!(
            cursor,
            "VOTE\n{}-{:x}\n{}\n127.0.0.1\n1234567890\n",
            ctx.site, suffix, username
        );

        payload_len = cursor.position() as usize;
    }

    let mut encrypted_buf = [0u8; 256];

    let result = ctx.public_key.public_encrypt(
        &payload_buf[..payload_len],
        &mut encrypted_buf,
        Padding::PKCS1,
    );

    match result {
        Ok(size) => {
            let encrypted_bytes = &encrypted_buf[..size];

            if (execute_vote_transaction(ctx, encrypted_bytes).await).is_err() {
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
    socket.set_keepalive(false)?;
    socket.set_reuseaddr(true)?;

    let mut stream = socket.connect(ctx.addr).await?;

    stream.write_all(encrypted_bytes).await?;

    // wait until the server acknowledges the transaction
    let mut buffer = [0u8; 1];
    stream.peek(&mut buffer).await?;

    Ok(())
}

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
