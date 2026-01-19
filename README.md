![banner (2)](https://github.com/user-attachments/assets/4379e6ec-86a0-4f90-a359-2d9a47dd509d)
# Devoter
A minimal, high-throughput Votifier stress-tester. Designed to benchmark server performance under heavy vote load or potential protocol abuse. 
Uses Votifier v1 protocol to simulate high-concurrency vote spikes.

Was created with [Pay Everyone](https://github.com/aurickk/Pay-Everyone)'s Player Export feature in mind.

Note: This tool is for educational purposes and stress-testing your own infrastructure. Use it only on your local server.

## Configuration
```jsonc
{
  "votifier_host": "localhost",
  "votifier_port": 8192,
  "votifier_key": "MIIBIjANBgkqhk...", // public Votifier RSA key
  "site_name": "devoter", // note that a random suffix will be added to the site name, to bypass voting limits
  "rate": 0, // delay between the requests in milliseconds. use 0 to disable
  "max_connections": 350 // maximum amount of concurrent connections. values 50-500 seem to work best
}
```

## Setup
### Building from source
Requires [Rust](https://rust-lang.org/tools/install/). Compiling yourself is prefered method for performance and stability.

1. Clone the repository
```sh
git clone https://github.com/ploxxxy/devoter
```

2. Build using the release flag. There is no reason to run this in debug mode.

NOTE: It might take around 10 minutes to compile this for the first time. This is because you're building OpenSSL from scratch. Subsequent builds are much faster.
```sh
cargo build --release -vv
```

3. Moving the binary
When moving the executable don't forget to include `config.json` and `scanned_players` to the launch directory:
```
devoter.exe
config.json
scanned_players.json
```

### Pre-built binary
Standard Windows 10 x64 / AVX2 build available [here](https://github.com/ploxxxy/devoter/releases/latest/). It *should* work on any modern CPU.
