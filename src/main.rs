use reqwest::Client;
use colored::*;
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions, File};
use std::io::{Write, BufWriter};
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Semaphore};
use url::Url;
use rand::Rng;



const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 (compatible; CrawlerFr/1.0)";
const FILE_DOMAINS: &str = "domaines.txt";
const FILE_LINKS: &str = "liens.txt";
const MAX_BYTES: u64 = 1_000_000_000;
const MAX_MS : u64 = 1000;

const DEFAULTMAX_CONCURRENT_REQUESTS: usize = 15;



const SEED_URLS_FR: &[&str] = &[
    "https://www.google.fr/",
    "https://www.orange.fr/",
    "https://www.free.fr/",
    "https://www.sfr.fr/",
    "https://www.laposte.fr/",
    "https://www.lemonde.fr/",
    "https://www.lefigaro.fr/",
    "https://www.liberation.fr/",
    "https://www.francetvinfo.fr/",
    "https://www.20minutes.fr/",
    "https://www.lesechos.fr/",
    "https://www.ouest-france.fr/",
    "https://www.leparisien.fr/",
    "https://www.lavoixdunord.fr/",
    "https://www.mediapart.fr/",
    "https://www.info.gouv.fr/",
    "https://www.data.gouv.fr/",
    "https://www.service-public.fr/",
    "https://www.legifrance.gouv.fr/",
    "https://www.insee.fr/",
    "https://www.bnf.fr/",
    "https://www.culture.gouv.fr/",
    "https://www.francetravail.fr/",
    "https://www.cnrs.fr/",
    "https://www.inria.fr/",
    "https://www.sorbonne-universite.fr/",
    "https://www.univ-rennes.fr/",
    "https://www.univ-lyon1.fr/",
    "https://www.leboncoin.fr/",
    "https://www.pagesjaunes.fr/",
    "https://www.allocine.fr/",
    "https://www.doctissimo.fr/",
    "https://www.pap.fr/",
];

const SEED_URLS: &[&str] = &[
    "https://curlie.org/",
    "https://www.wikipedia.org/",
    "https://archive.org/",
    "https://www.w3.org/",
    "https://dmoz-odp.org/",
    "https://www.google.com/",
    "https://www.bing.com/",
    "https://duckduckgo.com/",
    "https://www.baidu.com/",
    "https://yandex.com/",
    "https://www.ecosia.org/",
    "https://github.com/",
    "https://gitlab.com/",
    "https://sourceforge.net/",
    "https://news.ycombinator.com/",
    "https://www.reddit.com/",
    "https://stackoverflow.com/",
    "https://crates.io/",
    "https://npmjs.com/",
    "https://pypi.org/",
    "https://www.bbc.com/",
    "https://www.reuters.com/",
    "https://www.apnews.com/",
    "https://www.cnn.com/",
    "https://www.theguardian.com/",
    "https://www.nytimes.com/",
    "https://www.dw.com/",
    "https://www.aljazeera.com/",
    "https://elpais.com/",
    "https://www.asahi.com/",
    "https://www.un.org/",
    "https://www.who.int/",
    "https://www.nasa.gov/",
    "https://home.cern/",
    "https://arxiv.org/",
    "https://www.mit.edu/",
    "https://www.ox.ac.uk/",
    "https://www.ethz.ch/",
    "https://www.u-tokyo.ac.jp/",
    "https://medium.com/",
    "https://wordpress.org/",
    "https://substack.com/",
    "https://www.producthunt.com/",
];



struct CrawlResult {
    source: String,
    // FIX 1: On remonte l'URL précise découverte, pas seulement le domaine
    discovered: HashMap<String, String>, 
    autorise: bool,
    erreur: Option<String>
}

pub struct GraphStorage {
    domain_to_index: HashMap<String, (usize, bool)>,
    file_domains: BufWriter<File>,
    file_links: BufWriter<File>,
}

impl GraphStorage {
    
    pub fn new() -> std::io::Result<Self> {
        let file_domains = OpenOptions::new().create(true).append(true).open(FILE_DOMAINS)?;
        let file_links = OpenOptions::new().create(true).append(true).open(FILE_LINKS)?;

        Ok(Self {
            domain_to_index: HashMap::new(),
            file_domains: BufWriter::new(file_domains),
            file_links: BufWriter::new(file_links),
        })
    }

