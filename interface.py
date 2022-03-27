# ユーザとのインターフェース部分を提供する，Rustプログラムとソケットを通じて通信する
import socket
import tkinter
import sys
import subprocess

WIN = 1
DRAW = 0
LOSE = -1

BLACK = 1
EMPTY = 0
WHITE = -1
WALL = 2

NONE = 0
UPPER = 1
UPPERLEFT = 2
LEFT = 4
LOWERLEFT = 8
LOWER = 16
LOWERRIGHT = 32
RIGHT = 64
UPPERRIGHT = 128

MAXTURNS = 60
BOARD_LINE_SIZE = 8
BOARD_SIZE = BOARD_LINE_SIZE ** 2

# canvas
root = tkinter.Tk()
root.title("Othello")
canvas = tkinter.Canvas(root, width=300, height=300, bg="palegreen")

def encoder(str):
    try:
        x = ord(str[0]) - ord("A") + 1
        y = int(str[1])
        point = Point(x,y)
        return point
    except: #意味不明入力はおけないものを返す
        return Point(0,0)

class Point:
    def __init__(self,x=0,y=0): #デフォルト値は(0,0)
        self.x = x
        self.y = y
    
class Disc(Point):#Pointを継承
    def __init__(self,x=0,y=0,color=1):
        super().__init__(x,y)
        self.color = color

