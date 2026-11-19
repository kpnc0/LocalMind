mod embed;
mod extract;
mod index;
mod search;

use std::path::{Path, PathBuf};
use anyhow::Result;
use rayon::prelude::*;

use embed::Embedder;
use extract::extract_text;
use index::Index;
use search::top_k;

fn get_home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|_| PathBuf::from("/root"))
}

fn find_database() -> Option<PathBuf> {
    let home_db = get_home_dir().join(".localmind").join("index.bin");
    let tmp_db = PathBuf::from("/tmp/localmind_watch/index.bin");

    match (home_db.exists(), tmp_db.exists()) {
        (true, true) => {
            let mtime_home = std::fs::metadata(&home_db).and_then(|m| m.modified()).ok();
            let mtime_tmp = std::fs::metadata(&tmp_db).and_then(|m| m.modified()).ok();
            if let (Some(th), Some(tt)) = (mtime_home, mtime_tmp) {
                if tt > th {
                    Some(tmp_db)
                } else {
                    Some(home_db)
                }
            } else {
                Some(home_db)
            }
        }
        (true, false) => Some(home_db),
        (false, true) => Some(tmp_db),
        (false, false) => None,
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.starts_with('.') || name.ends_with(".tmp") || name.ends_with(".swp") || name == "index.bin" {
            continue;
        }
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let ignored_exts = [
                "bin", "exe", "png", "jpg", "jpeg", "gif", "webp", "zip", "tar", "gz",
                "bz2", "7z", "so", "dylib", "dll", "o", "a", "pyc", "class",
            ];
            if !ignored_exts.contains(&ext.as_str()) {
                out.push(path);
            }
        }
    }
    Ok(())
}

fn index_directory(dir: &Path, embedder: &Embedder) -> Result<(Vec<Vec<f32>>, Vec<String>)> {
    let mut files = Vec::new();
    collect_files(dir, &mut files)?;
    if files.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    files.sort();

    let records: Vec<Result<(String, Vec<f32>)>> = files
        .par_iter()
        .filter_map(|p| {
            let path_str = p.to_string_lossy().to_string();
            let text = match extract_text(p) {
                Ok(t) => t,
                Err(_) => return None,
            };
            let text = text.trim();
            let emb_res = if text.is_empty() {
                Ok(vec![0.0f32; 384])
            } else if text.len() > 10000 {
                embedder.embed_chunked(text)
            } else {
                embedder.embed(text)
            };
            Some(emb_res.map(|emb| (path_str, emb)))
        })
        .collect();

    let mut vectors = Vec::with_capacity(records.len());
    let mut paths = Vec::with_capacity(records.len());
    for r in records {
        let (path, emb) = r?;
        paths.push(path);
        vectors.push(emb);
    }

    Ok((vectors, paths))
}

fn run_index(embedder: &Embedder) -> Result<()> {
    let watch_dir = PathBuf::from("/tmp/localmind_watch");
    if !watch_dir.exists() {
        std::fs::create_dir_all(&watch_dir)?;
    }
    println!("Indexing files in {}...", watch_dir.display());
    let (vectors, paths) = index_directory(&watch_dir, embedder)?;
    if paths.is_empty() {
        println!("No files found in {} to index.", watch_dir.display());
        return Ok(());
    }
    let count = paths.len();
    let tmp_db = watch_dir.join("index.bin");
    Index::save(&tmp_db, 384, &vectors, &paths)?;
    println!("Successfully indexed {} file(s) into {}", count, tmp_db.display());
    Ok(())
}

fn run_save(embedder: &Embedder) -> Result<()> {
    let localmind_dir = get_home_dir().join(".localmind");
    std::fs::create_dir_all(&localmind_dir)?;
    let save_path = localmind_dir.join("index.bin");
    let tmp_db = PathBuf::from("/tmp/localmind_watch/index.bin");

    if tmp_db.exists() {
        std::fs::copy(&tmp_db, &save_path)?;
        let index = Index::open(&save_path)?;
        println!("Saved database to {} ({} records)", save_path.display(), index.len());
    } else {
        let watch_dir = PathBuf::from("/tmp/localmind_watch");
        if !watch_dir.exists() {
            std::fs::create_dir_all(&watch_dir)?;
        }
        println!("Indexing files in {}...", watch_dir.display());
        let (vectors, paths) = index_directory(&watch_dir, embedder)?;
        if paths.is_empty() {
            println!("No files found in {} to save.", watch_dir.display());
            return Ok(());
        }
        let count = paths.len();
        Index::save(&tmp_db, 384, &vectors, &paths)?;
        Index::save(&save_path, 384, &vectors, &paths)?;
        println!("Saved database to {} ({} records)", save_path.display(), count);
    }
    Ok(())
}

