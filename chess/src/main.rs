use clap::{Arg, Command};
use csv::{Reader, StringRecord};
use indicatif::ProgressBar;
use std::{error::Error, thread::current};
use std::fs;
use chess_puzzle_gen::{ChessMove, ChessError, compress_fen, expand_fen, move_to_index, reverse_fen};

#[derive(Debug)]
struct Puzzle {
    id: String,
    fen: String,
    moves: Vec<String>,
    rating: u32,
    themes: Vec<String>,
    first_move: char,
}

#[derive(Debug, Clone)]
struct Config {
    verbose: bool,
    dry_run: bool,
    max_moves: usize,
    min_moves: usize,
    theme_tag: Option<String>,
    max_rating: u32,
    min_rating: u32,
    exclude_pieces: Vec<char>,
    last_move_pieces: Vec<char>,
    max_num_pages: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let matches = Command::new("chess_puzzle_gen")
        .about("Generates chess puzzles from FEN notation")
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .help("Be verbose")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("dry-run")
            .long("dry-run")
            .help("Only count puzzles")
            .action(clap::ArgAction::SetTrue))
        .arg(Arg::new("max-moves").long("max-moves").value_name("NUM").default_value("10"))
        .arg(Arg::new("min-moves").long("min-moves").value_name("NUM").default_value("1"))
        .arg(Arg::new("theme-tag").long("theme-tag").value_name("TAG"))
        .arg(Arg::new("max-rating").long("max-rating").value_name("RATING").default_value("3000"))
        .arg(Arg::new("min-rating").long("min-rating").value_name("RATING").default_value("500"))
        .arg(Arg::new("exclude-pieces").long("exclude-pieces").value_name("PIECES"))
        .arg(Arg::new("last-move-pieces").long("last-move-pieces").value_name("PIECES").default_value("prnbkq"))
        .get_matches();

    let config = Config {
        verbose: matches.get_flag("verbose"),
        dry_run: matches.get_flag("dry-run"),
        max_moves: matches.get_one::<String>("max-moves").unwrap().parse()?,
        min_moves: matches.get_one::<String>("min-moves").unwrap().parse()?,
        theme_tag: matches.get_one::<String>("theme-tag").cloned(),
        max_rating: matches.get_one::<String>("max-rating").unwrap().parse()?,
        min_rating: matches.get_one::<String>("min-rating").unwrap().parse()?,
        exclude_pieces: matches
            .get_one::<String>("exclude-pieces")
            .map(|s| s.to_lowercase().chars().collect())
            .unwrap_or_default(),
        last_move_pieces: matches
            .get_one::<String>("last-move-pieces")
            .unwrap()
            .to_lowercase()
            .chars()
            .collect(),
        max_num_pages: 16 * 1024 * 1024 / 96,
    };

    if config.dry_run {
        println!("Dry run, no puzzles will be generated...");
    } else {
        fs::create_dir_all("fenpuzzles")?;
    }

    if config.verbose {
        println!("Running with configuration:");
        println!("  Min rating: {}", config.min_rating);
        println!("  Max rating: {}", config.max_rating);
        println!("  Min moves: {}", config.min_moves);
        println!("  Max moves: {}", config.max_moves);
        if let Some(theme) = &config.theme_tag {
            println!("  Theme filter: {}", theme);
        }
        if !config.exclude_pieces.is_empty() {
            println!("  Excluding pieces: {:?}", config.exclude_pieces);
        }
        println!("  Last move pieces: {:?}", config.last_move_pieces);
    }

    let mut rdr = Reader::from_reader(std::io::stdin());
    let headers = rdr.headers()?.clone();
    println!("CSV Headers: {:?}", headers);
    
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_, _>>()?;
    let total_records = records.len();
    println!("Found {} records (including header)", total_records + 1);
    
    let pb = ProgressBar::new(total_records as u64);
    let mut puzzle_count = 0;
    let mut page_count = 0;
    let mut skipped_count = 0;

    // Process each record
    for record in records {
        pb.inc(1);
        if config.verbose {
            println!("Processing record: {:?}", record);
        }

        let puzzle = match parse_puzzle_record(&record) {
            Ok(p) => p,
            Err(e) => {
                if config.verbose {
                    println!("Error parsing puzzle record: {}", e);
                }
                continue;
            }
        };
        
        if should_skip_puzzle(&puzzle, &config) {
            if config.verbose {
                let reason = skip_reason(&puzzle, &config);
                println!("Skipping puzzle {}: {}", puzzle.id, reason);
            }
            skipped_count += 1;
            continue;
        }

        match process_puzzle(&puzzle, &config) {
            Ok(_) => puzzle_count += 1,
            Err(e) => {
                println!("Error processing puzzle {}: {}", puzzle.id, e);
                continue;
            }
        }
        
        page_count += puzzle.moves.len();

        if page_count > config.max_num_pages {
            if config.verbose || config.dry_run {
                println!("Maximum pages limit ({}) reached", config.max_num_pages);
            }
            break;
        }
    }

    pb.finish_with_message("Done");

    let kbytes = page_count * 96 / 1024;
    println!("\nSummary:");
    println!("  Total puzzles processed: {}", total_records);
    println!("  Puzzles generated: {}", puzzle_count);
    println!("  Puzzles skipped: {}", skipped_count);
    println!("  Total screens/pages: {} ({} KB)", page_count, kbytes);
    
    if config.verbose && skipped_count > 0 {
        println!("\nSkipped puzzles breakdown:");
        // Could add more detailed breakdown here if needed
    }

    Ok(())
}