    fn append_domain(&mut self, domain: &str) -> std::io::Result<()> {
        writeln!(self.file_domains, "{}", domain)?;
        //self.file_domains.flush()?;
        Ok(())
    }

    fn append_link(&mut self, line: &str) -> std::io::Result<()> {
        writeln!(self.file_links, "{}", line)?;
        //self.file_links.flush()?;
        Ok(())
    }

    // FIX 1: La file d'attente stocke maintenant (Domaine, URL_Exacte)
    pub fn load_or_create() -> std::io::Result<(Self, Vec<(String, String)>)> {
        let mut storage = Self::new()?;
        let mut visited_indices = HashSet::new();

        if std::path::Path::new(FILE_DOMAINS).exists() {
            let content = fs::read_to_string(FILE_DOMAINS)?;
            for (idx, line) in content.lines().enumerate() {
                let domain = line.trim();
                if !domain.is_empty() {
                    storage.domain_to_index.insert(domain.to_string(), (idx, false));
                }
            }
        }

        if std::path::Path::new(FILE_LINKS).exists() {
            let content = fs::read_to_string(FILE_LINKS)?;
            for line in content.lines() {
                if let Some(first_word) = line.split_whitespace().next() {
                    if let Ok(src_idx) = first_word.parse::<usize>() {
                        visited_indices.insert(src_idx);
                    }
                }
            }
        }

        let mut to_visit = Vec::new();
        for (domain, (idx, is_visited)) in storage.domain_to_index.iter_mut() {
            if visited_indices.contains(idx) {
                *is_visited = true;
            } else {
                // Pour une reprise, on est obligé de retomber sur la page d'accueil
                to_visit.push((domain.clone(), format!("https://{}", domain)));
            }
        }

        Ok((storage, to_visit))
    }
    
    fn get_or_insert_domain(&mut self, domain: &str) -> std::io::Result<usize> {
        if let Some((index, _)) = self.domain_to_index.get(domain) {
            Ok(*index)
        } else {
            let next_index = self.domain_to_index.len();
            self.domain_to_index.insert(domain.to_string(), (next_index, false));
            self.append_domain(domain)?;
            Ok(next_index)
        }
    }

    pub fn mark_as_visited(&mut self, domain: &str) -> std::io::Result<bool> {
        if let Some((_, is_visited)) = self.domain_to_index.get_mut(domain) {
            if *is_visited {
                return Ok(true); 
            }
            *is_visited = true; 
            Ok(false)
        } else {
            let idx = self.get_or_insert_domain(domain)?;
            self.domain_to_index.insert(domain.to_string(), (idx, true));
            Ok(false)
        }
    }

    // FIX 1: Prend le HashMap d'URLs exactes et renvoie les nouveaux tuples (Domaine, URL)
    pub fn save_crawl_results(
        &mut self,
        source_domain: &str,
        discovered: &HashMap<String, String>,
    ) -> std::io::Result<Vec<(String, String)>> {
        let source_index = self.get_or_insert_domain(source_domain)?;

        let mut target_indexes = Vec::new();
        let mut new_domains = Vec::new();

        for (target_domain, full_url) in discovered {
            if !self.domain_to_index.contains_key(target_domain) {
                new_domains.push((target_domain.clone(), full_url.clone()));
            }

            let target_index = self.get_or_insert_domain(target_domain)?;
            target_indexes.push(target_index.to_string());
        }

        let line = format!("{} {}", source_index, target_indexes.join(" "));
        self.append_link(line.trim())?;

        self.flush()?;

        Ok(new_domains)
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.file_domains.flush()?;
        self.file_links.flush()?;
        Ok(())
    }
}