fn run_index_and_save(embedder: &Embedder) -> Result<()> {
    let watch_dir = PathBuf::from("/tmp/localmind_watch");
    if !watch_dir.exists() {
        std::fs::create_dir_all(&watch_dir)?;
    }
    println!("Indexing files in {}...", watch_dir.display());
    let (vectors, paths) = index_directory(&watch_dir, embedder)?;
    if paths.is_empty() {
        println!("No files found in {} to index.", watch_dir.display());
        return Ok(());
    }
    let count = paths.len();
    let tmp_db = watch_dir.join("index.bin");
    Index::save(&tmp_db, 384, &vectors, &paths)?;

    let localmind_dir = get_home_dir().join(".localmind");
    std::fs::create_dir_all(&localmind_dir)?;
    let save_path = localmind_dir.join("index.bin");
    Index::save(&save_path, 384, &vectors, &paths)?;

    println!(
        "Successfully indexed {} file(s) into {} and saved to {}",
        count,
        tmp_db.display(),
        save_path.display()
    );
    Ok(())
}

fn run_search(embedder: &Embedder, query: &str, show_scores: bool) -> Result<()> {
    let db_path = match find_database() {
        Some(p) => p,
        None => {
            eprintln!("Error: No database found. Please run 'localmind --index' first.");
            std::process::exit(1);
        }
    };

    let index = Index::open(&db_path)?;
    if index.len() == 0 {
        println!("Database is empty.");
        return Ok(());
    }

    let query_vec = embedder.embed(query)?;
    let all_results = top_k(&index, &query_vec, index.len() as usize);

    let positive_results: Vec<_> = all_results.into_iter().filter(|r| r.score > 0.0).collect();
    let results = if positive_results.is_empty() && index.len() > 0 {
        top_k(&index, &query_vec, 1)
    } else {
        positive_results
    };

    if results.is_empty() {
        println!("No results found.");
    } else {
        for r in &results {
            if show_scores {
                println!("[{:.4}] {}", r.score, r.path);
            } else {
                println!("{}", r.path);
            }
        }
    }

    Ok(())
}

fn print_usage() {
    println!("LocalMind — Local semantic search CLI

USAGE:
    localmind --index              Index files in /tmp/localmind_watch into a database
    localmind --save               Save database to ~/.localmind directory
    localmind search <query>       Search indexed files for <query>

OPTIONS:
    --index                        Index files in /tmp/localmind_watch
    --save                         Save database to ~/.localmind directory
    --scores                       Show similarity scores in search results
    -h, --help                     Print help information
");
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let has_help = args.iter().any(|a| a == "-h" || a == "--help");
    if has_help {
        print_usage();
        return Ok(());
    }

    let has_index = args.iter().any(|a| a == "--index" || a == "-i");
    let has_save = args.iter().any(|a| a == "--save");
    let show_scores = args.iter().any(|a| a == "--scores" || a == "--score");

    let search_idx = args.iter().position(|a| a == "search" || a == "--search" || a == "-s");

    if let Some(pos) = search_idx {
        let query_tokens: Vec<&str> = args[pos + 1..]
            .iter()
            .filter(|a| !a.starts_with('-'))
            .map(|s| s.as_str())
            .collect();

        if query_tokens.is_empty() {
            eprintln!("Error: Missing query string for search.");
            std::process::exit(1);
        }

        let query = query_tokens.join(" ");
        let embedder = Embedder::new()?;
        return run_search(&embedder, &query, show_scores);
    }

    if has_index || has_save {
        let embedder = Embedder::new()?;
        if has_index && has_save {
            return run_index_and_save(&embedder);
        } else if has_index {
            return run_index(&embedder);
        } else if has_save {
            return run_save(&embedder);
        }
    }

    print_usage();
    Ok(())
}
