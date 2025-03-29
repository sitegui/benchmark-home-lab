use clap::Parser;
use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use tokio::{io, try_join};

#[derive(Parser)]
enum Cli {
    Benchmark {
        #[clap(long)]
        file: String,
        #[clap(long, default_value_t = 30.0)]
        transcode_seconds: f64,
        #[clap(long, default_value_t = 1144)]
        echo_port: u16,
        #[clap(long)]
        remote_ip: IpAddr,
    },
    EchoServer {
        #[clap(long, default_value_t = 1144)]
        port: u16,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli {
        Cli::Benchmark {
            file,
            transcode_seconds,
            echo_port,
            remote_ip,
        } => {
            benchmark(file, transcode_seconds, echo_port, remote_ip).await;
        }
        Cli::EchoServer { port } => {
            echo_server(port).await;
        }
    }
}

async fn benchmark(file_path: String, transcode_seconds: f64, echo_port: u16, remote_ip: IpAddr) {
    time("Read file", read(&file_path)).await;
    time("Read file again", read(&file_path)).await;

    time(
        "Transcoded file",
        transcode(&file_path, Duration::from_secs_f64(transcode_seconds)),
    )
    .await;

    time(
        "Transferred data locally",
        transfer(
            &file_path,
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), echo_port),
        ),
    )
    .await;

    time(
        "Transferred data in LAN",
        transfer(&file_path, SocketAddr::new(remote_ip, echo_port)),
    )
    .await;
}

async fn echo_server(port: u16) {
    let server = TcpListener::bind(("0.0.0.0", port))
        .await
        .expect("failed to bind");

    loop {
        let (connection, address) = server.accept().await.expect("failed to accept connection");
        println!("Got connection from {}", address);

        tokio::spawn(async move {
            let (mut reader, mut writer) = connection.into_split();
            io::copy(&mut reader, &mut writer)
                .await
                .expect("failed to echo bytes");
            println!("Finished connection from {}", address);
        });
    }
}

async fn read(file_path: &str) -> u8 {
    hash(File::open(file_path).await.expect("failed to open file")).await
}

async fn transcode(file_path: &str, duration: Duration) -> u8 {
    let mut child = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-t",
            &duration.as_secs_f64().to_string(),
            "-i",
            file_path,
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-r",
            "30",
            "-crf",
            "26",
            "-f",
            "matroska",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn ffmpeg process");

    let mut child_err = child.stderr.take().expect("failed to open ffmpeg stderr");
    let read_err_future = tokio::spawn(async move {
        let mut child_err_str = String::new();
        child_err
            .read_to_string(&mut child_err_str)
            .await
            .expect("failed to read ffmpeg stderr");
        child_err_str
    });

    let child_out = child.stdout.take().expect("failed to open ffmpeg stdout");
    let hash_future = tokio::spawn(hash(child_out));

    let status_future =
        tokio::spawn(async move { child.wait().await.expect("ffmpeg failed to execute") });

    let (read_err, hash, status) =
        try_join!(read_err_future, hash_future, status_future).expect("failed to transcode");
    if !status.success() {
        panic!("failed to execute ffmpeg, got stderr:\n{}", read_err);
    }

    hash
}

async fn transfer(file_path: &str, address: SocketAddr) -> u8 {
    let connection = TcpStream::connect(address)
        .await
        .expect("failed to connect");

    let (reader, mut writer) = connection.into_split();

    let mut file = File::open(file_path).await.expect("failed to open file");
    let copy_future = tokio::spawn(async move {
        io::copy(&mut file, &mut writer)
            .await
            .expect("failed to send file");
    });

    let hash_future = tokio::spawn(async move { hash(reader).await });

    let (_, hash) = try_join!(copy_future, hash_future).expect("failed to transfer");

    hash
}

async fn hash(mut reader: impl AsyncRead + Unpin) -> u8 {
    let mut buffer = [0; 1024];
    let mut hash = 0;
    loop {
        let bytes = reader.read(&mut buffer).await.expect("failed to read file");
        if bytes == 0 {
            break;
        }
        for &byte in buffer[..bytes].iter() {
            hash ^= byte;
        }
    }

    hash
}

async fn time<F, O>(name: &str, f: F)
where
    F: Future<Output = O>,
    O: Debug,
{
    let start = Instant::now();
    let t = f.await;

    println!(
        "{} in {:.1} s (got {:?})",
        name,
        start.elapsed().as_secs_f64(),
        t
    );
}
