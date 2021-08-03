use std::str;
use std::time::{Instant};
use std::io::{BufRead, BufReader, Write, BufWriter};
use std::net::{TcpStream, ToSocketAddrs};
use std::env;
use std::thread;
use std::sync::mpsc;

const BLACK: i8 = 1;
const WHITE: i8 = -1;
const NONE : i8 = 0;
const TURN_CHANGE_FACTOR: i8 = -1;
const BOARDSIZE: i32 = 64;
const LINESIZE: i32 = 8;
const MAX_TURNS: i8 = 60;

// placeの返り値で用いる
const CONTINUE: i8 = 0;
const PLACE_ERR: i8 = -1;
const GAME_SET: i8 = 1;

// evaluateするとき，どの関数で計算するか定める
const EVAL_BY_POINTTABLE: i8 = -1; // デバッグ用
const EVAL_NORMAL: i8 = 0;
const EVAL_PERFECT: i8 = 1; // 完全読み切り（個数も読む）
const EVAL_WIN: i8 = 2; // 勝つかどうかだけ読む（個数は読まない）

const EVAL_BY_POINTTABLE_DEPTH: i8 = 8;
const EVAL_NORMAL_DEPTH: i8 = 8;
const EVAL_PERFECT_DEPTH: i8 = 16;
const EVAL_WIN_DEPTH: i8 = 18;

// eval_normalにおける重み
const WEIGHT_STABLE:   i32 = 101;
const WEIGHT_WING:     i32 = -308;
const WEIGHT_XMOVE:    i32 = -449;
const WEIGHT_CMOVE:    i32 = -552;
const WEIGHT_MOBILITY: i32 = 134;
const WEIGHT_OPENNESS: i32 = -13;

struct BoardInfo {
    now_turn: i8,
    now_index: i8,
    player_board: u64,
    opponent_board: u64,
}

impl Clone for BoardInfo {
    fn clone(&self) -> Self {
        return BoardInfo {
            now_turn: self.now_turn.clone(),
            now_index: self.now_index.clone(),
            player_board: self.player_board.clone(),
            opponent_board: self.opponent_board.clone(),
        };
    }
}

fn max(a: i32, b: i32) -> i32 {
    return if a < b {b} else {a};
}

// 盤面の情報を簡易的に出力する
fn print_board_info_simply(board_info: &BoardInfo) -> () {
    let (black_count, white_count, _superior) = get_result(&board_info);
    println!("{}'s TURN, BLACK:{}, WHITE:{}, INDEX:{}", 
        match board_info.now_turn {
            BLACK => "BLACK",
            WHITE => "WHITE",
            _     => panic!("there is not sych color"),
        },
        black_count,
        white_count,
        board_info.now_index,
    );
}

// 盤面を出力する
fn print_board_info(board_info: &BoardInfo, eval: i32) -> () {
    println!("");
    println!("********************");
    let mut player_color: char = 'b';
    let mut opponent_color: char = 'w';
    let (black_count, white_count, _superior) = get_result(&board_info);
    println!("{}'s TURN, BLACK:{}, WHITE:{}, INDEX:{}, PLAYER's EVAL:{}", 
        match board_info.now_turn {
            BLACK => "BLACK",
            WHITE => {
                player_color = 'w';
                opponent_color = 'b';
                "WHITE"
            },
            _     => panic!("there is not sych color"),
        },
        black_count,
        white_count,
        board_info.now_index,
        eval
    );
    println!("");
    println!("   A B C D E F G H ");
    for i in (0..8).rev() {
        print!(" {} ", i32::abs(8-i));
        for j in (0..8).rev() {
            if ((board_info.player_board >> (i*8)+j) & (1 as u64)) == 1 {
                print!("{} ", player_color);
            }else if ((board_info.opponent_board >> (i*8)+j) & (1 as u64)) == 1 {
                print!("{} ", opponent_color);
            }else{
                print!("- ");
            }
        }
        println!("");
    }
    println!("");
    println!("********************");
}

// char2つによる文字の入力に対応する場所のビットを立てた盤面を返す
fn point_to_bit(inp1: char, inp2: char) -> u64 {
    let mut ret: u64 = 0x8000000000000000;
    match inp1 {
        'A' => (),
        'B' => ret = ret >> 1,
        'C' => ret = ret >> 2,
        'D' => ret = ret >> 3,
        'E' => ret = ret >> 4,
        'F' => ret = ret >> 5,
        'G' => ret = ret >> 6,
        'H' => ret = ret >> 7,
         _  => (),
    }
    let tmp: i32 = (inp2 as i32) - 48;
    ret = ret >> ((tmp-1)*8);

    return ret;
}

// ビットが1つだけ立っているu64の場所をchar2つに変換する
fn bit_to_point(num: u64) -> (char, char) {
    let mut count = 0;
    let mut num_cpy = num;
    while num_cpy != 0x8000000000000000 {
        count = count + 1;
        num_cpy = num_cpy << 1;
    }
    let c1: char = match count % LINESIZE {
        0 => 'A',
        1 => 'B',
        2 => 'C',
        3 => 'D',
        4 => 'E',
        5 => 'F',
        6 => 'G',
        7 => 'H',
        _ => panic!("invalid rem"),
    };
    let c2: char = std::char::from_digit(((count / LINESIZE) + 1) as u32, 10).unwrap();
    return (c1, c2);
}

