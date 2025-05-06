#! /bin/bash

PUZZLES_ZST=lichess_db_puzzle.csv.zst
PUZZLES=lichess_db_puzzle.csv
PUZZLES_DIR=fenpuzzles
OUTPUT_DIR=output

[ -f ${PUZZLES_ZST} ] || wget https://database.lichess.org/lichess_db_puzzle.csv.zst
[ -f ${PUZZLES} ] || zstd -d lichess_db_puzzle.csv.zst -o lichess_db_puzzle.csv
[ -d ${PUZZLES_DIR} ] || mkdir ${PUZZLES_DIR}
[ -d ${OUTPUT_DIR} ] || mkdir ${OUTPUT_DIR}
echo Patience, this will stay at 0% progress for several minutes and may take
echo 10-20 hours to complete...
./genfenpuzzles.sh --theme-tag mate --min-moves 4 <lichess_db_puzzle.csv