async fn is_allowed_by_robots(client: &Client, domain: &str) -> bool {
    let robots_url = format!("https://{}/robots.txt", domain);

    let res = match client.get(&robots_url).header("User-Agent", USER_AGENT).send().await {
        Ok(res) => res,
        Err(_) => return true,
    };

    if !res.status().is_success() {
        return true;
    }

    let body = match res.text().await {
        Ok(text) => text,
        Err(_) => return true,
    };

    let mut current_user_agent_applies = false;

    for line in body.lines() {
        let clean_line = line.split('#').next().unwrap_or("").trim();
        if clean_line.is_empty() { continue; }

        let mut parts = clean_line.splitn(2, ':');
        let key = match parts.next() { Some(k) => k.trim().to_lowercase(), None => continue };
        let value = match parts.next() { Some(v) => v.trim(), None => continue };

        if key == "user-agent" {
            current_user_agent_applies = value == "*" || USER_AGENT.to_lowercase().contains(&value.to_lowercase());
        } else if key == "disallow" && current_user_agent_applies {
            if value == "/" { return false; }
        }
    }
    true
}

fn extract_domain_from_url(parsed_url: &Url) -> Option<String> {
    if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
        return None;
    }

    /*
    let host = parsed_url.host_str()?.to_lowercase();

    if host.ends_with(".fr") {
        let clean_host = host.strip_prefix("www.").unwrap_or(&host);
        Some(clean_host.to_string())
    } else {
        None
    }
    */
    
    Some(parsed_url.host_str()?.to_lowercase())
}

