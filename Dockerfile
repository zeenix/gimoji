FROM rust

RUN cargo install cargo-watch cargo-deb
RUN apt update && apt install -qy gcc-x86-64-linux-gnu nvim
