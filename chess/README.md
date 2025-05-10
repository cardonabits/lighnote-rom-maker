## To generate puzzles from lichess

## Using legacy bash scripts
```
wget https://database.lichess.org/lichess_db_puzzle.csv.zst
zstd -d lichess_db_puzzle.csv.zst -o lichess_db_puzzle.csv
./genfenpuzzles.sh --theme-tag mate --min-moves 4 <lichess_db_puzzle.csv
```

See `./genfenpuzzles.sh -h` for filter arguments

## To generate rom

```
./fenpuzzles_toflash.sh
```

## Using the rust re-implementation (Experimental)

This will generate the puzzle files as well as the ROM in one single step.
```
cargo run --release -- --theme-tag mate --min-moves 4 <lichess_db_puzzle.csv
```

