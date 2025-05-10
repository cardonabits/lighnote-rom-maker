use clap::{Arg, Command};
use csv::{Reader, StringRecord};
use indicatif::ProgressBar;
use std::error::Error;
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
    from_puzzle_id: Option<String>,
    to_puzzle_id: Option<String>,
    generate_rom: bool,
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
        .arg(Arg::new("from-puzzle-id")
            .long("from-puzzle-id")
            .value_name("ID")
            .help("Skip puzzles with IDs lexicographically before this"))
        .arg(Arg::new("to-puzzle-id")
            .long("to-puzzle-id")
            .value_name("ID")
            .help("Skip puzzles with IDs lexicographically after this"))
        .arg(Arg::new("do-not-generate-rom")
            .long("do-not-generate-rom")
            .help("Skip generating lightnote.rom file")
            .action(clap::ArgAction::SetTrue))
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
        from_puzzle_id: matches.get_one::<String>("from-puzzle-id").cloned(),
        to_puzzle_id: matches.get_one::<String>("to-puzzle-id").cloned(),
        generate_rom: !matches.get_flag("do-not-generate-rom"),
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
        if let Some(from_id) = &config.from_puzzle_id {
            println!("  From puzzle ID: {}", from_id);
        }
        if let Some(to_id) = &config.to_puzzle_id {
            println!("  To puzzle ID: {}", to_id);
        }
        println!("  ROM generation: {}", if config.generate_rom { "enabled" } else { "disabled" });
    }

    let mut rdr = Reader::from_reader(std::io::stdin());
    let headers = rdr.headers()?.clone();
    println!("CSV Headers: {:?}", headers);
    
    let records: Vec<StringRecord> = rdr.records().collect::<Result<_, _>>()?;
    let total_records = records.len();
    println!("Found {} records (including header)", total_records + 1);
    
    let pb = ProgressBar::new(total_records as u64);
    const ROW_SIZE: usize = 96;
    const FLASH_SIZE: usize = 16_777_216;
    const CONFIG_SECTOR_SIZE: usize = 0x1000;
    const MAX_ROM_PAGES: usize = (FLASH_SIZE - CONFIG_SECTOR_SIZE) / ROW_SIZE;

    let mut puzzle_count = 0;
    let mut page_count = 0;
    let mut skipped_count = 0;
    let mut current_puzzle_pages = 0;

    // Process each record
    for record in records {
        // Check if adding this puzzle would exceed capacity
        if page_count + current_puzzle_pages > MAX_ROM_PAGES {
            if config.verbose || config.dry_run {
                println!("ROM capacity reached ({} pages)", MAX_ROM_PAGES);
            }
            break;
        }
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
            Ok(pages) => {
                current_puzzle_pages = pages;
                page_count += pages;
                puzzle_count += 1;
            }
            Err(e) => {
                println!("Error processing puzzle {}: {}", puzzle.id, e);
                continue;
            }
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

    if config.generate_rom {
        generate_rom(page_count)?;
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
        if !puzzle.themes.iter().any(|t| t.contains(theme)) {
            return true;
        }
    }

    // Check puzzle ID range
    if let Some(from_id) = &config.from_puzzle_id {
        if puzzle.id < *from_id {
            return true;
        }
    }
    if let Some(to_id) = &config.to_puzzle_id {
        if puzzle.id > *to_id {
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
        if !puzzle.themes.iter().any(|t| t.contains(theme)) {
            return format!("missing required theme '{}' (has: {})", theme, puzzle.themes.join(", "));
        }
    }
    let piece_chars: Vec<char> = puzzle.fen.chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    if let Some(excluded) = config.exclude_pieces.iter().find(|p| piece_chars.contains(p)) {
        return format!("contains excluded piece '{}'", excluded);
    }
    if let Some(from_id) = &config.from_puzzle_id {
        if puzzle.id < *from_id {
            return format!("ID {} < from ID {}", puzzle.id, from_id);
        }
    }
    if let Some(to_id) = &config.to_puzzle_id {
        if puzzle.id > *to_id {
            return format!("ID {} > to ID {}", puzzle.id, to_id);
        }
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

fn generate_rom(row_count: usize) -> Result<(), Box<dyn Error>> {
    const ROW_SIZE: usize = 96;
    const FLASH_SIZE: usize = 16_777_216;
    const CONFIG_SECTOR_SIZE: usize = 0x1000;
    const MAX_ROM_DATA_SIZE: usize = FLASH_SIZE - CONFIG_SECTOR_SIZE;

    let rom_file = "lightnote.rom";
    let _ = fs::remove_file(rom_file);

    println!("Generating rom file...");
    
    // Group puzzle files by their base ID (everything before last hyphen and number)
    let mut puzzle_groups: Vec<Vec<std::path::PathBuf>> = Vec::new();
    let mut current_group: Vec<std::path::PathBuf> = Vec::new();
    let mut current_base = String::new();

    let mut puzzle_files = fs::read_dir("fenpuzzles")?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_name().to_string_lossy().ends_with(".txt") {
                Some(entry.path())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    puzzle_files.sort();

    for file in puzzle_files {
        let filename = file.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(last_hyphen) = filename.rfind('-') {
            let base = &filename[..last_hyphen];
            if base != current_base {
                if !current_group.is_empty() {
                    puzzle_groups.push(current_group);
                    current_group = Vec::new();
                }
                current_base = base.to_string();
            }
            current_group.push(file);
        }
    }
    if !current_group.is_empty() {
        puzzle_groups.push(current_group);
    }

    // Write puzzle data - only complete puzzles that fit
    let mut rom_data = Vec::new();
    let mut actual_puzzle_count = 0;
    let mut actual_file_count = 0;

    for group in puzzle_groups {
        // Check if this puzzle will fit
        let puzzle_size = group.len() * ROW_SIZE;
        if rom_data.len() + puzzle_size > MAX_ROM_DATA_SIZE {
            println!("Stopping - next puzzle would exceed ROM capacity");
            break;
        }

        // Write all files for this puzzle
        for file in group {
            let content = fs::read_to_string(&file)?;
            let trimmed = content.trim_end();
            if trimmed.len() > ROW_SIZE {
                return Err(format!("Puzzle data too large in {:?}", file).into());
            }
            rom_data.extend_from_slice(trimmed.as_bytes());
            // Pad to ROW_SIZE
            rom_data.resize(rom_data.len() + (ROW_SIZE - trimmed.len()), 0);
            actual_file_count += 1;
        }
        actual_puzzle_count += 1;
    }

    let free_space = MAX_ROM_DATA_SIZE - rom_data.len();
    println!("Used {} bytes ({} free)", rom_data.len(), free_space);

    // Pad to config sector
    rom_data.resize(MAX_ROM_DATA_SIZE, 0);

    // Write config sector
    let mut config_sector = Vec::new();
    // magic: u32 = 0x11131719
    config_sector.extend_from_slice(&0x11131719u32.to_le_bytes());
    // num_pages: u32 (a record is 1 page)
    config_sector.extend_from_slice(&(row_count as u32).to_le_bytes());
    // total_size: u32
    config_sector.extend_from_slice(&((row_count * ROW_SIZE) as u32).to_le_bytes());
    // num_types: u8
    config_sector.push(0x1);
    // font_size: u8
    config_sector.push(0x1);
    // reserved0, reserved1
    config_sector.extend_from_slice(&0u16.to_le_bytes());
    // type0: u8 (ChessPuzzle = 4)
    config_sector.push(0x4);
    // type1-3: u8
    config_sector.extend_from_slice(&[0u8; 3]);
    // size0: u32
    config_sector.extend_from_slice(&(ROW_SIZE as u32).to_le_bytes());
    // size1-3: u32
    config_sector.extend_from_slice(&[0u8; 12]);
    // Fill remaining config sector with zeros
    config_sector.resize(CONFIG_SECTOR_SIZE, 0);

    // Combine and write final ROM
    rom_data.extend_from_slice(&config_sector);
    fs::write(rom_file, rom_data)?;

    println!("{} puzzles in {} files...", actual_puzzle_count, actual_file_count);
    println!("Done");
    Ok(())
}

fn process_puzzle(puzzle: &Puzzle, config: &Config) -> Result<usize, Box<dyn Error>> {
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

    Ok(processed_moves)
}