class Board:
    def __init__(self):
        #ボードの設定
        self.board = [[EMPTY for _ in range(BOARD_LINE_SIZE+2)] for _ in range(BOARD_LINE_SIZE+2)]
        for x in range(BOARD_LINE_SIZE+2):
            self.board[x][0] = WALL
            self.board[x][BOARD_LINE_SIZE+1] = WALL
        for y in range(BOARD_LINE_SIZE+2):
            self.board[0][y] = WALL
            self.board[BOARD_LINE_SIZE+1][y] = WALL
        self.board[4][4] = WHITE
        self.board[5][5] = WHITE
        self.board[4][5] = BLACK
        self.board[5][4] = BLACK

        self.turns = 0
        self.currentcolor = BLACK #黒が先手
        self.updatelog = []
        self.movable_dir = [[[NONE for _ in range(BOARD_LINE_SIZE+2)] for _ in range(BOARD_LINE_SIZE+2)] for _ in range(MAXTURNS+1)]
        self.movable_pos = [[] for _ in range(MAXTURNS+1)]
        self.colorstorage = [2,BOARD_LINE_SIZE*BOARD_LINE_SIZE - 4,2] #[0]:WHITE, [1]:EMPTY, [2]:BLACKの数
        self.liberty = [[0 for _ in range(BOARD_LINE_SIZE+2)] for _ in range(BOARD_LINE_SIZE+2)]

        self.initMovable()

    def checkMobility(self, disc):
        if self.board[disc.x][disc.y] != EMPTY:
            return NONE
        
        direc = NONE

        if self.board[disc.x][disc.y-1] == -disc.color: #上
            new_x = disc.x
            new_y = disc.y-2
            while self.board[new_x][new_y] == -disc.color:
                new_y -= 1
            if self.board[new_x][new_y] == disc.color:
                direc = direc | UPPER

        if self.board[disc.x][disc.y+1] == -disc.color: #下
            new_x = disc.x
            new_y = disc.y+2
            while self.board[new_x][new_y] == -disc.color:
                new_y += 1
            if self.board[new_x][new_y] == disc.color:
                direc = direc | LOWER

        if self.board[disc.x+1][disc.y] == -disc.color: #右
            new_x = disc.x+2
            new_y = disc.y
            while self.board[new_x][new_y] == -disc.color:
                new_x += 1
            if self.board[new_x][new_y] == disc.color:
                direc = direc | RIGHT
        
        if self.board[disc.x-1][disc.y] == -disc.color: #左
            new_x = disc.x-2
            new_y = disc.y
            while self.board[new_x][new_y] == -disc.color:
                new_x -= 1
            if self.board[new_x][new_y] == disc.color:
                direc = direc | LEFT
        
        if self.board[disc.x+1][disc.y-1] == -disc.color: #右上
            new_x = disc.x+2
            new_y = disc.y-2
            while self.board[new_x][new_y] == -disc.color:
                new_x += 1
                new_y -= 1
            if self.board[new_x][new_y] == disc.color:
                direc = direc | UPPERRIGHT

        if self.board[disc.x+1][disc.y+1] == -disc.color: #右下
            new_x = disc.x+2
            new_y = disc.y+2
            while self.board[new_x][new_y] == -disc.color:
                new_x += 1
                new_y += 1
            if self.board[new_x][new_y] == disc.color:
                direc = direc | LOWERRIGHT

        if self.board[disc.x-1][disc.y+1] == -disc.color: #左下
            new_x = disc.x-2
            new_y = disc.y+2
            while self.board[new_x][new_y] == -disc.color:
                new_x -= 1
                new_y += 1
            if self.board[new_x][new_y] == disc.color:
                direc = direc | LOWERLEFT

        if self.board[disc.x-1][disc.y-1] == -disc.color: #左上
            new_x = disc.x-2
            new_y = disc.y-2
            while self.board[new_x][new_y] == -disc.color:
                new_x -= 1
                new_y -= 1
            if self.board[new_x][new_y] == disc.color:
                direc = direc | UPPERLEFT
        
        return direc

    def initMovable(self):
        self.movable_pos[self.turns] = [] #clear

        for x in range(1,BOARD_LINE_SIZE+1):
            for y in range(1,BOARD_LINE_SIZE+1):
                direc = self.checkMobility(Disc(x,y,self.currentcolor))

                if direc != NONE: #置ける場所だった
                    #self.movable_pos[self.turns].append(Disc(x,y,self.currentcolor)) #discをこのように代入する，disc.x等を準備してdiscを代入しようとすると，append後の値も動いてしまう！！！
                    self.movable_pos[self.turns].append(Point(x,y)) #debug
                self.movable_dir[self.turns][x][y] = direc
    
    def flipDiscs(self, point):

        direc = self.movable_dir[self.turns][point.x][point.y]

        update = []

        self.board[point.x][point.y] = self.currentcolor
        update.append(Disc(point.x, point.y, self.currentcolor))

        #開放度計算
        self.liberty[point.x+1][point.y] -= 1
        self.liberty[point.x+1][point.y-1] -= 1
        self.liberty[point.x][point.y-1] -= 1
        self.liberty[point.x-1][point.y-1] -= 1
        self.liberty[point.x-1][point.y] -= 1
        self.liberty[point.x-1][point.y+1] -= 1
        self.liberty[point.x][point.y+1] -= 1
        self.liberty[point.x+1][point.y+1] -= 1

        if direc & UPPER != 0: #上における
            y = point.y - 1
            while self.board[point.x][y] != self.currentcolor:
                self.board[point.x][y] = self.currentcolor
                update.append(Disc(point.x, y, self.currentcolor))
                y -= 1
        
        if direc & LOWER != 0: #下における
            y = point.y + 1
            while self.board[point.x][y] != self.currentcolor:
                self.board[point.x][y] = self.currentcolor
                update.append(Disc(point.x, y, self.currentcolor))
                y += 1

        if direc & RIGHT != 0: #右における
            x = point.x + 1
            while self.board[x][point.y] != self.currentcolor:
                self.board[x][point.y] = self.currentcolor
                update.append(Disc(x, point.y, self.currentcolor))
                x += 1

        if direc & LEFT != 0: #左における
            x = point.x - 1
            while self.board[x][point.y] != self.currentcolor:
                self.board[x][point.y] = self.currentcolor
                update.append(Disc(x, point.y, self.currentcolor))
                x -= 1

        if direc & UPPERRIGHT != 0: #右上における
            x = point.x + 1
            y = point.y - 1
            while self.board[x][y] != self.currentcolor:
                self.board[x][y] = self.currentcolor
                update.append(Disc(x, y, self.currentcolor))
                x += 1
                y -= 1

        if direc & LOWERRIGHT != 0: #右下における
            x = point.x + 1
            y = point.y + 1
            while self.board[x][y] != self.currentcolor:
                self.board[x][y] = self.currentcolor
                update.append(Disc(x, y, self.currentcolor))
                x += 1
                y += 1

        if direc & LOWERLEFT != 0: #左下における
            x = point.x - 1
            y = point.y + 1
            while self.board[x][y] != self.currentcolor:
                self.board[x][y] = self.currentcolor
                update.append(Disc(x, y, self.currentcolor))
                x -= 1
                y += 1
        
        if direc & UPPERLEFT != 0: #左上における
            x = point.x - 1
            y = point.y - 1
            while self.board[x][y] != self.currentcolor:
                self.board[x][y] = self.currentcolor
                update.append(Disc(x, y, self.currentcolor))
                x -= 1
                y -= 1
        
        flipped = len(update)

        self.colorstorage[self.currentcolor + 1] += flipped
        self.colorstorage[-self.currentcolor + 1] -= flipped-1
        self.colorstorage[EMPTY + 1] -= 1

        self.updatelog.append(update)

    def isGameOver(self):
        if self.turns == MAXTURNS:
            return True
        
        if len(self.movable_pos[self.turns]) != 0:
            return False

        disc = Disc(0,0,-self.currentcolor)

        for x in range(1,BOARD_LINE_SIZE+1):
            disc.x = x
            for y in range(1,BOARD_LINE_SIZE+1):
                disc.y = y
                if self.checkMobility(disc) != NONE:
                    return False
        
        return True
            
    def move(self, point):
        if point.x < 0 or point.x > BOARD_LINE_SIZE:
            return False
        if point.y < 0 or point.y > BOARD_LINE_SIZE:
            return False
        if self.movable_dir[self.turns][point.x][point.y] == NONE:
            return False

        self.flipDiscs(point)

        self.turns += 1
        self.currentcolor *= -1

        self.initMovable()

        return True

    def move_pass(self):
        if len(self.movable_pos[self.turns]) != 0:
            return False
        
        if self.isGameOver == True:
            return False
        
        self.currentcolor *= -1

        self.updatelog.append([])

        self.initMovable()

        return True

    def undo(self):
        if self.turns == 0:
            return False
        
        self.currentcolor *= -1

        update = self.updatelog.pop()

        if len(update) == 0: #前回がパス
            self.movable_pos[self.turns] = []

            for x in range(1,BOARD_LINE_SIZE+1):
                for y in range(1,BOARD_LINE_SIZE+1):
                    self.movable_dir[self.turns][x][y] = NONE
        
        else:
            self.turns -= 1

            self.board[update[0].x][update[0].y] = EMPTY
            for i in range(1,len(update)):
                self.board[update[i].x][update[i].y] = -self.currentcolor
            
            undo_flipped = len(update)
            self.colorstorage[self.currentcolor + 1] -= undo_flipped
            self.colorstorage[-self.currentcolor + 1] += undo_flipped - 1
            self.colorstorage[EMPTY + 1] += 1

        return True

    def printBoard(self):
        
        for y in range(1,BOARD_LINE_SIZE+1):
            for x in range(1, BOARD_LINE_SIZE+1):
                canvas.delete("disc"+str(x)+str(y))
                canvas.delete("indicator"+str(x)+str(y))
        for i in self.movable_pos[self.turns]:
            canvas.create_rectangle(30+30*(i.x-1)+5, 20+30*(i.y-1)+5, 30+30*i.x-5, 20+30*i.y-5, fill="green", tag = "indicator"+str(i.x)+str(i.y))
        for y in range(1,BOARD_LINE_SIZE+1):
            for x in range(1, BOARD_LINE_SIZE+1):
                if self.board[x][y] == WHITE:
                    canvas.create_oval(30+30*(x-1)+5, 20+30*(y-1)+5, 30+30*x-5,20+30*y-5, fill="white", outline="black", width="3", tag = "disc"+str(x)+str(y))
                elif self.board[x][y] == BLACK:
                    canvas.create_oval(30+30*(x-1)+5, 20+30*(y-1)+5, 30+30*x-5,20+30*y-5, fill="black", outline="black", width="3", tag = "disc"+str(x)+str(y))
        canvas.pack()
        print("")
        print("TURN:", self.turns)
        print("BLACK:", self.colorstorage[2])
        print("WHITE:", self.colorstorage[0])
        print("EMPTY:", self.colorstorage[1])
        print("")