// player_boardからみておけるマスにフラグが立っている盤面を返す
fn make_legal_board(board_info: &BoardInfo) -> u64 {
    let horizontal_side: u64 = board_info.opponent_board & 0x7e7e7e7e7e7e7e7e;
    let vertical_side: u64 = board_info.opponent_board & 0x00FFFFFFFFFFFF00;
    let all_side: u64 = board_info.opponent_board & 0x007e7e7e7e7e7e00;
    let blank_board: u64 = !(board_info.player_board | board_info.opponent_board);
    let mut tmp: u64;
    let mut legal_board: u64;

    // 左側から挟めるビットを立てる
    tmp = horizontal_side & (board_info.player_board << 1);
    for _ in 0..5 {
        tmp |= horizontal_side & (tmp << 1)
    }
    legal_board = blank_board & (tmp << 1);

    // 右
    tmp = horizontal_side & (board_info.player_board >> 1);
    for _ in 0..5 {
        tmp |= horizontal_side & (tmp >> 1)
    }
    legal_board |= blank_board & (tmp >> 1);

    // 上
    tmp = vertical_side & (board_info.player_board << 8);
    for _ in 0..5 {
        tmp |= vertical_side & (tmp << 8)
    }
    legal_board |= blank_board & (tmp << 8);

    // 下
    tmp = vertical_side & (board_info.player_board >> 8);
    for _ in 0..5 {
        tmp |= vertical_side & (tmp >> 8)
    }
    legal_board |= blank_board & (tmp >> 8);

    // 右上
    tmp = all_side & (board_info.player_board << 7);
    for _ in 0..5 {
        tmp |= all_side & (tmp << 7)
    }
    legal_board |= blank_board & (tmp << 7);

    // 左上
    tmp = all_side & (board_info.player_board << 9);
    for _ in 0..5 {
        tmp |= all_side & (tmp << 9)
    }
    legal_board |= blank_board & (tmp << 9);

    // 右下
    tmp = all_side & (board_info.player_board >> 9);
    for _ in 0..5 {
        tmp |= all_side & (tmp >> 9)
    }
    legal_board |= blank_board & (tmp >> 9);

    // 左下
    tmp = all_side & (board_info.player_board >> 7);
    for _ in 0..5 {
        tmp |= all_side & (tmp >> 7)
    }
    legal_board |= blank_board & (tmp >> 7);

    return legal_board;
}

// ビットを指定した方向に一つ動かす，ただし端は除かれる
fn transfer(place_bit: &u64, direc: &i8) -> u64 {
    return match direc {
        0 => (place_bit << 8) & 0xffffffffffffff00, // 上
        1 => (place_bit << 7) & 0x7f7f7f7f7f7f7f00, // 右上
        2 => (place_bit >> 1) & 0x7f7f7f7f7f7f7f7f, // 右
        3 => (place_bit >> 9) & 0x007f7f7f7f7f7f7f, // 右下
        4 => (place_bit >> 8) & 0x00ffffffffffffff, // 下
        5 => (place_bit >> 7) & 0x00fefefefefefefe, // 左下
        6 => (place_bit << 1) & 0xfefefefefefefefe, // 左
        7 => (place_bit << 9) & 0xfefefefefefefe00, // 左上
        _ => panic!("transfer error."),
    };
}

// playerが石をうつ部分の処理
fn place(place_bit: u64, board_info: &mut BoardInfo) -> i8 {
    
    let legal_board: u64 = make_legal_board(&board_info);

    // パス
    if place_bit == 0 as u64 {
        if legal_board == 0 as u64 { // 本当に着手可能な手がないならパス
            // 相手側もパスなら終局
            let tmp_board_info: BoardInfo = BoardInfo {
                now_turn: board_info.now_turn.clone() * TURN_CHANGE_FACTOR,
                now_index: board_info.now_index.clone(),
                player_board: board_info.opponent_board.clone(),
                opponent_board: board_info.player_board.clone(),
            };
            if make_legal_board(&tmp_board_info) == 0 { // 相手側もパスの場合終局
                return GAME_SET;
            }

            return CONTINUE;
        }else{ // おける場所があるのにパスすることはできない
            return PLACE_ERR;
        }
    }

    // 石をうつ
    if (place_bit & legal_board) == place_bit { // 着手可能なら処理を続ける
        // 石を裏返す処理
        let mut rev: u64 = 0; // 裏返す部分のビットを立てる変数
        for k in 0..8 { // kで方向を定める
            let mut rev_sub: u64 = 0;
            let mut tmp: u64 = transfer(&place_bit, &k);
            // 相手のコマが続く限りrevの候補となるrev_subを更新し続ける
            while (tmp != 0) && ((tmp & board_info.opponent_board) != 0) {
                rev_sub |= tmp;
                tmp = transfer(&tmp, &k);
            }
            if (tmp & board_info.player_board) != 0 { // 自分の色で挟まれていれば実際にrev_subをrevに加える
                rev |= rev_sub;
            }
        }
        board_info.player_board ^= place_bit | rev; // XORをとる
        board_info.opponent_board ^= rev;
        // 石を裏返す処理終了

        board_info.now_index += 1;
        // 60手目を打ち終わったら終局
        if board_info.now_index > 60 {
            return GAME_SET;
        }
    }else{ // 着手可能でない
        return PLACE_ERR;
    };

    return CONTINUE;

}

// 終局かどうかを判定
fn is_game_over(board_info: &BoardInfo) -> bool {
    let player_legal_board = make_legal_board(board_info);
    let tmp_board_info = board_info.clone();
    let opponent_legal_board = make_legal_board(&tmp_board_info);
    return player_legal_board == 0 as u64 && opponent_legal_board == 0 as u64;
}

// 手番入れ替え
fn swap(board_info: &mut BoardInfo) {
    let tmp: u64 = board_info.player_board;
    board_info.player_board = board_info.opponent_board;
    board_info.opponent_board = tmp;
    board_info.now_turn *= TURN_CHANGE_FACTOR;
}