fn parse_puzzle_record(record: &StringRecord) -> Result<Puzzle, Box<dyn Error>> {
    if record.len() < 8 {
        return Err("Invalid record format".into());
    }

    let fen = record[1].split_whitespace().next().unwrap_or("").to_string();
    let first_move = record[1].split_whitespace()
        .nth(1)
        .and_then(|s| s.chars().next())
        .unwrap_or('w');

    Ok(Puzzle {
        id: record[0].to_string(),
        fen,
        moves: record[2].split_whitespace().map(|s| s.to_string()).collect(),
        rating: record[3].parse()?,
        themes: record[7].split(',').map(|s| s.trim().to_lowercase()).collect(),
        first_move,
    })
}

fn should_skip_puzzle(puzzle: &Puzzle, config: &Config) -> bool {
    // Check move count - be more lenient
    if puzzle.moves.is_empty() {
        return true;
    }
    if puzzle.moves.len() > config.max_moves {
        return true;
    }
    if puzzle.moves.len() < config.min_moves {
        return true;
    }

    // Check rating
    if puzzle.rating > config.max_rating {
        return true;
    }
    if puzzle.rating < config.min_rating {
        return true;
    }

    // Check excluded pieces - only look at piece characters
    let piece_chars: Vec<char> = puzzle.fen.chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    if config.exclude_pieces.iter().any(|p| piece_chars.contains(p)) {
        return true;
    }

    // Check theme tag if specified
    if let Some(theme) = &config.theme_tag {
        if !puzzle.themes.iter().any(|t| t == theme) {
            return true;
        }
    }

    false
}

fn skip_reason(puzzle: &Puzzle, config: &Config) -> String {
    if puzzle.moves.is_empty() {
        return "no moves".to_string();
    }
    if puzzle.moves.len() > config.max_moves {
        return format!("move count {} > max {}", puzzle.moves.len(), config.max_moves);
    }
    if puzzle.moves.len() < config.min_moves {
        return format!("move count {} < min {}", puzzle.moves.len(), config.min_moves);
    }
    if puzzle.rating > config.max_rating {
        return format!("rating {} > max {}", puzzle.rating, config.max_rating);
    }
    if puzzle.rating < config.min_rating {
        return format!("rating {} < min {}", puzzle.rating, config.min_rating);
    }
    if let Some(theme) = &config.theme_tag {
        if !puzzle.themes.iter().any(|t| t == theme) {
            return format!("missing required theme '{}' (has: {})", theme, puzzle.themes.join(", "));
        }
    }
    let piece_chars: Vec<char> = puzzle.fen.chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    if let Some(excluded) = config.exclude_pieces.iter().find(|p| piece_chars.contains(p)) {
        return format!("contains excluded piece '{}'", excluded);
    }
    "unknown reason (this shouldn't happen)".to_string()
}

