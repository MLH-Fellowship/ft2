fn main() {
    if let Ok(port) = std::env::var("PORT") {
        println!("Starting TCP Listener on port {}.", &port);
        let listener = std::net::TcpListener::bind(format!("0.0.0.0:{}", &port)).unwrap();
        println!("Started TCP Listener on port {}.", &port);
        for _ in listener.incoming() {}
    }
}