// 結果取得, 返り値は(黒コマ数，白コマ数，優勢)
fn get_result(board_info: &BoardInfo) -> (u32, u32, i8) {
    let mut black_count: u32 = board_info.player_board.count_ones();
    let mut white_count: u32 = board_info.opponent_board.count_ones();
    if board_info.now_turn == WHITE {
        let tmp = black_count;
        black_count = white_count;
        white_count = tmp;
    }
    let mut superior: i8 = BLACK;
    if black_count <= white_count {
        if black_count == white_count {
            superior = NONE;
        }else{
            superior = WHITE;
        }
    }
    return (black_count, white_count, superior);
}

// ゲーム開始
fn game_start(board_info: &BoardInfo) -> () {
    println!("");
    println!("************************");
    println!("");
    println!("GAME START");
    
    print_board_info(&board_info, evaluate(EVAL_NORMAL, &board_info));

    println!("");
    println!("************************");
    println!("");
}
// ゲーム終了
fn game_set(board_info: &BoardInfo) -> () {
    println!("");
    println!("************************");
    println!("");
    println!("GAME SET");
    print_board_info(&board_info, evaluate(EVAL_PERFECT, &board_info));

    println!("");
    println!("************************");
    println!("");
}

// どの関数で評価するか定める
fn evaluate(n: i8, board_info: &BoardInfo) -> i32 {
    return match n {
        EVAL_BY_POINTTABLE => eval_by_pointtable(board_info),
        EVAL_NORMAL        => eval_normal(board_info),
        EVAL_PERFECT       => eval_perfect(board_info),
        EVAL_WIN           => eval_win(board_info),
        _                  => panic!("there is not such way of evaluation"),
    };
}

// 得点テーブルを用いて盤面を評価する（デバッグ用）
fn eval_by_pointtable(board_info: &BoardInfo) -> i32 {
    let mut point: i32 = 0;
    point += (board_info.player_board & 0x8100000000000081).count_ones() as i32 * (100); // A1
    point += (board_info.player_board & 0x4281000000008142).count_ones() as i32 * (-50); // B1
    point += (board_info.player_board & 0x2400810000810024).count_ones() as i32 * (10);  // C1
    point += (board_info.player_board & 0x0042000000004200).count_ones() as i32 * (-70); // B2
    point += (board_info.player_board & 0x0024420000422400).count_ones() as i32 * (-5);  // C2
    point += (board_info.player_board & 0x0018244242241800).count_ones() as i32 * (-10); // D2,C3
    point += (board_info.player_board & 0x0000182424180000).count_ones() as i32 * (-5);  // D3
    
    point -= (board_info.opponent_board & 0x8100000000000081).count_ones() as i32 * (100); // A1
    point -= (board_info.opponent_board & 0x4281000000008142).count_ones() as i32 * (-50); // B1
    point -= (board_info.opponent_board & 0x2400810000810024).count_ones() as i32 * (10);  // C1
    point -= (board_info.opponent_board & 0x0042000000004200).count_ones() as i32 * (-70); // B2
    point -= (board_info.opponent_board & 0x0024420000422400).count_ones() as i32 * (-5);  // C2
    point -= (board_info.opponent_board & 0x0018244242241800).count_ones() as i32 * (-10); // D2,C3
    point -= (board_info.opponent_board & 0x0000182424180000).count_ones() as i32 * (-5);  // D3

    return point;
}

// 指定したますの開放度を計算する
fn count_openness(empty_board: u64, bit: u64) -> i32 {
    let mut count = 0;
    if bit & 0x00000000000000ff == 0 { // 下にますがある
        if (bit >> 8) & empty_board != 0 { // 下が空いている
            count += 1;
        }
    }
    if bit & 0x80808080808080ff == 0 { // 左下
        if (bit >> 7) & empty_board != 0 { // 左下
            count += 1;
        }
    }
    if bit & 0x8080808080808080 == 0 { // 左
        if (bit << 1) & empty_board != 0 { // 左
            count += 1;
        }
    }
    if bit & 0xff80808080808080 == 0 { // 左上
        if (bit << 9) & empty_board != 0 { // 左上
            count += 1;
        }
    }
    if bit & 0xff00000000000000 == 0 { // 上
        if (bit << 8) & empty_board != 0 { // 上
            count += 1;
        }
    }
    if bit & 0xff01010101010101 == 0 { // 右上
        if (bit << 7) & empty_board != 0 { // 右上
            count += 1;
        }
    }
    if bit & 0x0101010101010101 == 0 { // 右
        if (bit >> 1) & empty_board != 0 { // 右
            count += 1;
        }
    }
    if bit & 0x01010101010101ff == 0 { // 右下
        if (bit >> 9) & empty_board != 0 { // 右下
            count += 1;
        }
    }
    return count;
}