async fn crawl_page(client: &Client, url_str: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error + Send + Sync>> {
    
    let base_url = Url::parse(url_str)?;

    match client.get(url_str).header("User-Agent", USER_AGENT).send().await {
        Ok(res) => {
            let body = res.text().await?;
            let document = Html::parse_document(&body);
            let selector = Selector::parse("a[href]").unwrap();

            let mut discovered_domains = HashMap::new();

            for element in document.select(&selector) {
                if let Some(href) = element.value().attr("href") {
                    if let Ok(full_url) = base_url.join(href) {
                        if let Some(domain) = extract_domain_from_url(&full_url) {
                            // On conserve l'URL exacte pour contourner le cul-de-sac de la page d'accueil
                            discovered_domains.entry(domain).or_insert_with(|| full_url.to_string());
                        }
                    }
                }
            }

            Ok(discovered_domains)
        }
        Err(err) => {
            let code_erreur = err
                .status()
                .map(|s| s.as_u16().to_string())
                .unwrap_or_else(|| {
                    if err.is_timeout() { "TIMEOUT".to_string() } 
                    else if err.is_connect() { "CONNECT_ERR".to_string() } 
                    else { "NET_ERR".to_string() }
                });
            Err(code_erreur.into())
        }
    }
}

fn encore_stockage() -> bool {
    let size_domains = fs::metadata(FILE_DOMAINS).map(|m| m.len()).unwrap_or(0);
    let size_links = fs::metadata(FILE_LINKS).map(|m| m.len()).unwrap_or(0);
    size_domains + size_links < MAX_BYTES
}

fn demander_max_tasks() -> usize {
    
    print!("Nombre de tâches simultanées max (défaut {}) : ", DEFAULTMAX_CONCURRENT_REQUESTS);
    io::stdout().flush().unwrap(); // Force l'affichage immédiat du texte

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_ok() {
        if let Ok(valeur) = input.trim().parse::<usize>() {
            if valeur > 0 {
                return valeur;
            }
        }
    }

    // En cas de touche Entrée directe ou de valeur invalide
    println!("Utilisation de la valeur par défaut : {}", DEFAULTMAX_CONCURRENT_REQUESTS);
    DEFAULTMAX_CONCURRENT_REQUESTS
}

async fn attendre_signal_arret() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Échec de la capture de Ctrl+C");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Échec de la capture du signal SIGTERM")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => println!("\nSignal Ctrl+C (SIGINT) reçu."),
        _ = terminate => println!("\nSignal d'arrêt système (SIGTERM) reçu."),
    }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let start_time = Instant::now();
    let mut minute_count = (0,0);
    let mut minute_start = Instant::now();
    let mut last_minute_total = (0,0);

    let max_concurrent_requests = demander_max_tasks();

    let client = Arc::new(
        Client::builder()
            .timeout(Duration::from_secs(8))
            .connect_timeout(Duration::from_secs(6))
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?,
    );

    let semaphore = Arc::new(Semaphore::new(max_concurrent_requests));
    let (tx, mut rx) = mpsc::channel::<CrawlResult>(500);

    let (mut storage, mut to_visit) = GraphStorage::load_or_create()?;

    if to_visit.is_empty() {
        println!("Nouveau crawl à partir des graines");
        

        for url_str in SEED_URLS {
            if let Ok(parsed) = Url::parse(url_str) {
                if let Some(seed_domain) = extract_domain_from_url(&parsed) {
                    storage.get_or_insert_domain(&seed_domain)?;
                    if let Some((_, is_visited)) = storage.domain_to_index.get(&seed_domain) {
                        if !is_visited {
                            to_visit.push((seed_domain, url_str.to_string()));
                        }
                    }
                }
            }
        }
    }

    println!(
        "Reprise du crawl : {} domaines connus | {} en attente de visite.",
        storage.domain_to_index.len().to_string().bold().cyan(),
        to_visit.len().to_string().bold().blue()
    );

    let mut active_tasks = 0;

    loop {
        
        while active_tasks < max_concurrent_requests && !to_visit.is_empty() && encore_stockage() {
            let (current_domain, current_url) = {
                let mut rng = rand::thread_rng();
                let idx = rng.gen_range(0..to_visit.len());
                to_visit.swap_remove(idx)
            };

            if storage.mark_as_visited(&current_domain)? {
                continue;
            }

            let client_clone = Arc::clone(&client);
            let semaphore_clone = Arc::clone(&semaphore);
            let tx_clone = tx.clone();

            active_tasks += 1;

            tokio::spawn(async move {
                let _permit = semaphore_clone.acquire().await.unwrap();

                let delay = rand::random::<u64>() % MAX_MS;
                tokio::time::sleep(Duration::from_millis(delay)).await;

                let mut discovered = HashMap::new();
                let autorise = is_allowed_by_robots(&client_clone, &current_domain).await;
                let mut erreur = None;

                if autorise {
                    let crawl = crawl_page(&client_clone, &current_url).await;
                    match crawl {
                        Ok(domains) => discovered = domains,
                        Err(e) => erreur = Some(e.to_string()),
                    }
                }

                let _ = tx_clone.send(CrawlResult { source: current_domain, discovered, autorise, erreur }).await;
            });
        }

        if !encore_stockage() || (to_visit.is_empty() && active_tasks == 0) {
            break;
        }

        tokio::select! {

            _ = attendre_signal_arret() => {
                storage.flush()?;
                println!("{}", "Sauvegarde terminée.".bold().green());
                return Ok(());
            }


            maybe_result = rx.recv() => {

                if let Some(result) = maybe_result {

                    active_tasks -= 1;

                    if minute_start.elapsed().as_secs() >= 60 {
                        last_minute_total = minute_count;
                        minute_count = (0,0);
                        minute_start = Instant::now();
                    }

                    let erreurs = minute_count.1 + if result.erreur.is_some() { 1 } else { 0 };
                    let resultats = minute_count.0 + result.discovered.len();

                    minute_count = (resultats, erreurs);

                    

                    let new_domains = storage.save_crawl_results(&result.source, &result.discovered)?;

                    for target_tuple in new_domains {
                        to_visit.push(target_tuple);
                    }

                    let msg_erreur = if let Some(erreur) = &result.erreur {
                        erreur.to_string().bold().red()
                    } else if !result.autorise {
                        "Ignoré".to_string().bold().yellow()
                    } else {
                        "".to_string().black()
                    };

                    let minutes_ecoulees = (start_time.elapsed().as_secs_f32() / 60.) as usize;

                    println!(
                        "[{}] [{}] ({}/m) ({}/m) {} En Attente | {} + {} Découverts | {} {}",
                        active_tasks.to_string().bold(),
                        minutes_ecoulees.to_string().bold().white(),
                        last_minute_total.0.to_string().bold().cyan(),
                        last_minute_total.1.to_string().bold().red(),
                        to_visit.len().to_string().bold().blue(),
                        storage.domain_to_index.len().to_string().bold().cyan(),
                        result.discovered.len().to_string().bold().cyan(),
                        result.source,
                        msg_erreur,
                    );
                }
            }
        }
    }

    // Flush de sécurité si la boucle se termine naturellement
    storage.flush()?;
    println!("\nCrawl terminé : {} domaines découverts", storage.domain_to_index.len().to_string().bold().cyan());
    Ok(())
}