def gameover(board):
    print("")
    print("-----Game Over-----")
    board.printBoard()
    if board.colorstorage[0] > board.colorstorage[2]:
        print("WINNER WHITE")
    elif board.colorstorage[0] < board.colorstorage[2]:
        print("WINNER BLACK")
    else:
        print("DRAW")
    exit()

def exit():
    input("press enter to exit")
    sys.exit()

def main():
    # guiの用意
    for i in range(1,10):
        canvas.create_line(30*i,30-10,30*i,270-10,fill="black",width="5")
        canvas.create_line(30,30*i-10,270,30*i-10,fill="black",width="5")
    for i in range(1,9):
        canvas.create_text(47+30*(i-1), 10, text = chr(ord("a")+(i-1)), fill="black", font = ("Times New Roman", 20))
        canvas.create_text(17,30*i+5, text=str(i),fill="black", font = ("Times New Roman", 20))
    canvas.pack()

    inp = input("Your Color? (b/w): ")
    if inp == "b":
        my_color = "WHITE"
    elif inp == "w":
        my_color = "BLACK"
    else:
        print("INVALID")
        return

    host = 'localhost'
    port = 3000
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    while True:
        try:
            # socket接続
            print("Connecting...")
            sock.bind((host,port))
            sock.listen(1) # キューの最大数を規定

            # 子プロセスとしてrustのプログラムをたてる
            proc = subprocess.Popen(['./target/release/reversi', '-p', str(port)])

            print("Listening on {}.{}".format(host, port))
            break
        except:
            # portが開いていなかった場合portをずらす
            port += 1

    client_sock, client_addr = sock.accept()
    print("Connection Succeeded")

    # OPENメッセージが送られてくる
    recv_line = client_sock.recv(4096).decode()

    # 対戦開始を知らせる
    send_line = "START "+my_color+" user 6000000\n"
    client_sock.send(bytes(send_line, 'utf-8'))

    #準備
    board = Board()

    while True:
        if my_color == "BLACK": # CPU先攻

            board.printBoard()

            # CPUのターン
            recv_line = client_sock.recv(4096).decode()
            if recv_line[0:5] == "MOVE ":
                if recv_line[5] != "P": # PASSでない
                    board.move(encoder(recv_line[5:7]))
                    print("CPU's Move: "+recv_line[5:7])
                else: # PASS
                    board.move_pass()
                    print("CPU's Move: PASS")
            else:
                print("INVALID MOVE")
            
            if board.isGameOver(): # ゲーム終了
                # ENDを送信
                send_line = "END\n"
                client_sock.send(bytes(send_line, 'utf-8'))
                send_line = "BYE\n"
                client_sock.send(bytes(send_line, 'utf-8'))
                client_sock.close()
                sock.close()
                gameover(board)
            else:
                # ACKを知らせる
                send_line = "ACK 6000000\n"
                client_sock.send(bytes(send_line, 'utf-8'))

            board.printBoard()

            # playerのターン
            if len(board.movable_pos[board.turns]) == 0:
                print("Cannot put anywhere")
                send_line = "MOVE PASS\n"
                client_sock.send(bytes(send_line, 'utf-8'))
                board.move_pass()
            else:
                while True:
                    inp = input("enter your move here (pass:\"p\", undo:\"u\", exit:\"x\") : ")
                    if inp == "p":
                        if board.move_pass() == False:
                            print("Cannot pass")
                            continue
                    elif inp == "u":
                        board.undo()
                        is_undo_succeeded = board.undo()
                        if is_undo_succeeded:
                            send_line = "UNDO\n"
                            client_sock.send(bytes(send_line, 'utf-8'))
                        board.printBoard()
                        continue
                    elif inp == "x":
                        send_line = "END\n"
                        client_sock.send(bytes(send_line, 'utf-8'))
                        send_line = "BYE\n"
                        client_sock.send(bytes(send_line, 'utf-8'))
                        client_sock.close()
                        sock.close()
                        sys.exit()
                    elif board.move(encoder(inp[0].upper()+inp[1])) == False:
                        print("Cannot put there")
                        continue

                    if board.isGameOver(): # ゲーム終了
                        # ENDを送信
                        send_line = "END\n"
                        client_sock.send(bytes(send_line, 'utf-8'))
                        send_line = "BYE\n"
                        client_sock.send(bytes(send_line, 'utf-8'))
                        client_sock.close()
                        sock.close()
                        gameover(board)

                    send_line = "MOVE "+inp[0].upper()+inp[1]+"\n"
                    client_sock.send(bytes(send_line, 'utf-8'))
                    break
        
        else: # CPU後攻
            board.printBoard()

            # playerのターン
            if len(board.movable_pos[board.turns]) == 0:
                print("Cannot put anywhere")
                send_line = "MOVE PASS\n"
                client_sock.send(bytes(send_line, 'utf-8'))
                board.move_pass()
            else:
                while True:
                    inp = input("enter your move here (pass:\"p\", undo:\"u\", exit:\"x\") : ")
                    if inp == "p":
                        if board.move_pass() == False:
                            print("Cannot pass")
                            continue
                    elif inp == "u":
                        board.undo()
                        is_undo_succeeded = board.undo()
                        if is_undo_succeeded:
                            send_line = "UNDO\n"
                            client_sock.send(bytes(send_line, 'utf-8'))
                        board.printBoard()
                        continue
                    elif inp == "x":
                        send_line = "END\n"
                        client_sock.send(bytes(send_line, 'utf-8'))
                        send_line = "BYE\n"
                        client_sock.send(bytes(send_line, 'utf-8'))
                        client_sock.close()
                        sock.close()
                        sys.exit()
                    elif board.move(encoder(inp[0].upper()+inp[1])) == False:
                        print("Cannot put there")
                        continue

                    if board.isGameOver(): # ゲーム終了
                        # ENDを送信
                        send_line = "END\n"
                        client_sock.send(bytes(send_line, 'utf-8'))
                        send_line = "BYE\n"
                        client_sock.send(bytes(send_line, 'utf-8'))
                        client_sock.close()
                        sock.close()
                        gameover(board)

                    send_line = "MOVE "+inp[0].upper()+inp[1]+"\n"
                    client_sock.send(bytes(send_line, 'utf-8'))
                    break


            board.printBoard()

            # CPUのターン
            recv_line = client_sock.recv(4096).decode()
            if recv_line[0:5] == "MOVE ":
                if recv_line[5] != "P": # PASSでない
                    board.move(encoder(recv_line[5:7]))
                    print("CPU's Move: "+recv_line[5:7])
                else: # PASS
                    board.move_pass()
                    print("CPU's Move: PASS")
            else:
                print("INVALID MOVE")
            
            if board.isGameOver(): # ゲーム終了
                # ENDを送信
                send_line = "END\n"
                client_sock.send(bytes(send_line, 'utf-8'))
                send_line = "BYE\n"
                client_sock.send(bytes(send_line, 'utf-8'))
                client_sock.close()
                sock.close()
                gameover(board)
            else:
                # ACKを知らせる
                send_line = "ACK 6000000\n"
                client_sock.send(bytes(send_line, 'utf-8'))

    root.mainloop()

if __name__ == "__main__":
    main()