// 中盤に用いる評価関数
fn eval_normal(board_info: &BoardInfo) -> i32 {
    if board_info.player_board.count_ones() == 0 as u32 {
        return std::i32::MIN;
    }
    if board_info.opponent_board.count_ones() == 0 as u32 {
        return std::i32::MAX;
    }

    let empty_board = !(board_info.player_board | board_info.opponent_board);
    
    // ウイング，危険なC打ちをカウント
    let mut player_wing_count = 0;
    let mut player_c_place_count = 0;
    // 上の辺
    if empty_board & 0x8100000000000000 == 0x8100000000000000 { /* 隅にはなにも置かれていない */
        if board_info.player_board & 0x3c00000000000000 == 0x3c00000000000000 { // ブロックができている
            if (board_info.player_board & 0xff00000000000000 == 0x7c00000000000000 && empty_board & 0x0200000000000000 == 0x0200000000000000)
                || (board_info.player_board & 0xff00000000000000 == 0x3e00000000000000 && empty_board & 0x4000000000000000 == 0x4000000000000000) /* ウイングの形ができている */ {
                player_wing_count += 1;
            }
        }else{
            player_c_place_count += (board_info.player_board & 0x4200000000000000).count_ones() as i32;
        }
    }
    // 左の辺
    if empty_board & 0x8000000000000080 == 0x8000000000000080 { /* 隅にはなにも置かれていない */
        if board_info.player_board & 0x0000808080800000 == 0x0000808080800000 { // ブロックができている
            if (board_info.player_board & 0x8080808080808080 == 0x0080808080800000 && empty_board & 0x0000000000008000 == 0x0000000000008000)
                || (board_info.player_board & 0x8080808080808080 == 0x0000808080808000 && empty_board & 0x0080000000000000 == 0x0080000000000000) /* ウイングの形ができている */ {
                player_wing_count += 1;
            }
        }else{
            player_c_place_count += (board_info.player_board & 0x0080000000008000).count_ones() as i32;
        }
    }
    // 右の辺
    if empty_board & 0x0100000000000001 == 0x0100000000000001 { /* 隅にはなにも置かれていない */
        if board_info.player_board & 0x0000010101010000 == 0x0000010101010000 { // ブロックができている
            if (board_info.player_board & 0x0101010101010101 == 0x0001010101010000 && empty_board & 0x0000000000000100 == 0x0000000000000100)
                || (board_info.player_board & 0x0101010101010101 == 0x0000010101010100 && empty_board & 0x0001000000000000 == 0x0001000000000000) /* ウイングの形ができている */ {
                player_wing_count += 1;
            }
        }else{
            player_c_place_count += (board_info.player_board & 0x0001000000000100).count_ones() as i32;
        }
    }
    // 上の辺
    if empty_board & 0x0000000000000081 == 0x0000000000000081 { /* 隅にはなにも置かれていない */
        if board_info.player_board & 0x000000000000003c == 0x000000000000003c { // ブロックができている
            if (board_info.player_board & 0x00000000000000ff == 0x000000000000007c && empty_board & 0x0000000000000002 == 0x0000000000000002)
                || (board_info.player_board & 0x00000000000000ff == 0x000000000000003e && empty_board & 0x0000000000000040 == 0x0000000000000040) /* ウイングの形ができている */ {
                player_wing_count += 1;
            }
        }else{
            player_c_place_count += (board_info.player_board & 0x0000000000000042).count_ones() as i32;
        }
    }
    let mut opponent_wing_count = 0;
    let mut opponent_c_place_count = 0;
    // 上の辺
    if empty_board & 0x8100000000000000 == 0x8100000000000000 { /* 隅にはなにも置かれていない */
        if board_info.opponent_board & 0x3c00000000000000 == 0x3c00000000000000 { // ブロックができている
            if (board_info.opponent_board & 0xff00000000000000 == 0x7c00000000000000 && empty_board & 0x0200000000000000 == 0x0200000000000000)
                || (board_info.opponent_board & 0xff00000000000000 == 0x3e00000000000000 && empty_board & 0x4000000000000000 == 0x4000000000000000) /* ウイングの形ができている */ {
                opponent_wing_count += 1;
            }
        }else{
            opponent_c_place_count += (board_info.opponent_board & 0x4200000000000000).count_ones() as i32;
        }
    }
    // 左の辺
    if empty_board & 0x8000000000000080 == 0x8000000000000080 { /* 隅にはなにも置かれていない */
        if board_info.opponent_board & 0x0000808080800000 == 0x0000808080800000 { // ブロックができている
            if (board_info.opponent_board & 0x8080808080808080 == 0x0080808080800000 && empty_board & 0x0000000000008000 == 0x0000000000008000)
                || (board_info.opponent_board & 0x8080808080808080 == 0x0000808080808000 && empty_board & 0x0080000000000000 == 0x0080000000000000) /* ウイングの形ができている */ {
                opponent_wing_count += 1;
            }
        }else{
            opponent_c_place_count += (board_info.opponent_board & 0x0080000000008000).count_ones() as i32;
        }
    }
    // 右の辺
    if empty_board & 0x0100000000000001 == 0x0100000000000001 { /* 隅にはなにも置かれていない */
        if board_info.opponent_board & 0x0000010101010000 == 0x0000010101010000 { // ブロックができている
            if (board_info.opponent_board & 0x0101010101010101 == 0x0001010101010000 && empty_board & 0x0000000000000100 == 0x0000000000000100)
                || (board_info.opponent_board & 0x0101010101010101 == 0x0000010101010100 && empty_board & 0x0001000000000000 == 0x0001000000000000) /* ウイングの形ができている */ {
                opponent_wing_count += 1;
            }
        }else{
            opponent_c_place_count += (board_info.opponent_board & 0x0001000000000100).count_ones() as i32;
        }
    }
    // 上の辺
    if empty_board & 0x0000000000000081 == 0x0000000000000081 { /* 隅にはなにも置かれていない */
        if board_info.opponent_board & 0x000000000000003c == 0x000000000000003c { // ブロックができている
            if (board_info.opponent_board & 0x00000000000000ff == 0x000000000000007c && empty_board & 0x0000000000000002 == 0x0000000000000002)
                || (board_info.opponent_board & 0x00000000000000ff == 0x000000000000003e && empty_board & 0x0000000000000040 == 0x0000000000000040) /* ウイングの形ができている */ {
                opponent_wing_count += 1;
            }
        }else{
            opponent_c_place_count += (board_info.opponent_board & 0x0000000000000042).count_ones() as i32;
        }
    }
    
    // 確定石（4隅とそれに連接する石の数）をカウント
    let mut player_stable_count = 0;
    if board_info.player_board & 0x8000000000000000 == 0x8000000000000000 { // 左上
        let mut mask = 0x8000000000000000;
        let mut tmp_count = 1;
        for i in 1..8 { // 下へ
            if board_info.player_board & (mask >> 8*i) == (mask >> 8*i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 8 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 4;
        }
        player_stable_count += tmp_count;

        mask = 0x8000000000000000;
        tmp_count = 0;
        for i in 1..8 { // 右へ
            if board_info.player_board & (mask >> i) == (mask >> i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 7 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 3;
        }
        player_stable_count += tmp_count;
    }
    if board_info.player_board & 0x0100000000000000 == 0x0100000000000000 { // 右上
        let mut mask = 0x0100000000000000;
        let mut tmp_count = 1;
        for i in 1..8 { // 下へ
            if board_info.player_board & (mask >> 8*i) == (mask >> 8*i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 8 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 4;
        }
        player_stable_count += tmp_count;

        mask = 0x0100000000000000;
        tmp_count = 0;
        for i in 1..8 { // 左へ
            if board_info.player_board & (mask << i) == (mask << i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 7 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 3;
        }
        player_stable_count += tmp_count;
    }
    if board_info.player_board & 0x0000000000000001 == 0x0000000000000001 { // 右下
        let mut mask = 0x0000000000000001;
        let mut tmp_count = 1;
        for i in 1..8 { // 上へ
            if board_info.player_board & (mask << 8*i) == (mask << 8*i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 8 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 4;
        }
        player_stable_count += tmp_count;

        mask = 0x0000000000000001;
        tmp_count = 0;
        for i in 1..8 { // 左へ
            if board_info.player_board & (mask << i) == (mask << i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 7 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 3;
        }
        player_stable_count += tmp_count;
    }
    if board_info.player_board & 0x0000000000000080 == 0x0000000000000080 { // 左下
        let mut mask = 0x0000000000000080;
        let mut tmp_count = 1;
        for i in 1..8 { // 上へ
            if board_info.player_board & (mask << 8*i) == (mask << 8*i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 8 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 4;
        }
        player_stable_count += tmp_count;

        mask = 0x0000000000000080;
        tmp_count = 0;
        for i in 1..8 { // 右へ
            if board_info.player_board & (mask >> i) == (mask >> i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 7 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 3;
        }
        player_stable_count += tmp_count;
    }
    let mut opponent_stable_count = 0;
    if board_info.opponent_board & 0x8000000000000000 == 0x8000000000000000 { // 左上
        let mut mask = 0x8000000000000000;
        let mut tmp_count = 1;
        for i in 1..8 { // 下へ
            if board_info.opponent_board & (mask >> 8*i) == (mask >> 8*i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 8 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 4;
        }
        opponent_stable_count += tmp_count;

        mask = 0x8000000000000000;
        tmp_count = 0;
        for i in 1..8 { // 右へ
            if board_info.opponent_board & (mask >> i) == (mask >> i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 7 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 3;
        }
        opponent_stable_count += tmp_count;
    }
    if board_info.opponent_board & 0x0100000000000000 == 0x0100000000000000 { // 右上
        let mut mask = 0x0100000000000000;
        let mut tmp_count = 1;
        for i in 1..8 { // 下へ
            if board_info.opponent_board & (mask >> 8*i) == (mask >> 8*i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 8 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 4;
        }
        opponent_stable_count += tmp_count;

        mask = 0x0100000000000000;
        tmp_count = 0;
        for i in 1..8 { // 左へ
            if board_info.opponent_board & (mask << i) == (mask << i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 7 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 3;
        }
        opponent_stable_count += tmp_count;
    }
    if board_info.opponent_board & 0x0000000000000001 == 0x0000000000000001 { // 右下
        let mut mask = 0x0000000000000001;
        let mut tmp_count = 1;
        for i in 1..8 { // 上へ
            if board_info.opponent_board & (mask << 8*i) == (mask << 8*i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 8 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 4;
        }
        opponent_stable_count += tmp_count;

        mask = 0x0000000000000001;
        tmp_count = 0;
        for i in 1..8 { // 左へ
            if board_info.opponent_board & (mask << i) == (mask << i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 7 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 3;
        }
        opponent_stable_count += tmp_count;
    }
    if board_info.opponent_board & 0x0000000000000080 == 0x0000000000000080 { // 左下
        let mut mask = 0x0000000000000080;
        let mut tmp_count = 1;
        for i in 1..8 { // 上へ
            if board_info.opponent_board & (mask << 8*i) == (mask << 8*i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 8 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 4;
        }
        opponent_stable_count += tmp_count;

        mask = 0x0000000000000080;
        tmp_count = 0;
        for i in 1..8 { // 右へ
            if board_info.opponent_board & (mask >> i) == (mask >> i) {
                tmp_count += 1;
                continue;
            }
            break;
        }
        if tmp_count == 7 { // 一列全てが埋まっている場合は重複を考慮してcountを÷2しておく
            tmp_count = 3;
        }
        opponent_stable_count += tmp_count;
    }

    // 危険なX打ちの数
    let mut player_x_place_count = 0;
    if board_info.player_board & 0x0040000000000000 == 0x0040000000000000 && empty_board & 0x8000000000000000 == 0x8000000000000000 { // 左上
        player_x_place_count += 1;
    }
    if board_info.player_board & 0x0002000000000000 == 0x0002000000000000 && empty_board & 0x0100000000000000 == 0x0100000000000000 { // 右上
        player_x_place_count += 1;
    }
    if board_info.player_board & 0x0000000000000200 == 0x0000000000000200 && empty_board & 0x0000000000000001 == 0x0000000000000001 { // 右下
        player_x_place_count += 1;
    }
    if board_info.player_board & 0x0000000000004000 == 0x0000000000004000 && empty_board & 0x0000000000000080 == 0x0000000000000080 { // 左下
        player_x_place_count += 1;
    }

    let mut opponent_x_place_count = 0;
    if board_info.opponent_board & 0x0040000000000000 == 0x0040000000000000 && empty_board & 0x8000000000000000 == 0x8000000000000000 { // 左上
        opponent_x_place_count += 1;
    }
    if board_info.opponent_board & 0x0002000000000000 == 0x0002000000000000 && empty_board & 0x0100000000000000 == 0x0100000000000000 { // 右上
        opponent_x_place_count += 1;
    }
    if board_info.opponent_board & 0x0000000000000200 == 0x0000000000000200 && empty_board & 0x0000000000000001 == 0x0000000000000001 { // 右下
        opponent_x_place_count += 1;
    }
    if board_info.opponent_board & 0x0000000000004000 == 0x0000000000004000 && empty_board & 0x0000000000000080 == 0x0000000000000080 { // 左下
        opponent_x_place_count += 1;
    }

    // 着手可能手数
    let player_legal_board_count = make_legal_board(&board_info).count_ones() as i32;
    let mut tmp_board_info = board_info.clone();
    swap(&mut tmp_board_info);
    let opponent_legal_board_count = make_legal_board(&tmp_board_info).count_ones() as i32;

    // 開放度計算
    let mut player_openness = 0;
    let mut opponent_openness = 0;
    let mut mask: u64 = 0x8000000000000000;
    for _ in 0..BOARDSIZE {
        if board_info.player_board & mask != 0 {
            player_openness += count_openness(empty_board, mask);
        }else if board_info.opponent_board & mask != 0 {
            opponent_openness += count_openness(empty_board, mask);
        }
        mask = mask >> 1;
    }

    return (player_stable_count - opponent_stable_count) * WEIGHT_STABLE
        + (player_wing_count - opponent_wing_count) * WEIGHT_WING
        + (player_x_place_count - opponent_x_place_count) * WEIGHT_XMOVE
        + (player_c_place_count - opponent_c_place_count) * WEIGHT_CMOVE
        + (player_legal_board_count - opponent_legal_board_count) * WEIGHT_MOBILITY
        + (player_openness - opponent_openness) * WEIGHT_OPENNESS;
}

// 完全読み切り（個数も読む）
fn eval_perfect(board_info: &BoardInfo) -> i32 {
    return board_info.player_board.count_ones() as i32 - board_info.opponent_board.count_ones() as i32;
}

// 勝つかどうかだけ読む（個数は読まない）
fn eval_win(board_info: &BoardInfo) -> i32 {
    let diff = board_info.player_board.count_ones() as i32 - board_info.opponent_board.count_ones() as i32;
    return if diff > 0 {1} else if diff == 0 {0} else {-1};
}

// 探索（alpha-beta法による）
fn negamax(alpha_: i32, beta_: i32, limit: i8, board_info: &mut BoardInfo, way_of_eval: i8) -> i32 {
    let mut alpha: i32 = alpha_;
    let beta: i32 = beta_;

    if limit == 0 || is_game_over(board_info) { // 深さ制限 or 終局
        return evaluate(way_of_eval, &board_info);
    }

    let legal_board: u64 = make_legal_board(&board_info);
    let mut score: i32;

    if legal_board.count_ones() == 0 as u32 { // パス
        let tmp_board_info: BoardInfo = board_info.clone();
        swap(board_info);
        score = -negamax(-beta, -alpha, limit, board_info, way_of_eval); // さらに奥を深さを変えずに探索
        *board_info = tmp_board_info; // 盤面を元に戻す
        return score;
    }

    let mut score_max: i32 = std::i32::MIN;
    let mut mask: u64 = 0x0000000000000001;

    for _ in 0..BOARDSIZE {
        if mask & legal_board != 0 { // maskが実際における場所であるとき
            let tmp_board_info: BoardInfo = board_info.clone();
            place(mask, board_info); // 実際においてみる
            swap(board_info);
            score = -negamax(-beta, -alpha, limit-1, board_info, way_of_eval);
            *board_info = tmp_board_info; // 盤面を元に戻す

            if score >= beta { // βカット
                return score;
            }
            if score > score_max { // 得点が高くなるように更新
                score_max = score;
                alpha = max(alpha, score_max); // α値更新
            }
        }
        mask = mask << 1;
    }

    return score_max;
}

// 着手する手を思考する
fn decide(board_info: &mut BoardInfo, left_time: i32, way_of_eval: i8, limit: i8) -> u64 {
    let legal_board: u64 = make_legal_board(&board_info);

    if legal_board == 0 as u64 { // おける手がなければパスを選択
        return 0 as u64;
    }

    if legal_board.count_ones() == 1 as u32 { // おける手が一つしかなければそのままそれを返す
        return legal_board;
    }

    let mut ret: u64 = 0;

    // 実行速度計測開始
    let start = Instant::now();

    println!("debug: left_time={}, way_of_eval={}, limit={}", left_time, way_of_eval, limit);

    // 作戦: alpha-beta法を用いた探索
    let mut mask: u64 = 0x0000000000000001;
    let mut max_eval: i32 = std::i32::MIN;

    // マルチスレッドによる実装
    let mut thread_count = 0;
    let mut threads = Vec::new();
    let mut receiver = Vec::new();

    for _ in 0..BOARDSIZE {
        if mask & legal_board != 0 { // maskが実際における場所であるとき
            thread_count += 1;
            // メインスレッド -> サブスレッドのチャンネル
            let (s1, r1) = mpsc::channel();
            // サブスレッド -> メインスレッドのチャンネル
            let (s2, r2) = mpsc::channel();
            threads.push(thread::spawn(move || {
                // メインスレッドから情報が送られてくる
                let (mask, limit, mut tmp_board_info, way_of_eval) = r1.recv().unwrap();
                place(mask, &mut tmp_board_info);
                swap(&mut tmp_board_info);
                let tmp = -negamax(std::i32::MIN+1, std::i32::MAX-1, limit-1, &mut tmp_board_info, way_of_eval); // int_maxやint_minをnegateするとoverflowが発生するため，値を調節している
                let (c1, c2) = bit_to_point(mask);
                let mov = vec![c1, c2];
                let mov_string: String = mov.iter().collect();
                match s2.send((mask, tmp)) {
                    Ok(_) => println!("debug: score={}, place={}", tmp, mov_string),
                    Err(mpsc::SendError(_)) => println!("debug: this thread is not useless; ignore it."), // すでに計算が不要でいらないthread
                };
            }));
            s1.send((mask, limit, board_info.clone(), way_of_eval)).unwrap();
            receiver.push(r2);
        }
        mask = mask << 1;
    }

    // 集計
    let mut finished_thread_count = 0;
    let mut i = 0;
    while finished_thread_count != thread_count {
        match receiver[i].try_recv() { // busy loopでスレッドを順にみていく
            Ok((bit_ok, tmp_ok)) => {
                finished_thread_count += 1;
                let (bit, tmp) = (bit_ok, tmp_ok);

                if way_of_eval == EVAL_WIN { // 必勝読みでは，必勝できる手が見つかったら探索を打ち切ってその手を打つ
                    if tmp == 1 { // 必勝できる手が見つかった
                        println!("Win-Road found: stop searching");
                        return bit;
                        // 注意：このときまだ動き続けているスレッドがあるので，Err(mpsc::SendError(_))として別処理が必要
                    }
                }

                // 得点が高くなるように更新
                if tmp > max_eval { 
                    max_eval = tmp;
                    ret = bit;
                }
            },
            Err(mpsc::TryRecvError::Empty) => {
                let elapse = start.elapsed();
                let sec = elapse.as_secs();
                // 必勝読みや完全読みの境目にいるとき，残り10秒以下ならやばくなってくるので，EVAL_NORMALで計算し直す
                if (way_of_eval == EVAL_WIN || way_of_eval == EVAL_PERFECT) && left_time - sec as i32 * 1000 <= 10000 && EVAL_PERFECT_DEPTH <= (60 - board_info.now_index) && (60 - board_info.now_index) <= EVAL_WIN_DEPTH { 
                    return decide(board_info, left_time, EVAL_NORMAL, 10);
                    // 注意：このときまだ動き続けているスレッドがあるので，Err(mpsc::SendError(_))として別処理が必要
                }
                continue;
            },
            _ => panic!("Non-expected error occured"),
        };  
        i += 1;
        if i == thread_count {
            i = 0;
        }
    }
    for thread in threads {
        thread.join().unwrap();
    }

    // 勝敗予想
    if way_of_eval == EVAL_WIN {
        match max_eval {
            1  => println!("I predicate I will win."),
            0  => println!("I predicate this game will end in a draw."),
            -1 => println!("I predicate I will lose."),
            _  => panic!("Invalid value"),
        };
    }else if way_of_eval == EVAL_PERFECT {
        if max_eval > 0 {
            println!("I predicate I will win by {} points.", max_eval);
        }else if max_eval == 0 {
            println!("I predicate this game will end in a draw.");
        }else{
            println!("I predicate I will lose by {} points.", -max_eval);
        }
    }

    return ret;
}

// board_infoの状況に合わせて適切なevaluatorを選択する
fn choose_evaluator(board_info: &BoardInfo) -> i8 {
    if MAX_TURNS - board_info.now_index <= EVAL_WIN_DEPTH {
        if MAX_TURNS - board_info.now_index <= EVAL_PERFECT_DEPTH {
            return EVAL_PERFECT;
        }
        return EVAL_WIN;
    }
    return EVAL_NORMAL;
}

fn read_tcp(reader: &mut BufReader<&TcpStream>) -> Vec<u8> {
    let mut msg = String::new();
    reader.read_line(&mut msg).expect("read failure");
    println!("RECEIVED: {}", msg);

    return Vec::<u8>::from(msg);
}

fn write_tcp(writer: &mut BufWriter<&TcpStream>, comment: &str) {
    let msg = format!("{}\n", comment);
    writer.write(msg.as_bytes()).expect("write failure");
    writer.flush().unwrap();
    println!("SENT: {}", msg);
}

fn concat(vec: &Vec<u8>) -> i32 {
    let mut acc: i32 = 0;
    for i in 0..(vec.len()) {
        acc *= 10;
        acc += vec[i] as i32 - 48;
    }
    return acc;
}

// STARTを受信した際，自分の色，相手の名前，残りの時間をタプルで返す
fn read_start(buffer: &Vec<u8>) -> (i8, Vec<char>, i32) { 
    if buffer[0] == 'S' as u8 {  // STARTがきた 
        let mut i = 1;
        while buffer[i] != 'B' as u8 && buffer[i] != 'W' as u8 {
            i += 1;
        }
        let my_color: i8;
        if buffer[i] == 'B' as u8 { // 自分はBLACK
            my_color = BLACK; // 自分の色を格納する
        }else{ // 自分はWHITE
            my_color = WHITE; // 自分の色を格納する
        }
        i += 6;
        let mut opponent_name = Vec::new();
        while buffer[i] != ' ' as u8 {
            opponent_name.push(buffer[i] as char);
            i += 1;
        }
        i += 1;
        let mut left_time_vec = Vec::new();
        while buffer[i] != '\n' as u8 {
            left_time_vec.push(buffer[i]);
            i += 1;
        }
        let left_time: i32 = concat(&left_time_vec);
        return (my_color, opponent_name, left_time);
    }else{
        panic!("could not receive 'START'");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut host = "localhost";
    let mut port = "3000";
    let mut name = "Player";

    let mut i = 1;
    while i < args.len() {
        if args[i] == "-H" { // host
            host = &args[i+1];
            i += 2;
        }else if args[i] == "-p" { // port
            port = &args[i+1];
            i += 2;
        }else if args[i] == "-n" { // name
            name = &args[i+1];
            i += 2;
        }else {
            panic!("INVALID args");
        }
    }

    let host_and_port = format!("{}:{}", host, port);
    let mut addrs = host_and_port.to_socket_addrs().unwrap();

    if let Some(addr) = addrs.find(|x| (*x).is_ipv4()) {
        match TcpStream::connect(addr) {
            Err(_) => println!("connection failed"),
            Ok(stream) => {
                // 接続完了
                println!("connection succeeded");
                let mut reader = BufReader::new(&stream);
                let mut writer = BufWriter::new(&stream);

                // 開始（OPENをsendする）
                write_tcp(&mut writer, &format!("OPEN {}", name));

                // 自分の色の情報を格納
                let mut my_color: i8 = BLACK;
                // 相手の名前の情報を格納
                let mut _opponent_name = Vec::<char>::new();
                // 残り時間の情報を格納
                let mut left_time = 0;
                // 盤面の履歴の情報を格納
                let mut board_info_history = Vec::<BoardInfo>::new();
                // 盤面の情報を格納
                let mut board_info = BoardInfo{
                    now_turn: BLACK,
                    now_index: 1,
                    player_board: 0x0000000810000000,
                    opponent_board: 0x0000001008000000,
                };

                // メインループ
                let mut bit: u64; // 打つ手（0ならpassを表す）
                let mut way_of_eval: i8; // 評価関数をどれにするかを定める
                let mut limit: i8;

                let mut is_waiting: bool = true; // 対戦待ち状態ならtrue
                let mut buffer = Vec::<u8>::new();
                loop{

                    if is_waiting == true { // 対戦待ち状態

                        buffer = read_tcp(&mut reader);

                        if buffer[0] == 'B' as u8 { // BYEがきた
                            return (); // プログラム終了
                        }else if buffer[0] == 'S' as u8 { // STARTがきた
                            // 初期化処理
                            let tmp = read_start(&buffer);
                            my_color = tmp.0;
                            _opponent_name = tmp.1;
                            left_time = tmp.2;
                            board_info_history = Vec::<BoardInfo>::new();
                            board_info = BoardInfo{
                                now_turn: BLACK,
                                now_index: 1,
                                player_board: 0x0000000810000000,
                                opponent_board: 0x0000001008000000,
                            };
                            way_of_eval = choose_evaluator(&board_info);
                            is_waiting = false;
                            game_start(&board_info);
                        }else{
                            panic!("could not receive 'BYE' or 'START'");
                        }
                    }

                    if board_info.now_turn == my_color { //自分のターン

                        // 自分の手を思考
                        way_of_eval = choose_evaluator(&board_info);
                        limit = match way_of_eval {
                            EVAL_BY_POINTTABLE => EVAL_BY_POINTTABLE_DEPTH,
                            EVAL_NORMAL        => EVAL_NORMAL_DEPTH,
                            EVAL_WIN           => EVAL_WIN_DEPTH,
                            EVAL_PERFECT       => EVAL_PERFECT_DEPTH,
                            _                  => panic!("there is not such way of evaluation"),
                        };
                        bit = decide(&mut board_info, left_time, way_of_eval, limit);

                        // 自分の手を送信
                        if bit == 0 as u64 {
                            write_tcp(&mut writer, "MOVE PASS");
                        }else{
                            let (c1, c2) = bit_to_point(bit);
                            write_tcp(&mut writer, &format!("MOVE {}{}", c1, c2));
                        }

                        // ACK待ち
                        buffer = read_tcp(&mut reader);

                        if buffer[0] == 'E' as u8 { // ENDがきた
                            // 試合終了
                            game_set(&board_info);
                            is_waiting = true;
                            continue;
                        }else if buffer[0] == 'A' as u8 { // ACKがきた
                            let mut i = 4;
                            let mut left_time_vec = Vec::new();
                            while buffer[i] != '\n' as u8 {
                                left_time_vec.push(buffer[i]);
                                i += 1;
                            }
                            left_time = concat(&left_time_vec);

                            board_info_history.push(board_info.clone());

                            match place(bit, &mut board_info) {
                                CONTINUE | PLACE_ERR | GAME_SET => {swap(&mut board_info);
                                    print_board_info_simply(&board_info);
                                    continue
                                }, // ゲーム終了，中断の判定はサーバーがやってくれるのでとりあえず中断，終了の場合もとりあえず次に回す
                                _ => panic!("undefined return value of place"),
                            };
                        }else{
                            panic!("could not receive 'ACK'");
                        }

                    }else{ // 相手のターン
                        buffer = read_tcp(&mut reader);

                        if buffer[0] == 'M' as u8 {  // MOVEがきた（moveがきたので相手は合法手をうっていると確定）
                            let mut i = 1;
                            while buffer[i] != ' ' as u8 {
                                i += 1;
                            }
                            i += 1;
                            if buffer[i] == 'P' as u8 { // PASS
                                bit = 0 as u64;
                            }else{
                                bit = point_to_bit(buffer[i] as char, buffer[i+1] as char);
                            }
                        
                            board_info_history.push(board_info.clone());

                            match place(bit, &mut board_info) {
                                CONTINUE | PLACE_ERR | GAME_SET => {swap(&mut board_info);
                                    print_board_info_simply(&board_info);
                                    continue
                                }, // ゲーム終了，中断の判定はサーバーがやってくれるのでとりあえず中断，終了の場合もとりあえず次に回す
                                _ => panic!("undefined return value of place"),
                            };

                        }else if buffer[0] == 'E' as u8 { // ENDがきた
                            // 試合終了
                            game_set(&board_info);
                            is_waiting = true;
                            continue;
                        }else{
                            panic!("could not receive 'MOVE'");
                        }     
                    }
                }
            },
        }
    }else{
        panic!("invalid host:port");
    }
}