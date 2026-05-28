use clap::{Parser, Subcommand};
use rust_scrapling::fetchers::client::Fetcher;
use rust_scrapling::fetchers::config::FetcherConfig;

#[derive(Parser)]
#[command(name = "rust-scrapling")]
#[command(about = "RUSTScrapling - A Rust web scraping framework", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Fetch a URL and extract content
    Fetch {
        url: String,
        #[arg(short, long)]
        selector: Option<String>,
        #[arg(short, long, default_value = "text")]
        format: String,
        #[arg(long)]
        no_stealth: bool,
    },
    /// Extract text content from a URL
    Extract {
        url: String,
        #[arg(short, long)]
        selector: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Fetch {
            url,
            selector,
            format,
            no_stealth,
        } => {
            let config = FetcherConfig::builder().stealth(!no_stealth).build();
            let fetcher = Fetcher::new(config);
            match fetcher.get(&url).await {
                Ok(response) => {
                    if let Some(css) = selector {
                        let sel = response.selector();
                        let results = sel.css(&css);
                        for item in &results {
                            match format.as_str() {
                                "html" => println!("{}", item.outer_html()),
                                "json" => {
                                    let obj = serde_json::json!({
                                        "tag": item.tag(),
                                        "text": item.text().as_str(),
                                        "html": item.html_content().as_str(),
                                    });
                                    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
                                }
                                _ => println!("{}", item.text()),
                            }
                        }
                        eprintln!("Found {} elements", results.len());
                    } else {
                        match format.as_str() {
                            "html" => println!("{}", response.text()),
                            _ => {
                                let sel = response.selector();
                                println!(
                                    "{}",
                                    sel.get_all_text("\n", true, &["script", "style"], None)
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Extract { url, selector } => {
            let fetcher = Fetcher::new(FetcherConfig::default());
            match fetcher.get(&url).await {
                Ok(response) => {
                    let sel = response.selector();
                    if let Some(css) = selector {
                        let results = sel.css(&css);
                        for item in &results {
                            println!("{}", item.text());
                        }
                    } else {
                        println!(
                            "{}",
                            sel.get_all_text("\n", true, &["script", "style"], None)
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
