[![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/flashy64?style=flat-square)](https://crates.io/crates/flashy64)
[![Documentation](https://img.shields.io/docsrs/flashy64-backend?style=flat-square)](https://docs.rs/flashy64-backend)

### Description
`flashy64` is a tool for interfacing with different N64 flashcarts. All flashcart-specific code can be found in the `flashy64-backend` crate.

The [UNFLoader](https://github.com/buu342/N64-UNFLoader) protocol is supported. However, only receiving text data from the cartridge is available at this time.

#### Cartridges
- 64drive (supported)
- SummerCart64 (planned, high priority)
- Everdrive (low priority)
- PicoCart (low priority)

### Usage
For users, install flashy64 as a runnable program using `cargo install flashy64` (you will need [rustup installed](https://www.rust-lang.org/tools/install))

If you wish to install from source:
```
git clone https://github.com/bigbass1997/flashy64
cd flashy64
cargo install --path .
```

Once installed, run `flashy64 --help` for more details.

If you're a programmer who needs API access, include the `flashy64-backend` crate in your `Cargo.toml` dependencies.