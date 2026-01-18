use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time;

use crate::config::{load_config, load_usernames};
use crate::vote::{VoteContext, VoteError, spawn_vote_task};

mod config;
mod vote;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub struct Stats {
    votes: AtomicU64,
    errors: AtomicU64,
}

#[tokio::main]
async fn main() -> Result<(), VoteError> {
    let config = load_config()?;
    let usernames = load_usernames()?;
    let target_rate = config.rate;
    let max_connections = config.max_connections;

    let ctx = Arc::new(VoteContext::new(config, usernames)?);
    let stats = Arc::new(Stats {
        votes: AtomicU64::new(0),
        errors: AtomicU64::new(0),
    });

    let banner = r#"
▓█████▄ ▓█████ ██▒   █▓ ▒█████  ▄▄▄█████▓▓█████  ██▀███
▒██▀ ██▌▓█   ▀▓██░   █▒▒██▒  ██▒▓  ██▒ ▓▒▓█   ▀ ▓██ ▒ ██▒
░██   █▌▒███   ▓██  █▒░▒██░  ██▒▒ ▓██░ ▒░▒███   ▓██ ░▄█ ▒
░▓█▄   ▌▒▓█  ▄  ▒██ █░░▒██   ██░░ ▓██▓ ░ ▒▓█  ▄ ▒██▀▀█▄
░▒████▓ ░▒████▒  ▒▀█░  ░ ████▓▒░  ▒██▒ ░ ░▒████▒░██▓ ▒██▒
 ▒▒▓  ▒ ░░ ▒░ ░  ░ ▐░  ░ ▒░▒░▒░   ▒ ░░   ░░ ▒░ ░░ ▒▓ ░▒▓░
 ░ ▒  ▒  ░ ░  ░  ░ ░░    ░ ▒ ▒░     ░     ░ ░  ░  ░▒ ░ ▒░
 ░ ░  ░    ░       ░░  ░ ░ ░ ▒    ░         ░     ░░   ░
   ░       ░  ░     ░      ░ ░              ░  ░   ░
 ░                 ░                                     "#;

    println!("{}\n", banner);

    println!("Target: {}:{}", ctx.addr, ctx.site);

    let target_mode = match target_rate {
        0 => "Unlimited".to_string(),
        _ => format!("Rate Limited ({}ms interval)", target_rate),
    };

    println!(
        "{} | {} connections | {} usernames",
        target_mode,
        max_connections,
        ctx.usernames.len()
    );
    println!("\nPress Ctrl+C to stop\n");

    let stats_ref = stats.clone();
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(1));
        let start = std::time::Instant::now();

        loop {
            interval.tick().await;
            let votes = stats_ref.votes.load(Ordering::Relaxed);
            let errors = stats_ref.errors.load(Ordering::Relaxed);
            let elapsed = start.elapsed().as_secs_f64();

            let avg_rps = votes as f64 / elapsed;
            let avg_rpm = avg_rps * 60.0;

            print!(
                "\rTotal: {} | Errors: {} | Rate: {:.0} v/s ({:.0} v/m)",
                votes, errors, avg_rps, avg_rpm,
            );
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    });

    // acquire permit from semaphore before spawning vote task to respect max concurrency
    let semaphore = Arc::new(Semaphore::new(max_connections));

    if target_rate > 0 {
        let mut interval = time::interval(Duration::from_millis(target_rate));
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Burst);
        loop {
            interval.tick().await;

            let permit = semaphore.clone().acquire_owned().await.unwrap();
            spawn_vote_task(permit, ctx.clone(), stats.clone());
        }
    } else {
        loop {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            spawn_vote_task(permit, ctx.clone(), stats.clone());
        }
    }
}
