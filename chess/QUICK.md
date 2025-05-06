## To generate puzzles from lichess

```
wget https://database.lichess.org/lichess_db_puzzle.csv.zst
zstd -d lichess_db_puzzle.csv.zst -o lichess_db_puzzle.csv
./genfenpuzzles.sh --theme-tag mate --min-moves 4 <lichess_db_puzzle.csv
```

or 

```
./quick.sh
```

See `./genfenpuzzles.sh -h` for filter arguments

## To generate rom

```
./fenpuzzles_toflash.sh
```