pub fn parse_move(move_str: &str) -> Result<ChessMove, ChessError> {
    if move_str.len() < 4 {
        return Err(ChessError::InvalidMove);
    }
    
    let from_file = move_str.chars().nth(0).unwrap() as u8 - b'a';
    let from_rank = 8 - move_str.chars().nth(1).unwrap().to_digit(10).unwrap() as u8;
    let to_file = move_str.chars().nth(2).unwrap() as u8 - b'a';
    let to_rank = 8 - move_str.chars().nth(3).unwrap().to_digit(10).unwrap() as u8;
    
    let from = (from_rank * 8 + from_file) as u8;
    let to = (to_rank * 8 + to_file) as u8;
    
    let promotion = if move_str.len() > 4 {
        Some(move_str.chars().nth(4).unwrap())
    } else {
        None
    };
    
    Ok(ChessMove { from, to, promotion })
}

pub fn apply_move(fen: &str, chess_move: &ChessMove) -> Result<(String, char), ChessError> {
    let mut expanded = expand_fen(fen);
    let from_char = expanded.chars().nth(chess_move.from as usize).ok_or(ChessError::InvalidMove)?;
    
    // Handle promotion
    let to_piece = match chess_move.promotion {
        Some(p) => {
            if from_char.is_uppercase() {
                p.to_ascii_uppercase()
            } else {
                p.to_ascii_lowercase()
            }
        }
        None => from_char,
    };
    
    // Apply the move
    expanded.replace_range(chess_move.from as usize..=chess_move.from as usize, "1");
    expanded.replace_range(chess_move.to as usize..=chess_move.to as usize, &to_piece.to_string());
    
    Ok((compress_fen(&expanded), from_char))
}

fn process_puzzle(puzzle: &Puzzle, config: &Config) -> Result<(), Box<dyn Error>> {
    let mut current_fen = puzzle.fen.split_whitespace().next().unwrap().to_string();
    let mut processed_moves = 0;

    // First pass: validate all moves
    for move_str in &puzzle.moves {
        match parse_move(move_str) {
            Ok(chess_move) => {
                let (new_fen, _) = apply_move(&current_fen, &chess_move)?;
                current_fen = new_fen;
                processed_moves += 1;
            }
            Err(e) => {
                if config.verbose {
                    println!("Failed to parse move '{}' in position {}: {}", move_str, current_fen, e);
                }
                return Err(Box::new(e));
            }
        }
    }

    // Second pass: generate files only if all moves are valid
    if processed_moves == puzzle.moves.len() {
        current_fen = puzzle.fen.split_whitespace().next().unwrap().to_string();
        let mut moved_piece = ' ';
        
        for (i, move_str) in puzzle.moves.iter().enumerate() {
            let (new_fen, piece) = apply_move(&current_fen, &parse_move(move_str)?)?;
            current_fen = new_fen;
            moved_piece = piece;

            let move_num = i + 1;
            let outfile = format!(
                "fenpuzzles/puzzle-{}-{}-{}-{:02}.txt",
                puzzle.id,
                puzzle.rating,
                config.theme_tag.as_deref().unwrap_or("none"),
                move_num
            );

            let reversed = puzzle.first_move == 'w';
            let output_fen = if reversed { reverse_fen(&current_fen) } else { current_fen.clone() };
            let expanded_fen = chess_puzzle_gen::expand_fen(&output_fen);
            let imove = move_to_index(move_str, reversed)?;

            let content = format!(
                "{},{},{},{},{}",
                puzzle.id,
                expanded_fen,
                imove,
                move_num,
                puzzle.moves.len()
            );
            fs::write(outfile, content)?;
        }

        // Clean up if last move piece doesn't match filter
        if !config.last_move_pieces.contains(&moved_piece.to_ascii_lowercase()) {
            for i in 0..puzzle.moves.len() {
                let outfile = format!(
                    "fenpuzzles/puzzle-{}-{}-{}-{:02}.txt",
                    puzzle.id,
                    puzzle.rating,
                    config.theme_tag.as_deref().unwrap_or("none"),
                    i + 1
                );
                fs::remove_file(outfile)?;
            }
        }
    }

    Ok(())
}
